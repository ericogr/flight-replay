// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/overlay/osd.rs (GPL-3.0-only), Copyright (C) Alexander van Saase.
// Modified for Flight Replay: the glyphs come from a pre-scaled font, and the grid offset is signed.

use image::RgbaImage;
use image::imageops;

use crate::font::ScaledFont;
use crate::osd::{Frame, OsdOptions};

/// Draws the glyphs of `frame` on `canvas`, placed on the grid of `font`'s cell size.
///
/// Cells that are masked in `options` are skipped. The grid is moved by the offsets of `options`.
pub fn draw_osd(canvas: &mut RgbaImage, frame: &Frame, font: &ScaledFont, options: &OsdOptions) {
    let cell_width = i64::from(font.size().width());
    let cell_height = i64::from(font.size().height());
    for (x, y, code) in frame.glyphs() {
        if options.is_masked(x, y) {
            continue;
        }
        let Some(glyph) = font.glyph(code) else {
            continue;
        };
        let left = i64::from(x) * cell_width + i64::from(options.x_offset);
        let top = i64::from(y) * cell_height + i64::from(options.y_offset);
        imageops::overlay(canvas, glyph, left, top);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use image::Rgba;

    use super::*;
    use crate::font::{CharacterSize, FontSheet};

    /// A 512-glyph sheet where glyph `n` is a flat grey of level `n`, so each glyph is recognisable on the canvas.
    fn font() -> ScaledFont {
        let (width, height) = (24u32, 36u32);
        let mut sheet = RgbaImage::new(width, height * 512);
        for code in 0..512u32 {
            let level = code as u8;
            for y in 0..height {
                for x in 0..width {
                    sheet.put_pixel(x, code * height + y, Rgba([level, level, level, 255]));
                }
            }
        }
        FontSheet::from_image(&sheet)
            .unwrap()
            .scaled(CharacterSize::Small)
    }

    fn frame(cells: &[((u32, u32), u16)]) -> Frame {
        let columns = 4;
        let mut data = vec![0u16; 4 * 2];
        for &((x, y), code) in cells {
            data[(y * columns + x) as usize] = code;
        }
        Frame {
            time_ms: 0,
            columns,
            cells: data,
        }
    }

    #[test]
    fn draws_each_glyph_in_its_cell() {
        let mut canvas = RgbaImage::from_pixel(120, 80, Rgba([0, 0, 0, 255]));
        let frame = frame(&[((1, 0), 9), ((2, 1), 30)]);
        draw_osd(&mut canvas, &frame, &font(), &OsdOptions::default());
        assert_eq!(canvas.get_pixel(24, 0)[0], 9);
        assert_eq!(canvas.get_pixel(48 + 5, 36 + 5)[0], 30);
        assert_eq!(canvas.get_pixel(0, 0)[0], 0);
    }

    #[test]
    fn masked_cells_and_offsets_are_applied() {
        let mut canvas = RgbaImage::from_pixel(120, 80, Rgba([0, 0, 0, 255]));
        let frame = frame(&[((0, 0), 9), ((1, 0), 12)]);
        let options = OsdOptions {
            x_offset: 10,
            y_offset: 4,
            masked_cells: BTreeSet::from([(1, 0)]),
        };
        draw_osd(&mut canvas, &frame, &font(), &options);
        assert_eq!(canvas.get_pixel(10 + 5, 4 + 5)[0], 9);
        assert_eq!(
            canvas.get_pixel(24 + 10 + 5, 4 + 5)[0],
            0,
            "the masked cell is not drawn"
        );
    }
}
