// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The `.osd` files that Walksnail goggles record next to each video.
//!
//! An `.osd` file does not store numbers. It stores the screen the pilot saw, frame by frame, as a grid of
//! glyph codes. The glyphs are drawn with a font sheet (see [`crate::font`]), and the telemetry is read from
//! them (see [`crate::telemetry`]).

mod file;
mod firmware;
mod join;
mod options;

pub use file::{Frame, HEADER_BYTES, OsdError, OsdFile, SPACE};
pub use firmware::Firmware;
pub use join::{Coverage, JoinError, JoinPart, Joined, join};
pub use options::OsdOptions;
