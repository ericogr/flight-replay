// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Rendering a video: the OSD and the SRT data drawn on the picture, the synthetic sound, or both.
//!
//! A [`RenderPlan`] says what to do. [`render`] does it, reporting progress and stopping when asked. The module has no
//! user interface: the application, or a command-line tool, only builds a plan and calls it.

mod events;
mod plan;
mod run;

pub use events::{CancelToken, Event, Stage};
pub use plan::{
    EncoderChoice, Mode, OsdLayer, OutputPlan, Overwrite, RenderPlan, ResolvedWindow, SrtLayer,
    VideoOutput, Window,
};
pub use run::{Outcome, OutputCheck, RenderError, command_preview, render};
