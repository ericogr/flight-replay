// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/font/dimensions.rs (GPL-3.0-only), Copyright (C) Alexander van
// Saase. Modified for Flight Replay: the table is generated from the character sizes and page counts instead of
// being written out, and the tests are kept.

use std::fmt;

/// Size of one character cell, matched to the video height it is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterSize {
    /// 24x36 cells, for 720p video.
    Small,
    /// 36x54 cells, for 1080p video.
    Large,
    /// 48x72 cells, for 1440p (2.7K) video.
    XLarge,
    /// 72x108 cells, for 2160p (4K) video.
    Ultra,
    /// 18x27 cells, for 540p video.
    Race,
}

impl CharacterSize {
    /// Every size, in the order the font sheets are checked.
    pub const ALL: [CharacterSize; 5] = [
        Self::Small,
        Self::Large,
        Self::XLarge,
        Self::Ultra,
        Self::Race,
    ];

    /// Width of a cell, in pixels.
    #[must_use]
    pub const fn width(self) -> u32 {
        match self {
            Self::Small => 24,
            Self::Large => 36,
            Self::XLarge => 48,
            Self::Ultra => 72,
            Self::Race => 18,
        }
    }

    /// Height of a cell, in pixels.
    #[must_use]
    pub const fn height(self) -> u32 {
        match self {
            Self::Small => 36,
            Self::Large => 54,
            Self::XLarge => 72,
            Self::Ultra => 108,
            Self::Race => 27,
        }
    }

    /// The size that matches a video of this height. Heights that are not a standard size use the 1080p size.
    #[must_use]
    pub fn for_video_height(height: u32) -> Self {
        match height {
            540 => Self::Race,
            720 => Self::Small,
            1440 => Self::XLarge,
            2160 => Self::Ultra,
            _ => Self::Large,
        }
    }
}

impl fmt::Display for CharacterSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Small => "720p",
            Self::Large => "1080p",
            Self::XLarge => "2.7K",
            Self::Ultra => "4K",
            Self::Race => "540p",
        })
    }
}

/// How a font sheet is laid out: the cell size, how many pages of colour it has, and characters per page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontLayout {
    /// Size of one cell.
    pub size: CharacterSize,
    /// Number of pages side by side. Four pages is the four-colour (Betaflight 4.5) layout.
    pub pages: u32,
    /// Characters per page: 256 or 512.
    pub characters_per_page: u32,
}

impl FontLayout {
    /// Total number of glyphs in the sheet.
    #[must_use]
    pub fn glyph_count(&self) -> u32 {
        self.pages * self.characters_per_page
    }
}

/// Errors when a sheet does not have a known layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// The width is not a known cell width times 1 to 4 pages.
    #[error("the font sheet is {width} pixels wide, which matches no known character size")]
    UnknownWidth {
        /// Width of the sheet.
        width: u32,
    },
    /// The height is not a whole number of cells, or the number of cells is not 256 or 512 per page.
    #[error(
        "the font sheet is {height} pixels high, which is not a whole number of 256 or 512 characters"
    )]
    BadHeight {
        /// Height of the sheet.
        height: u32,
    },
}

/// Works out the layout of a sheet of `width` x `height` pixels.
pub fn detect(width: u32, height: u32) -> Result<FontLayout, LayoutError> {
    // The pages are checked before the sizes: a 36 px wide sheet can be a 36 px cell or two 18 px cells. A
    // candidate only matches when its character count is a multiple of 256, which is what tells them apart.
    let mut width_matches = false;
    for pages in 1..=4 {
        for size in CharacterSize::ALL {
            if width != size.width() * pages {
                continue;
            }
            width_matches = true;
            let whole_cells = height / size.height();
            if height < size.height() || !whole_cells.is_multiple_of(256) {
                continue;
            }
            if !height.is_multiple_of(size.height()) {
                return Err(LayoutError::BadHeight { height });
            }
            return Ok(FontLayout {
                size,
                pages,
                characters_per_page: whole_cells,
            });
        }
    }
    if width_matches {
        Err(LayoutError::BadHeight { height })
    } else {
        Err(LayoutError::UnknownWidth { width })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_known_sheet_sizes() {
        let cases = [
            (72, 27_648, CharacterSize::Ultra, 1, 256),
            (48, 18_432, CharacterSize::XLarge, 1, 256),
            (36, 13_824, CharacterSize::Large, 1, 256),
            (24, 18_432, CharacterSize::Small, 1, 512),
            (36, 27_648, CharacterSize::Large, 1, 512),
            (48, 36_864, CharacterSize::XLarge, 1, 512),
            (72, 55_296, CharacterSize::Ultra, 1, 512),
            (48, 9_216, CharacterSize::Small, 2, 256),
            (72, 13_824, CharacterSize::Large, 2, 256),
            (96, 18_432, CharacterSize::XLarge, 2, 256),
            (144, 27_648, CharacterSize::Ultra, 2, 256),
            (72, 9_216, CharacterSize::Small, 3, 256),
            (108, 13_824, CharacterSize::Large, 3, 256),
            (144, 18_432, CharacterSize::XLarge, 3, 256),
            (216, 27_648, CharacterSize::Ultra, 3, 256),
            (96, 9_216, CharacterSize::Small, 4, 256),
            (144, 13_824, CharacterSize::Large, 4, 256),
            (192, 18_432, CharacterSize::XLarge, 4, 256),
            (288, 27_648, CharacterSize::Ultra, 4, 256),
            (18, 6_912, CharacterSize::Race, 1, 256),
            (36, 6_912, CharacterSize::Race, 2, 256),
            (54, 6_912, CharacterSize::Race, 3, 256),
            (72, 6_912, CharacterSize::Race, 4, 256),
            (18, 13_824, CharacterSize::Race, 1, 512),
        ];
        for (width, height, size, pages, characters) in cases {
            assert_eq!(
                detect(width, height).unwrap(),
                FontLayout {
                    size,
                    pages,
                    characters_per_page: characters
                },
                "{width}x{height}"
            );
        }
    }

    #[test]
    fn rejects_sheets_that_are_not_whole_cells() {
        assert!(detect(36, 13_825).is_err());
        assert!(detect(25, 18_432).is_err());
        assert!(detect(24, 1_000).is_err());
    }
}
