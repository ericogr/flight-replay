// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/pipeline/signals.go (Apache-2.0, Copyright 2026 EricoGR).

//! Which telemetry the sound uses, what the recording lacks, and what each missing signal costs the sound.

use std::fmt::Write as _;

use super::engine::Controls;
use super::settings::SoundSettings;
use crate::telemetry::{Series, Table, format_value};

/// The roles telemetry plays in the sound model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Drives the pitch. Required.
    Throttle,
    /// Drives the volume and the propeller noise.
    Current,
    /// Drives the wind.
    AirSpeed,
    /// Drives the wind when there is no air speed.
    GpsSpeed,
    /// Corrects the RPM by the battery voltage.
    CellVoltage,
    /// Optional secondary effect.
    GForce,
    /// Stereo balance of the wind.
    Roll,
    /// Slight load variation.
    Pitch,
}

impl Role {
    /// The name of the role, as shown to the user.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Throttle => "throttle",
            Self::Current => "current",
            Self::AirSpeed => "air_speed",
            Self::GpsSpeed => "gps_speed",
            Self::CellVoltage => "cell_voltage",
            Self::GForce => "gforce",
            Self::Roll => "roll",
            Self::Pitch => "pitch",
        }
    }

    /// The element of the INAV OSD that shows the signal, named as the INAV Configurator names it (OSD tab). The
    /// user turns it on there, and records again.
    #[must_use]
    pub fn osd_element(self) -> &'static str {
        match self {
            Self::Throttle => "Throttle Position",
            Self::Current => "Current Draw",
            Self::AirSpeed => "Air Speed",
            Self::GpsSpeed => "Ground Speed",
            Self::CellVoltage => "Battery Cell Voltage",
            Self::GForce => "g force",
            Self::Roll => "Roll Angle",
            Self::Pitch => "Pitch Angle",
        }
    }
}

/// The share of OSD frames in which a signal must have a reading to be trusted, in percent. An element that is
/// hidden or blinks most of the time leaves the sound following a value that is mostly carried over.
pub const MIN_COVERAGE_PCT: f64 = 50.0;

/// How long, in seconds, the throttle is held after its last reading. Once the OSD stops showing it the motor is off
/// (disarmed, or the recording has ended), so the motor then stops instead of running at the last value for the rest
/// of the video. Before the first reading the motor is off too (see [`Series::zero_before_first_reading`]).
pub const MOTOR_HOLD_S: f64 = 2.0;

/// A telemetry column the sound model can use, looked up in the recording.
#[derive(Debug, Clone)]
pub struct Signal {
    /// What the signal is used for.
    pub role: Role,
    /// The column that fills the role.
    pub column: String,
    /// The readings, or `None` when the recording has none.
    pub series: Option<Series>,
}

impl Signal {
    /// Whether the recording has readings of the signal.
    #[must_use]
    pub fn is_present(&self) -> bool {
        self.series.is_some()
    }
}

/// Errors when looking for the signals.
#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    /// The recording has no throttle readings, so there is nothing to generate.
    #[error(
        "{source_name} shows no throttle (column {column}) in any of its {frames} frames, and the motor pitch follows the throttle. In the INAV Configurator, on the OSD tab, turn on \"{element}\" and record again, or name the column to use in the sound settings"
    )]
    NoThrottle {
        /// The recording.
        source_name: String,
        /// The column that was looked for.
        column: String,
        /// Number of OSD frames.
        frames: usize,
        /// The element of the INAV OSD that shows the throttle.
        element: &'static str,
    },
}

/// The signals of one recording, with the warnings about what is missing.
#[derive(Debug, Clone)]
pub struct SignalSet {
    /// The signals that are on in the settings, in the order of the roles.
    pub signals: Vec<Signal>,
    /// What the user should know: signals that are rare, absent or useless, and what that costs the sound.
    pub warnings: Vec<String>,
    /// The elements of the INAV OSD to turn on to get the signals that are missing (see [`Role::osd_element`]).
    pub turn_on: Vec<&'static str>,
    source_name: String,
}

