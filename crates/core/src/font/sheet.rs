// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/font/font_file.rs (GPL-3.0-only), Copyright (C) Alexander van
// Saase. Modified for Flight Replay: glyphs are resized once per render size and not once per drawn glyph.

use std::path::Path;

use image::RgbaImage;
use image::imageops::{self, FilterType};

use super::layout::{self, CharacterSize, FontLayout, LayoutError};

/// Errors when reading a font sheet.
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    /// The image file could not be opened.
    #[error("could not open the font image: {0}")]
    Io(#[from] std::io::Error),
    /// The image could not be decoded.
    #[error("could not decode the font image: {0}")]
    Image(#[from] image::ImageError),
    /// The image has a size that is not a font sheet.
    #[error(transparent)]
    Layout(#[from] LayoutError),
}

/// A font sheet: one image holding the glyphs of an OSD font, as the goggles store it.
#[derive(Debug, Clone)]
pub struct FontSheet {
    layout: FontLayout,
    glyphs: Vec<RgbaImage>,
}

impl FontSheet {
    /// Reads a PNG font sheet from `path`.
    pub fn open(path: &Path) -> Result<Self, FontError> {
        let image = image::ImageReader::open(path)?.decode()?.into_rgba8();
        Self::from_image(&image)
    }

    /// Reads a PNG font sheet held in memory, such as one compiled into the program.
    pub fn from_png(bytes: &[u8]) -> Result<Self, FontError> {
        let image =
            image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?.into_rgba8();
        Self::from_image(&image)
    }

    /// Splits a font sheet image into glyphs.
    ///
    /// The glyphs are stored page by page: glyph `page * characters_per_page + character` is the character
    /// `character` of colour page `page`.
    pub fn from_image(image: &RgbaImage) -> Result<Self, FontError> {
        let layout = layout::detect(image.width(), image.height())?;
        let (width, height) = (layout.size.width(), layout.size.height());
        let mut glyphs = Vec::with_capacity(layout.glyph_count() as usize);
        for page in 0..layout.pages {
            for character in 0..layout.characters_per_page {
                let glyph =
                    imageops::crop_imm(image, page * width, character * height, width, height)
                        .to_image();
                glyphs.push(glyph);
            }
        }
        Ok(Self { layout, glyphs })
    }

    /// The layout the sheet was detected with.
    #[must_use]
    pub fn layout(&self) -> FontLayout {
        self.layout
    }

    /// Returns every glyph resized to `size`, ready to draw on a picture of that height.
    ///
    /// Glyphs keep their native size when it already matches. Resizing is done once here, not for every drawn
    /// glyph.
    #[must_use]
    pub fn scaled(&self, size: CharacterSize) -> ScaledFont {
        let native = self.layout.size;
        let glyphs = if native == size {
            self.glyphs.clone()
        } else {
            self.glyphs
                .iter()
                .map(|glyph| {
                    imageops::resize(glyph, size.width(), size.height(), FilterType::Lanczos3)
                })
                .collect()
        };
        ScaledFont {
            size,
            pitch: (f64::from(size.width()), f64::from(size.height())),
            glyphs,
        }
    }
}

/// The glyphs of a font sheet at one render size.
#[derive(Debug, Clone)]
pub struct ScaledFont {
    size: CharacterSize,
    /// How far apart the cells of the grid are, in pixels. It is the size of the cell, unless the font was made
    /// smaller with [`ScaledFont::resized`], where it keeps the proportion of the video exactly.
    pitch: (f64, f64),
    glyphs: Vec<RgbaImage>,
}

impl ScaledFont {
    /// Pixels of a glyph picture for a cell this far from the next.
    fn pixels(pitch: f64) -> u32 {
        (pitch.round() as u32).max(1)
    }

    /// The character size the font was made for. A font from [`ScaledFont::resized`] keeps the size of the font it
    /// came from: its cells are told by [`ScaledFont::cell_width`] and [`ScaledFont::cell_height`].
    #[must_use]
    pub fn size(&self) -> CharacterSize {
        self.size
    }

    /// Width of the glyph pictures, in pixels.
    #[must_use]
    pub fn cell_width(&self) -> u32 {
        Self::pixels(self.pitch.0)
    }

    /// Height of the glyph pictures, in pixels.
    #[must_use]
    pub fn cell_height(&self) -> u32 {
        Self::pixels(self.pitch.1)
    }

    /// The distance between the left edges of two cells in a row, and between the top edges of two rows, in pixels.
    /// It is the size of the cell for a font made by [`FontSheet::scaled`]. It is a fraction of a pixel when the font
    /// was resized, so that the grid does not drift from where it is on the video.
    #[must_use]
    pub fn pitch(&self) -> (f64, f64) {
        self.pitch
    }

    /// The same glyphs made `factor` times as large, for drawing on a picture that is `factor` times the size of the
    /// video, such as a preview. It is done once, not for every drawn glyph.
    #[must_use]
    pub fn resized(&self, factor: f64) -> Self {
        let pitch = (self.pitch.0 * factor, self.pitch.1 * factor);
        let cell = (Self::pixels(pitch.0), Self::pixels(pitch.1));
        let glyphs = if cell == (self.cell_width(), self.cell_height()) {
            self.glyphs.clone()
        } else {
            self.glyphs
                .iter()
                .map(|glyph| imageops::resize(glyph, cell.0, cell.1, FilterType::Triangle))
                .collect()
        };
        Self {
            size: self.size,
            pitch,
            glyphs,
        }
    }

    /// The image of glyph `code`, or `None` if the sheet has no such glyph.
    #[must_use]
    pub fn glyph(&self, code: u16) -> Option<&RgbaImage> {
        self.glyphs.get(usize::from(code))
    }
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::*;

    /// A sheet of 512 standard glyphs, each filled with its own grey level so that they can be told apart.
    fn synthetic_sheet() -> RgbaImage {
        let (width, height) = (24, 36);
        let mut image = RgbaImage::new(width, height * 512);
        for code in 0..512u32 {
            let level = (code % 256) as u8;
            for y in 0..height {
                for x in 0..width {
                    image.put_pixel(x, code * height + y, Rgba([level, level, level, 255]));
                }
            }
        }
        image
    }

    #[test]
    fn splits_the_sheet_into_glyphs() {
        let sheet = FontSheet::from_image(&synthetic_sheet()).unwrap();
        assert_eq!(sheet.layout().glyph_count(), 512);
        let scaled = sheet.scaled(CharacterSize::Small);
        assert_eq!(scaled.glyph(3).unwrap().get_pixel(0, 0)[0], 3);
        assert_eq!(scaled.glyph(300).unwrap().get_pixel(0, 0)[0], 44);
        assert!(scaled.glyph(512).is_none());
    }

    #[test]
    fn resizes_the_glyphs_to_the_render_size() {
        let sheet = FontSheet::from_image(&synthetic_sheet()).unwrap();
        let large = sheet.scaled(CharacterSize::Large);
        assert_eq!(large.size(), CharacterSize::Large);
        let glyph = large.glyph(1).unwrap();
        assert_eq!((glyph.width(), glyph.height()), (36, 54));
    }

    #[test]
    fn a_resized_font_keeps_every_glyph_and_scales_the_grid() {
        let sheet = FontSheet::from_image(&synthetic_sheet()).unwrap();
        let half = sheet.scaled(CharacterSize::Small).resized(0.5);
        assert_eq!(half.pitch(), (12.0, 18.0));
        assert_eq!((half.cell_width(), half.cell_height()), (12, 18));
        let glyph = half.glyph(3).unwrap();
        assert_eq!((glyph.width(), glyph.height()), (12, 18));
        assert_eq!(glyph.get_pixel(5, 5)[0], 3, "the glyph is still glyph 3");
        assert_eq!(half.glyph(300).unwrap().get_pixel(5, 5)[0], 44);
        assert!(half.glyph(512).is_none());
    }

    #[test]
    fn a_resized_font_keeps_the_exact_distance_between_cells_and_rounds_only_the_pictures() {
        let sheet = FontSheet::from_image(&synthetic_sheet()).unwrap();
        let font = sheet.scaled(CharacterSize::Small).resized(0.35);
        let (x, y) = font.pitch();
        assert!((x - 8.4).abs() < 1e-9 && (y - 12.6).abs() < 1e-9, "{x} {y}");
        assert_eq!((font.cell_width(), font.cell_height()), (8, 13));
        // A font is never made of pictures with no pixels.
        let tiny = sheet.scaled(CharacterSize::Small).resized(0.001);
        assert_eq!((tiny.cell_width(), tiny.cell_height()), (1, 1));
    }

    #[test]
    fn rejects_an_image_that_is_not_a_font() {
        let image = RgbaImage::new(10, 10);
        assert!(FontSheet::from_image(&image).is_err());
    }

    #[test]
    fn reads_a_png_from_memory() {
        let mut png = Vec::new();
        synthetic_sheet()
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let sheet = FontSheet::from_png(&png).unwrap();
        assert_eq!(sheet.layout().glyph_count(), 512);
        assert!(FontSheet::from_png(b"not a png").is_err());
    }
}
