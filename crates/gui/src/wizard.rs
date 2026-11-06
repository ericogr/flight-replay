// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The steps of the wizard and the rules for moving between them.
//!
//! This is plain Rust with no user interface in it, so the rules can be tested. The views only show what these
//! functions say.

/// A step of the wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Step {
    /// The environment: FFmpeg, encoders and the colour scheme.
    Welcome,
    /// The video and the files that go with it.
    Input,
    /// The OSD drawn on the picture.
    Overlay,
    /// The SRT data drawn on the picture.
    Srt,
    /// The synthetic motor sound.
    Sound,
    /// How the output is encoded and where it goes.
    Output,
    /// The summary, the render and the result.
    Render,
}

impl Step {
    /// Every step, in order.
    pub const ALL: [Step; 7] = [
        Step::Welcome,
        Step::Input,
        Step::Overlay,
        Step::Srt,
        Step::Sound,
        Step::Output,
        Step::Render,
    ];

    /// The name of the step.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Welcome => "Welcome",
            Self::Input => "Input",
            Self::Overlay => "OSD",
            Self::Srt => "SRT data",
            Self::Sound => "Motor sound",
            Self::Output => "Output",
            Self::Render => "Render",
        }
    }

    /// The position of the step, from 1.
    #[must_use]
    pub fn number(self) -> usize {
        Self::ALL
            .iter()
            .position(|&s| s == self)
            .map_or(1, |i| i + 1)
    }

    /// The step after this one.
    #[must_use]
    pub fn next(self) -> Option<Step> {
        Self::ALL.get(self.number()).copied()
    }

    /// The step before this one.
    #[must_use]
    pub fn previous(self) -> Option<Step> {
        self.number()
            .checked_sub(2)
            .and_then(|i| Self::ALL.get(i))
            .copied()
    }
}

/// What the wizard needs to know about the state of the application to decide where the user can go.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "a plain list of yes-or-no facts about the application"
)]
pub struct Context {
    /// FFmpeg and ffprobe were found.
    pub tools_found: bool,
    /// A video is open.
    pub video: bool,
    /// An `.osd` file is open.
    pub osd: bool,
    /// An OSD font file is open.
    pub font: bool,
    /// An `.srt` file is open.
    pub srt: bool,
    /// The OSD is to be drawn.
    pub draw_osd: bool,
    /// The SRT data is to be drawn.
    pub draw_srt: bool,
    /// The sound is to be made.
    pub make_sound: bool,
    /// Why the firmware of the `.osd` has no sound, if it has none.
    pub sound_unavailable: Option<String>,
    /// Why the telemetry cannot drive the sound, if it cannot.
    pub signals_error: Option<String>,
    /// The output file is chosen.
    pub output_path: bool,
    /// A video encoder is chosen.
    pub encoder: bool,
}

impl Context {
    /// Whether anything is drawn on the picture.
    #[must_use]
    pub fn draws(&self) -> bool {
        self.draw_osd || self.draw_srt
    }
}

/// What stops the user from leaving `step`. An empty list means the step is complete.
#[must_use]
pub fn problems(step: Step, ctx: &Context) -> Vec<String> {
    let mut problems = Vec::new();
    match step {
        Step::Welcome => {
            if !ctx.tools_found {
                problems.push(
                    "FFmpeg was not found. Install it, or choose the folder where it is."
                        .to_owned(),
                );
            }
        }
        Step::Input => {
            if !ctx.video {
                problems.push("Open a video.".to_owned());
            }
        }
        Step::Overlay => {
            if ctx.draw_osd && !ctx.osd {
                problems.push("The OSD cannot be drawn without an .osd file. Choose one, or turn the OSD off.".to_owned());
            }
            if ctx.draw_osd && !ctx.font {
                problems.push("Choose the OSD font file, or turn the OSD off.".to_owned());
            }
        }
        Step::Srt => {
            if ctx.draw_srt && !ctx.srt {
                problems.push("The SRT data cannot be drawn without an .srt file. Choose one, or turn it off.".to_owned());
            }
        }
        Step::Sound => {
            if ctx.make_sound && !ctx.osd {
                problems.push("The sound follows the telemetry in the .osd file. Choose one, or turn the sound off.".to_owned());
            } else if ctx.make_sound
                && let Some(reason) = &ctx.sound_unavailable
            {
                problems.push(reason.clone());
            } else if ctx.make_sound
                && let Some(reason) = &ctx.signals_error
            {
                problems.push(reason.clone());
            }
        }
        Step::Output => {
            if !ctx.output_path {
                problems.push("Choose where to save the video.".to_owned());
            }
            if ctx.draws() && !ctx.encoder {
                problems.push("Choose a video encoder.".to_owned());
            }
        }
        Step::Render => {
            if !ctx.draws() && !ctx.make_sound {
                problems.push(
                    "Turn on the OSD, the SRT data or the sound: there is nothing to add."
                        .to_owned(),
                );
            }
        }
    }
    problems
}