impl SignalSet {
    /// Looks up in the telemetry every signal the sound model can use, and tells what is missing and what that costs
    /// the sound. Only the throttle is mandatory: without it there is no motor pitch to generate. Signals that the
    /// settings do not use (a gain of 0, say) are not reported.
    ///
    /// `source_name` is the file name of the `.osd`, for the messages, and `frames` its number of OSD frames.
    pub fn collect(
        table: &Table,
        settings: &SoundSettings,
        source_name: &str,
        frames: usize,
    ) -> Result<Self, SignalError> {
        let c = &settings.columns;
        let mut signals = Vec::new();
        for (role, column) in [
            (Role::Throttle, &c.throttle),
            (Role::Current, &c.current),
            (Role::AirSpeed, &c.air_speed),
            (Role::GpsSpeed, &c.gps_speed),
            (Role::CellVoltage, &c.cell_voltage),
            (Role::GForce, &c.gforce),
            (Role::Roll, &c.roll),
            (Role::Pitch, &c.pitch),
        ] {
            if column.is_empty() {
                continue; // feature turned off in the settings
            }
            let mut series = table.series(column).ok(); // an error means no readings
            if role == Role::Throttle
                && let Some(series) = series.as_mut()
            {
                series.zero_before_first_reading();
                series.zero_after_last_reading(MOTOR_HOLD_S);
            }
            signals.push(Signal {
                role,
                column: column.clone(),
                series,
            });
        }

        let mut set = Self {
            signals,
            warnings: Vec::new(),
            turn_on: Vec::new(),
            source_name: source_name.to_owned(),
        };
        if set.lacks(Role::Throttle) {
            return Err(SignalError::NoThrottle {
                source_name: source_name.to_owned(),
                column: set.column(Role::Throttle),
                frames,
                element: Role::Throttle.osd_element(),
            });
        }
        set.warn_sparse();
        set.warn_absent(settings, table);
        if let Some(throttle) = set.series(Role::Throttle)
            && throttle.stats.max <= settings.motor.off_threshold_pct
        {
            let message = format!(
                "the throttle never goes above {:.0}% (motor.off_threshold_pct, highest reading {}): the motor will be silent for the whole video",
                settings.motor.off_threshold_pct,
                format_value(throttle.stats.max)
            );
            set.warnings.push(message);
        }
        Ok(set)
    }

