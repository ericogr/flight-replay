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

    /// Whether the step shows the preview picture.
    #[must_use]
    pub fn has_preview(self) -> bool {
        matches!(self, Self::Overlay | Self::Srt | Self::Sound)
    }

    /// What the step wants on its preview picture. The step of the OSD wants the OSD, and the step of the SRT data
    /// wants the SRT data; the step of the sound has no layer of its own. A layer of another step is wanted only
    /// when `show_other_layers` is on. The grid of the OSD cells, which the masks are chosen on, belongs to the step
    /// of the OSD.
    #[must_use]
    pub fn preview_layers(self, show_other_layers: bool) -> PreviewLayers {
        PreviewLayers {
            osd: show_other_layers || self == Self::Overlay,
            srt: show_other_layers || self == Self::Srt,
            grid: self == Self::Overlay,
        }
    }

    /// What the "also show" option of the preview adds in this step, as the option names it.
    #[must_use]
    pub fn other_layers_name(self) -> &'static str {
        match self {
            Self::Overlay => "SRT data",
            Self::Srt => "OSD",
            _ => "SRT and OSD data",
        }
    }
}

/// What is drawn on a preview picture: the OSD, the SRT data, and the grid of the OSD cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "a plain list of what is drawn, one yes-or-no for each layer"
)]
pub struct PreviewLayers {
    pub osd: bool,
    pub srt: bool,
    pub grid: bool,
}

/// What the wizard needs to know about the state of the application to decide where the user can go.
///
/// The three things that can be added are what will really be added: what the user asked for, less what the files
/// cannot give (see `flight_replay_core::render::Availability`). So a video with no `.osd` file never holds the user
/// at a step, it only renders without the OSD and the sound.
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
    /// The OSD will be drawn.
    pub draw_osd: bool,
    /// The SRT data will be drawn.
    pub draw_srt: bool,
    /// The sound will be made.
    pub make_sound: bool,
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
                    "FFmpeg is missing or too old. Open Settings to install it, or to choose its folder."
                        .to_owned(),
                );
            }
        }
        Step::Input => {
            if !ctx.video {
                problems.push("Open a video.".to_owned());
            }
        }
        // These steps only choose how to add what the files allow: they never hold the user back.
        Step::Overlay | Step::Srt | Step::Sound => {}
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
                    "There is nothing to add: turn on the OSD, the SRT data or the sound, and give it the files it needs."
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
            draw_osd: true,
            draw_srt: true,
            make_sound: true,
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
    fn only_three_steps_have_a_preview() {
        let with: Vec<Step> = Step::ALL
            .into_iter()
            .filter(|step| step.has_preview())
            .collect();
        assert_eq!(with, [Step::Overlay, Step::Srt, Step::Sound]);
    }

    #[test]
    fn a_step_wants_its_own_layer_and_the_others_only_when_asked() {
        let layers = |osd, srt, grid| PreviewLayers { osd, srt, grid };
        // Every layer is wanted while the option is on, and the grid is the OSD step's.
        assert_eq!(Step::Overlay.preview_layers(true), layers(true, true, true));
        assert_eq!(Step::Srt.preview_layers(true), layers(true, true, false));
        assert_eq!(Step::Sound.preview_layers(true), layers(true, true, false));
        // With it off each step keeps only its own, and the step of the sound has none.
        assert_eq!(
            Step::Overlay.preview_layers(false),
            layers(true, false, true)
        );
        assert_eq!(Step::Srt.preview_layers(false), layers(false, true, false));
        assert_eq!(
            Step::Sound.preview_layers(false),
            layers(false, false, false)
        );
    }

    #[test]
    fn the_option_names_what_it_adds() {
        assert_eq!(Step::Overlay.other_layers_name(), "SRT data");
        assert_eq!(Step::Srt.other_layers_name(), "OSD");
        assert_eq!(Step::Sound.other_layers_name(), "SRT and OSD data");
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
    }

    #[test]
    fn the_steps_that_choose_what_to_add_never_hold_the_user_back() {
        // Even when nothing is to be added: that is said at the end, where the render starts.
        let nothing = Context {
            tools_found: true,
            video: true,
            output_path: true,
            ..Context::default()
        };
        for step in [Step::Overlay, Step::Srt, Step::Sound] {
            assert_eq!(problems(step, &nothing), Vec::<String>::new(), "{step:?}");
        }
        assert!(can_go_to(Step::Input, Step::Render, &nothing));
        assert!(problems(Step::Render, &nothing)[0].contains("nothing to add"));
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
