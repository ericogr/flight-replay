// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont, point};
use image::{Rgba, RgbaImage};

/// Draws `text` on `canvas` with its top-left corner at (`x`, `y`), in a font of `px_height` pixels.
///
/// The glyphs are blended over the picture with their coverage, so the text is smooth at its edges.
pub fn draw_text(
    canvas: &mut RgbaImage,
    font: &FontRef<'_>,
    px_height: f32,
    x: f32,
    y: f32,
    text: &str,
    color: Rgba<u8>,
) {
    let scaled = font.as_scaled(PxScale::from(px_height));
    let mut caret = point(x, y + scaled.ascent());
    let mut previous: Option<GlyphId> = None;
    for character in text.chars() {
        let id = scaled.glyph_id(character);
        if let Some(previous) = previous {
            caret.x += scaled.kern(previous, id);
        }
        let glyph = id.with_scale_and_position(scaled.scale(), caret);
        caret.x += scaled.h_advance(id);
        previous = Some(id);

        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i64 + i64::from(gx);
                let py = bounds.min.y as i64 + i64::from(gy);
                blend(canvas, px, py, color, coverage);
            });
        }
    }
}

fn blend(canvas: &mut RgbaImage, x: i64, y: i64, color: Rgba<u8>, coverage: f32) {
    if x < 0 || y < 0 || x >= i64::from(canvas.width()) || y >= i64::from(canvas.height()) {
        return;
    }
    let alpha = coverage.clamp(0.0, 1.0) * f32::from(color[3]) / 255.0;
    if alpha <= 0.0 {
        return;
    }
    let pixel = canvas.get_pixel_mut(x as u32, y as u32);
    for channel in 0..3 {
        let under = f32::from(pixel[channel]);
        let over = f32::from(color[channel]);
        pixel[channel] = (under * (1.0 - alpha) + over * alpha).round() as u8;
    }
    let under_alpha = f32::from(pixel[3]) / 255.0;
    pixel[3] = ((under_alpha + alpha * (1.0 - under_alpha)) * 255.0).round() as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_visible_pixels_inside_the_text_box() {
        let font = FontRef::try_from_slice(crate::assets::SRT_FONT).unwrap();
        let mut canvas = RgbaImage::from_pixel(200, 60, Rgba([0, 0, 0, 255]));
        draw_text(
            &mut canvas,
            &font,
            24.0,
            5.0,
            5.0,
            "Signal:4",
            Rgba([240, 240, 240, 255]),
        );
        let lit = canvas.pixels().filter(|p| p[0] > 100).count();
        assert!(lit > 100, "only {lit} pixels were drawn");
        // Nothing is drawn outside the picture, and the text stays in the box.
        assert!(
            canvas
                .enumerate_pixels()
                .all(|(x, y, p)| p[0] < 100 || (x >= 5 && (5..60).contains(&y)))
        );
    }

    #[test]
    fn coverage_blends_over_the_background() {
        let mut canvas = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        blend(&mut canvas, 0, 0, Rgba([200, 200, 200, 255]), 0.5);
        assert_eq!(canvas.get_pixel(0, 0)[0], 100);
        blend(&mut canvas, 5, 5, Rgba([255, 255, 255, 255]), 1.0);
    }
}