    /// The series that drive the engine. The throttle is always present in a set that [`SignalSet::collect`]
    /// returned.
    #[must_use]
    pub fn controls(&self) -> Controls<'_> {
        Controls {
            throttle: self
                .series(Role::Throttle)
                .expect("collect checked the throttle"),
            current: self.series(Role::Current),
            air_speed: self.series(Role::AirSpeed),
            gps_speed: self.series(Role::GpsSpeed),
            cell_voltage: self.series(Role::CellVoltage),
            gforce: self.series(Role::GForce),
            roll: self.series(Role::Roll),
            pitch: self.series(Role::Pitch),
        }
    }

    /// The signals the recording has and the ones it lacks, in one line.
    #[must_use]
    pub fn describe(&self) -> String {
        let found: Vec<&str> = self
            .signals
            .iter()
            .filter(|s| s.is_present())
            .map(|s| s.role.name())
            .collect();
        let absent: Vec<&str> = self
            .signals
            .iter()
            .filter(|s| !s.is_present())
            .map(|s| s.role.name())
            .collect();
        let mut text = format!("found {}", found.join(", "));
        if !absent.is_empty() {
            let _ = write!(text, "; not in the .osd: {}", absent.join(", "));
        }
        text
    }

    fn signal(&self, role: Role) -> Option<&Signal> {
        self.signals.iter().find(|s| s.role == role)
    }

    fn series(&self, role: Role) -> Option<&Series> {
        self.signal(role).and_then(|s| s.series.as_ref())
    }

    fn column(&self, role: Role) -> String {
        self.signal(role)
            .map(|s| s.column.clone())
            .unwrap_or_default()
    }

    /// Whether the role is on in the settings but the recording has no readings of it.
    fn lacks(&self, role: Role) -> bool {
        self.signal(role).is_some_and(|s| !s.is_present())
    }

    /// Flags the signals that show up in too few OSD frames.
    fn warn_sparse(&mut self) {
        let mut messages = Vec::new();
        for signal in &self.signals {
            let Some(series) = &signal.series else {
                continue;
            };
            let percent = series.stats.coverage_pct();
            if percent < MIN_COVERAGE_PCT {
                messages.push(format!(
                    "only {percent:.0}% of the OSD frames show {} ({}); the rest repeat the last reading. Is the element hidden or blinking?",
                    signal.column,
                    signal.role.name()
                ));
            }
        }
        self.warnings.extend(messages);
    }

    /// Explains, for each signal the recording lacks, what the sound loses.
    fn warn_absent(&mut self, settings: &SoundSettings, table: &Table) {
        self.note_lost(
            Role::Current,
            "current",
            "the volume and the propeller noise will follow the throttle instead",
        );

        // The wind follows the air speed, or the ground speed when there is none.
        let air_present = self.signal(Role::AirSpeed).is_some_and(Signal::is_present);
        let gps_column = self
            .signal(Role::GpsSpeed)
            .filter(|g| g.is_present())
            .map(|g| g.column.clone());
        if air_present {
            // nothing to say
        } else if let Some(gps_column) = gps_column {
            let effect = format!("the wind will follow the ground speed ({gps_column}) instead");
            // The ground speed stands in, so there is no need to ask for an air speed (it takes a pitot tube).
            self.warn_lost(Role::AirSpeed, "airspeed", &effect);
        } else {
            let columns: Vec<String> = [Role::AirSpeed, Role::GpsSpeed]
                .into_iter()
                .filter(|&role| self.lacks(role))
                .map(|role| self.column(role))
                .collect();
            if !columns.is_empty() {
                let message = format!(
                    "no airspeed or ground speed readings in {} ({}): the wind sound will be off",
                    self.source_name,
                    columns.join(", ")
                );
                self.warnings.push(message);
                for role in [Role::GpsSpeed, Role::AirSpeed] {
                    if self.lacks(role) {
                        self.turn_on.push(role.osd_element());
                    }
                }
            }
        }

        if settings.motor.battery_gain != 0.0 {
            let mut effect = "the motor pitch will not follow the battery voltage".to_owned();
            if table.has_column("osd_batt_v") {
                effect.push_str(
                    " (the OSD only shows the voltage of the whole pack, osd_batt_v, and the model needs it per cell)",
                );
            }
            self.note_lost(Role::CellVoltage, "per-cell battery voltage", &effect);
        }
        if settings.wind.roll_balance != 0.0 && settings.audio.channels == 2 {
            self.note_lost(
                Role::Roll,
                "roll",
                "the wind will not pan between left and right with the bank angle",
            );
        }
        if settings.loudness.pitch_gain != 0.0 {
            self.note_lost(
                Role::Pitch,
                "pitch",
                "the load will not follow the climb angle",
            );
        }
        if settings.gforce.gain != 0.0 {
            self.note_lost(Role::GForce, "G-force", "gforce.gain will have no effect");
        }
    }

    /// Adds a warning if the role is on in the settings but the recording has no readings of it, and the element of
    /// the OSD that would give them.
    fn note_lost(&mut self, role: Role, what: &str, effect: &str) {
        if self.lacks(role) {
            self.warn_lost(role, what, effect);
            self.turn_on.push(role.osd_element());
        }
    }

    /// Adds a warning if the role is on in the settings but the recording has no readings of it.
    fn warn_lost(&mut self, role: Role, what: &str, effect: &str) {
        if self.lacks(role) {
            let message = format!(
                "no {what} readings in {} ({}): {effect}",
                self.source_name,
                self.column(role)
            );
            self.warnings.push(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::Row;

    /// A table of ten rows with only the columns named, all with a reading in every row.
    fn table(columns: &[&str]) -> Table {
        let rows = (0..10)
            .map(|i| {
                let mut row = Row {
                    time: f64::from(i),
                    ..Row::default()
                };
                for column in columns {
                    row.numbers
                        .insert((*column).to_owned(), 50.0 + f64::from(i));
                }
                row
            })
            .collect();
        Table::new(rows).unwrap()
    }

    fn collect(columns: &[&str]) -> Result<SignalSet, SignalError> {
        SignalSet::collect(&table(columns), &SoundSettings::default(), "a.osd", 10)
    }

    #[test]
    fn a_recording_without_the_throttle_says_which_element_to_turn_on() {
        let error = collect(&["osd_current_a"]).unwrap_err().to_string();
        assert!(error.contains("\"Throttle Position\""), "{error}");
        assert!(error.contains("OSD tab"), "{error}");
    }

    #[test]
    fn a_recording_with_everything_asks_for_nothing() {
        let all = [
            "osd_throttle_pct",
            "osd_current_a",
            "osd_air_speed_kmh",
            "osd_gps_speed_kmh",
            "osd_batt_cell_v",
            "osd_gforce",
            "osd_roll_deg",
            "osd_pitch_deg",
        ];
        let set = collect(&all).unwrap();
        assert!(set.turn_on.is_empty(), "{:?}", set.turn_on);
    }

    #[test]
    fn each_missing_signal_names_the_element_that_gives_it() {
        let set = collect(&["osd_throttle_pct", "osd_gps_speed_kmh"]).unwrap();
        assert_eq!(
            set.turn_on,
            [
                "Current Draw",
                "Battery Cell Voltage",
                "Roll Angle",
                "Pitch Angle"
            ],
            "the ground speed stands in for the air speed, so that is not asked for"
        );
    }

    #[test]
    fn without_any_speed_both_speeds_are_asked_for() {
        let set = collect(&["osd_throttle_pct"]).unwrap();
        assert!(
            set.turn_on.contains(&"Ground Speed") && set.turn_on.contains(&"Air Speed"),
            "{:?}",
            set.turn_on
        );
    }

    #[test]
    fn a_signal_the_settings_turn_off_is_not_asked_for() {
        let mut settings = SoundSettings::default();
        settings.columns.current.clear();
        let set =
            SignalSet::collect(&table(&["osd_throttle_pct"]), &settings, "a.osd", 10).unwrap();
        assert!(!set.turn_on.contains(&"Current Draw"), "{:?}", set.turn_on);
    }
}
