// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Drawing of the OSD, the SRT data and the chroma-key background on video frames.
//!
//! Frames are RGBA pictures. The functions draw on them in place, so one buffer is reused for every frame.

mod osd;
mod srt;
mod text;

pub use osd::draw_osd;
pub use srt::{SRT_TEXT_COLOR, draw_srt, srt_text};
pub use text::draw_text;

use image::{Rgba, RgbaImage};

/// A picture filled with a chroma-key colour, for an overlay that is composited in an editor.
#[must_use]
pub fn chroma_background(width: u32, height: u32, rgb: [u8; 3]) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba([rgb[0], rgb[1], rgb[2], 255]))
}

/// The default chroma-key colour of walksnail-osd-tool: a green that video editors key out easily.
pub const DEFAULT_CHROMA_KEY: [u8; 3] = [1, 177, 64];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chroma_background_is_one_colour() {
        let picture = chroma_background(4, 3, DEFAULT_CHROMA_KEY);
        assert_eq!(picture.dimensions(), (4, 3));
        assert!(picture.pixels().all(|p| p.0 == [1, 177, 64, 255]));
    }
}
