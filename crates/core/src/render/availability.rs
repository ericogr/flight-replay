// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! What the files at hand allow.
//!
//! The OSD needs an `.osd` file and the font of the goggles, the SRT data need an `.srt` file, and the sound needs an
//! `.osd` file of a firmware whose telemetry can be read, with the throttle in it. What a video lacks of these is
//! left out of its render, with the reason, instead of stopping it: a video with only an `.srt` file still gets the
//! SRT data. The application and the folder planner both work this out here, so they agree.

use std::fmt;

use crate::config::Settings;
use crate::osd::OsdFile;
use crate::telemetry::{Profile, TelemetrySupport};

/// Something the settings ask for and the files cannot give.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Omission {
    /// What is left out: `OSD`, `SRT data` or `motor sound`.
    pub what: &'static str,
    /// Why, in a sentence for the user.
    pub reason: String,
}

impl fmt::Display for Omission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Left out: the {}. {}", self.what, self.reason)
    }
}

/// For each part of the output, why it cannot be made with the files at hand. `None` means it can.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Availability {
    /// Why the OSD cannot be drawn.
    pub osd: Option<String>,
    /// Why the SRT data cannot be drawn.
    pub srt: Option<String>,
    /// Why the sound cannot be made.
    pub sound: Option<String>,
}

impl Availability {
    /// What the files allow: the `.osd` (if there is one), whether there is a font for the OSD, and whether there
    /// is an `.srt`. The sound is judged by the `.osd` and its firmware only; [`Availability::with_telemetry`]
    /// adds what the telemetry itself says.
    #[must_use]
    pub fn new(osd: Option<&OsdFile>, font: bool, srt: bool) -> Self {
        let osd_reason = if osd.is_none() {
            Some("There is no .osd file.".to_owned())
        } else if !font {
            Some("There is no OSD font file.".to_owned())
        } else {
            None
        };
        let sound = match osd {
            None => {
                Some("There is no .osd file: the sound follows the telemetry stored in it.".into())
            }
            Some(osd) => {
                let profile = Profile::from_firmware(&osd.firmware);
                (profile.telemetry_support() != TelemetrySupport::Available)
                    .then(|| profile.explanation().to_owned())
            }
        };
        Self {
            osd: osd_reason,
            srt: (!srt).then(|| "There is no .srt file.".to_owned()),
            sound,
        }
    }

    /// Adds why the telemetry cannot drive the sound, if it cannot (the throttle is not in the recording, say). An
    /// earlier reason is kept.
    #[must_use]
    pub fn with_telemetry(mut self, problem: Option<String>) -> Self {
        if self.sound.is_none() {
            self.sound = problem;
        }
        self
    }

    /// The settings with what the files cannot give turned off.
    #[must_use]
    pub fn restrict(&self, settings: &Settings) -> Settings {
        Settings {
            draw_osd: settings.draw_osd && self.osd.is_none(),
            draw_srt: settings.draw_srt && self.srt.is_none(),
            make_sound: settings.make_sound && self.sound.is_none(),
            ..settings.clone()
        }
    }

    /// What the settings ask for and the files cannot give, in the order OSD, SRT data, sound.
    #[must_use]
    pub fn omissions(&self, settings: &Settings) -> Vec<Omission> {
        [
            (settings.draw_osd, "OSD", &self.osd),
            (settings.draw_srt, "SRT data", &self.srt),
            (settings.make_sound, "motor sound", &self.sound),
        ]
        .into_iter()
        .filter_map(|(wanted, what, reason)| {
            reason.as_ref().filter(|_| wanted).map(|reason| Omission {
                what,
                reason: reason.clone(),
            })
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osd::Firmware;

    fn wanted() -> Settings {
        Settings {
            draw_osd: true,
            draw_srt: true,
            make_sound: true,
            ..Settings::default()
        }
    }

    #[test]
    fn with_no_files_nothing_can_be_made() {
        let availability = Availability::new(None, true, false);
        assert!(availability.osd.is_some());
        assert!(availability.srt.is_some());
        assert!(availability.sound.is_some());
        let restricted = availability.restrict(&wanted());
        assert!(!restricted.draw_osd && !restricted.draw_srt && !restricted.make_sound);
    }

    #[test]
    fn an_srt_alone_leaves_the_srt_data() {
        let availability = Availability::new(None, true, true);
        let restricted = availability.restrict(&wanted());
        assert!(restricted.draw_srt, "the SRT data do not need an .osd");
        assert!(!restricted.draw_osd && !restricted.make_sound);
        let left_out: Vec<&str> = availability
            .omissions(&wanted())
            .iter()
            .map(|o| o.what)
            .collect();
        assert_eq!(left_out, ["OSD", "motor sound"]);
    }

    fn osd_of(firmware: Firmware) -> OsdFile {
        OsdFile {
            firmware,
            columns: 1,
            rows: 1,
            frames: Vec::new(),
            trailing_bytes: 0,
        }
    }

    #[test]
    fn the_osd_needs_a_font_but_the_sound_does_not() {
        let osd = osd_of(Firmware::Inav);
        let availability = Availability::new(Some(&osd), false, false);
        assert!(
            availability
                .osd
                .as_deref()
                .is_some_and(|r| r.contains("font")),
            "{availability:?}"
        );
        assert_eq!(availability.sound, None);
    }

    #[test]
    fn only_the_firmwares_whose_telemetry_is_read_give_a_sound() {
        for firmware in [Firmware::Betaflight, Firmware::ArduPilot, Firmware::Kiss] {
            let osd = osd_of(firmware.clone());
            let availability = Availability::new(Some(&osd), true, true);
            assert_eq!(availability.osd, None, "{firmware}: the OSD can be drawn");
            assert!(availability.sound.is_some(), "{firmware}: no sound");
        }
    }

    #[test]
    fn what_is_not_asked_for_is_not_reported() {
        let availability = Availability::new(None, false, false);
        let settings = Settings {
            draw_osd: false,
            draw_srt: false,
            make_sound: true,
            ..Settings::default()
        };
        let omissions = availability.omissions(&settings);
        assert_eq!(omissions.len(), 1);
        assert_eq!(omissions[0].what, "motor sound");
        assert!(
            omissions[0]
                .to_string()
                .starts_with("Left out: the motor sound.")
        );
    }

    #[test]
    fn the_telemetry_adds_a_reason_but_never_replaces_one() {
        let none = Availability::default().with_telemetry(Some("no throttle".into()));
        assert_eq!(none.sound.as_deref(), Some("no throttle"));
        let kept = Availability {
            sound: Some("no .osd".into()),
            ..Availability::default()
        }
        .with_telemetry(Some("no throttle".into()));
        assert_eq!(kept.sound.as_deref(), Some("no .osd"));
    }
}
