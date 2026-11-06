// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/overlay/srt.rs (GPL-3.0-only), Copyright (C) Alexander van Saase.
// Modified for Flight Replay: the text is built by a function that can be tested, and the font is passed in.

use std::fmt::Write as _;

use ab_glyph::FontRef;
use image::{Rgba, RgbaImage};

use super::text::draw_text;
use crate::srt::{SrtData, SrtOptions};

/// Colour of the SRT text.
pub const SRT_TEXT_COLOR: Rgba<u8> = Rgba([240, 240, 240, 255]);

/// The text of the enabled SRT fields, with two spaces between fields, as drawn on the picture.
#[must_use]
pub fn srt_text(data: &SrtData, options: &SrtOptions) -> String {
    let mut text = String::new();
    if options.show_time {
        let minutes = data.flight_time_s / 60;
        let seconds = data.flight_time_s % 60;
        let _ = write!(text, "Time:{minutes}:{seconds:0>2}  ");
    }
    if options.show_sky_battery {
        let _ = write!(text, "SBat:{:>4.1}V  ", data.sky_battery_v);
    }
    if options.show_ground_battery {
        let _ = write!(text, "GBat:{:>4.1}V  ", data.ground_battery_v);
    }
    if options.show_signal {
        let _ = write!(text, "Signal:{}  ", data.signal);
    }
    if options.show_latency {
        let _ = write!(text, "Latency:{:>3}ms  ", data.latency_ms);
    }
    if options.show_bitrate {
        let _ = write!(text, "Bitrate:{:>4.1}Mbps  ", data.bitrate_mbps);
    }
    if options.show_distance {
        if data.distance_m > 999 {
            let _ = write!(
                text,
                "Distance:{:.2}km",
                f64::from(data.distance_m) / 1000.0
            );
        } else {
            let _ = write!(text, "Distance:{:>3}m", data.distance_m);
        }
    }
    text
}

/// Draws the enabled SRT fields on `canvas`.
///
/// The position and the text height follow the options: the position is a percentage of the picture, and the
/// height is `scale` pixels for a 1080p picture.
pub fn draw_srt(canvas: &mut RgbaImage, data: &SrtData, font: &FontRef<'_>, options: &SrtOptions) {
    let text = srt_text(data, options);
    if text.is_empty() {
        return;
    }
    let (width, height) = canvas.dimensions();
    let x = options.position_percent.0 / 100.0 * width as f32;
    let y = options.position_percent.1 / 100.0 * height as f32;
    let px_height = options.scale / 1080.0 * height as f32;
    draw_text(canvas, font, px_height, x, y, &text, SRT_TEXT_COLOR);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> SrtData {
        SrtData {
            signal: 4,
            channel: 7,
            flight_time_s: 125,
            sky_battery_v: 15.9,
            ground_battery_v: 7.8,
            latency_ms: 37,
            bitrate_mbps: 25.0,
            distance_m: 2,
        }
    }

    #[test]
    fn default_fields_match_the_recording_layout() {
        let text = srt_text(&data(), &SrtOptions::default());
        assert_eq!(
            text,
            "Signal:4  Latency: 37ms  Bitrate:25.0Mbps  Distance:  2m"
        );
    }

    #[test]
    fn all_fields_and_long_distances() {
        let options = SrtOptions {
            show_time: true,
            show_sky_battery: true,
            show_ground_battery: true,
            ..SrtOptions::default()
        };
        let mut far = data();
        far.distance_m = 1234;
        assert_eq!(
            srt_text(&far, &options),
            "Time:2:05  SBat:15.9V  GBat: 7.8V  Signal:4  Latency: 37ms  Bitrate:25.0Mbps  Distance:1.23km"
        );
    }

    #[test]
    fn nothing_enabled_gives_no_text() {
        let options = SrtOptions {
            show_signal: false,
            show_latency: false,
            show_bitrate: false,
            show_distance: false,
            ..SrtOptions::default()
        };
        assert_eq!(srt_text(&data(), &options), "");
    }
}
