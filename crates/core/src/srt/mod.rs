// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Walksnail SRT files: the flight data the goggles record next to the video.

mod data;
mod file;
mod options;

pub use data::SrtData;
pub use file::{SrtCue, SrtError, SrtFile};
pub use options::SrtOptions;
