// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::cmp::Reverse;

use super::layout::CharacterSize;
use super::sheet::{FontSheet, ScaledFont};

/// An OSD font: the sheet, or the sheets, that hold its glyphs.
///
/// A font file of the goggles is one sheet, drawn for one cell size. A font that comes with the program has a sheet
/// for each of the sizes the goggles use, and the one nearest to the video is drawn, so that a 720p video is not
/// made from a sheet drawn for 1080p.
#[derive(Debug, Clone)]
pub struct OsdFont {
    first: FontSheet,
    more: Vec<FontSheet>,
}

impl OsdFont {
    /// A font of one sheet, such as a file the user chose.
    #[must_use]
    pub fn single(sheet: FontSheet) -> Self {
        Self {
            first: sheet,
            more: Vec::new(),
        }
    }

    /// A font of several sheets, drawn for different cell sizes.
    #[must_use]
    pub fn new(first: FontSheet, more: Vec<FontSheet>) -> Self {
        Self { first, more }
    }

    /// Every sheet of the font.
    pub fn sheets(&self) -> impl Iterator<Item = &FontSheet> {
        std::iter::once(&self.first).chain(&self.more)
    }

    /// The sheet to draw on a video this many pixels high: the one whose cells are nearest in height to the cells
    /// of that video, and the larger one when two are as near.
    #[must_use]
    pub fn sheet_for(&self, video_height: u32) -> &FontSheet {
        let wanted = CharacterSize::for_video_height(video_height).height();
        self.sheets()
            .min_by_key(|sheet| {
                let height = sheet.layout().size.height();
                (height.abs_diff(wanted), Reverse(height))
            })
            .unwrap_or(&self.first)
    }

    /// The glyphs at the size of a video this many pixels high, made from the sheet that suits it best.
    #[must_use]
    pub fn scaled_for(&self, video_height: u32) -> ScaledFont {
        self.sheet_for(video_height)
            .scaled(CharacterSize::for_video_height(video_height))
    }
}

#[cfg(test)]
mod tests {
    use image::RgbaImage;

    use super::*;

    /// A sheet of 256 glyphs with cells of this size.
    fn sheet(size: CharacterSize) -> FontSheet {
        FontSheet::from_image(&RgbaImage::new(size.width(), size.height() * 256)).unwrap()
    }

    fn native(font: &OsdFont, video_height: u32) -> CharacterSize {
        font.sheet_for(video_height).layout().size
    }

    #[test]
    fn a_font_of_one_sheet_always_uses_it() {
        let font = OsdFont::single(sheet(CharacterSize::Large));
        for height in [360, 540, 720, 1080, 1440, 2160] {
            assert_eq!(native(&font, height), CharacterSize::Large, "{height}");
        }
    }

    #[test]
    fn the_sheet_with_the_nearest_cells_is_used() {
        let font = OsdFont::new(
            sheet(CharacterSize::Small),
            vec![sheet(CharacterSize::Large)],
        );
        // 540p cells are 27 high: 36 is nearer than 54.
        assert_eq!(native(&font, 540), CharacterSize::Small);
        assert_eq!(native(&font, 720), CharacterSize::Small);
        assert_eq!(native(&font, 1080), CharacterSize::Large);
        // 1440p cells are 72 high: 54 is nearer than 36.
        assert_eq!(native(&font, 1440), CharacterSize::Large);
        assert_eq!(native(&font, 2160), CharacterSize::Large);
        // Heights that are not a standard size are drawn as 1080p.
        assert_eq!(native(&font, 800), CharacterSize::Large);
    }

    #[test]
    fn the_order_of_the_sheets_does_not_matter() {
        let font = OsdFont::new(
            sheet(CharacterSize::Large),
            vec![sheet(CharacterSize::Small)],
        );
        assert_eq!(native(&font, 720), CharacterSize::Small);
        assert_eq!(native(&font, 1080), CharacterSize::Large);
    }

    #[test]
    fn the_glyphs_are_made_at_the_size_of_the_video() {
        let font = OsdFont::new(
            sheet(CharacterSize::Small),
            vec![sheet(CharacterSize::Large)],
        );
        assert_eq!(font.scaled_for(720).size(), CharacterSize::Small);
        assert_eq!(font.scaled_for(1440).size(), CharacterSize::XLarge);
    }
}