/// Whether the user can go to `target` from the step they are on: backwards always, and forwards only when every
/// step before the target is complete.
#[must_use]
pub fn can_go_to(from: Step, target: Step, ctx: &Context) -> bool {
    if target <= from {
        return true;
    }
    Step::ALL
        .iter()
        .take_while(|&&step| step < target)
        .all(|&step| problems(step, ctx).is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Context {
        Context {
            tools_found: true,
            video: true,
            osd: true,
            font: true,
            srt: true,
            draw_osd: true,
            draw_srt: true,
            make_sound: true,
            sound_unavailable: None,
            signals_error: None,
            output_path: true,
            encoder: true,
        }
    }

    #[test]
    fn steps_are_in_order_and_know_their_neighbours() {
        assert_eq!(Step::Welcome.previous(), None);
        assert_eq!(Step::Welcome.next(), Some(Step::Input));
        assert_eq!(Step::Sound.number(), 5);
        assert_eq!(Step::Render.next(), None);
        assert_eq!(Step::Render.previous(), Some(Step::Output));
        assert_eq!(Step::ALL.len(), 7);
    }

    #[test]
    fn a_complete_context_has_no_problems() {
        for step in Step::ALL {
            assert_eq!(problems(step, &ready()), Vec::<String>::new(), "{step:?}");
        }
    }

    #[test]
    fn each_step_names_what_is_missing() {
        let none = Context::default();
        assert!(problems(Step::Welcome, &none)[0].contains("FFmpeg"));
        assert!(problems(Step::Input, &none)[0].contains("video"));
        assert!(problems(Step::Output, &none)[0].contains("save"));

        let mut ctx = ready();
        ctx.font = false;
        assert!(problems(Step::Overlay, &ctx)[0].contains("font"));
        ctx.draw_osd = false;
        assert!(
            problems(Step::Overlay, &ctx).is_empty(),
            "turning the OSD off clears it"
        );
    }

    #[test]
    fn the_sound_needs_the_osd_a_supported_firmware_and_the_signals() {
        let mut ctx = ready();
        ctx.sound_unavailable = Some("Betaflight: no sound yet.".into());
        assert_eq!(
            problems(Step::Sound, &ctx),
            vec!["Betaflight: no sound yet.".to_owned()]
        );
        ctx.sound_unavailable = None;
        ctx.signals_error = Some("no throttle readings".into());
        assert_eq!(
            problems(Step::Sound, &ctx),
            vec!["no throttle readings".to_owned()]
        );
        ctx.make_sound = false;
        assert_eq!(problems(Step::Sound, &ctx), Vec::<String>::new());
        ctx.make_sound = true;
        ctx.osd = false;
        assert!(problems(Step::Sound, &ctx)[0].contains(".osd"));
    }

    #[test]
    fn rendering_needs_something_to_add() {
        let mut ctx = ready();
        ctx.draw_osd = false;
        ctx.draw_srt = false;
        ctx.make_sound = false;
        assert!(problems(Step::Render, &ctx)[0].contains("nothing to add"));
        ctx.make_sound = true;
        assert_eq!(problems(Step::Render, &ctx), Vec::<String>::new());
    }

    #[test]
    fn an_encoder_is_only_needed_when_the_picture_is_drawn() {
        let mut ctx = ready();
        ctx.encoder = false;
        assert!(problems(Step::Output, &ctx)[0].contains("encoder"));
        ctx.draw_osd = false;
        ctx.draw_srt = false;
        assert!(
            problems(Step::Output, &ctx).is_empty(),
            "the video is copied"
        );
    }

    #[test]
    fn going_back_is_always_allowed_and_going_forward_needs_the_steps_before() {
        let mut ctx = ready();
        assert!(can_go_to(Step::Welcome, Step::Render, &ctx));
        ctx.video = false;
        assert!(!can_go_to(Step::Input, Step::Overlay, &ctx));
        assert!(!can_go_to(Step::Welcome, Step::Output, &ctx));
        assert!(can_go_to(Step::Output, Step::Welcome, &ctx));
        assert!(can_go_to(Step::Input, Step::Input, &ctx));
        ctx.video = true;
        ctx.tools_found = false;
        assert!(!can_go_to(Step::Welcome, Step::Input, &ctx));
    }
}
