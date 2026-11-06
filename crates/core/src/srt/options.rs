// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/srt/options.rs (GPL-3.0-only), Copyright (C) Alexander van Saase.

use serde::{Deserialize, Serialize};

/// Which SRT fields are drawn, and where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag switches one SRT field on or off"
)]
pub struct SrtOptions {
    /// Position of the text, as percent of the picture width (x) and height (y).
    pub position_percent: (f32, f32),
    /// Text height, in pixels for a 1080p picture. It scales with the picture height.
    pub scale: f32,
    /// Flight time.
    pub show_time: bool,
    /// Aircraft battery voltage.
    pub show_sky_battery: bool,
    /// Goggles battery voltage.
    pub show_ground_battery: bool,
    /// Signal level.
    pub show_signal: bool,
    /// Video latency.
    pub show_latency: bool,
    /// Video bitrate.
    pub show_bitrate: bool,
    /// Distance from the goggles.
    pub show_distance: bool,
}

impl Default for SrtOptions {
    fn default() -> Self {
        Self {
            position_percent: (1.5, 95.0),
            scale: 35.0,
            show_time: false,
            show_sky_battery: false,
            show_ground_battery: false,
            show_signal: true,
            show_latency: true,
            show_bitrate: true,
            show_distance: true,
        }
    }
}
