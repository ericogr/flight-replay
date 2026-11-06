// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The OSD fonts that come with the program.
//!
//! They are the sheets of `assets/osd-fonts/`, compiled in like the font of the SRT text (see [`crate::assets`]).
//! Each has a sheet for 720p and one for 1080p. Where each one comes from and its license are in the README of that
//! folder and in `NOTICE`; the window shows the credit next to the font.

use std::fmt;

use super::osd_font::OsdFont;
use super::sheet::{FontError, FontSheet};
use crate::osd::Firmware;

/// An OSD font that comes with the program.
pub struct BundledFont {
    /// A short name that never changes, which the settings keep.
    pub id: &'static str,
    /// What the list shows.
    pub name: &'static str,
    /// Who made it, and under which license.
    pub credit: &'static str,
    /// The code of the firmware whose glyphs it draws, as an `.osd` file names it.
    firmware_code: &'static [u8; 4],
    /// The sheet for 720p, with cells of 24 x 36.
    small: &'static [u8],
    /// The sheet for 1080p, with cells of 36 x 54.
    large: &'static [u8],
}

/// Every font that comes with the program.
pub const BUNDLED_FONTS: &[BundledFont] = &[
    BundledFont {
        id: "inav-default",
        name: "INAV, default",
        credit: "The default font of INAV, by the INAV Configurator contributors (GPL-3.0).",
        firmware_code: b"INAV",
        small: include_bytes!("../../../../assets/osd-fonts/inav/WS_INAV_Default_24.png"),
        large: include_bytes!("../../../../assets/osd-fonts/inav/WS_INAV_Default_36.png"),
    },
    BundledFont {
        id: "betaflight-material",
        name: "Betaflight, Material",
        credit: "Material OSD by Knifa (CC BY-SA 4.0), made from Material Design Icons and Bebas Neue.",
        firmware_code: b"BTFL",
        small: include_bytes!("../../../../assets/osd-fonts/betaflight/WS_BTFL_Material_24.png"),
        large: include_bytes!("../../../../assets/osd-fonts/betaflight/WS_BTFL_Material_36.png"),
    },
];

impl BundledFont {
    /// The font with this id, if there is one.
    #[must_use]
    pub fn find(id: &str) -> Option<&'static BundledFont> {
        BUNDLED_FONTS.iter().find(|font| font.id == id)
    }

    /// The firmware whose glyphs the font draws.
    #[must_use]
    pub fn firmware(&self) -> Firmware {
        Firmware::from_code(self.firmware_code)
    }

    /// Reads the sheets.
    pub fn load(&self) -> Result<OsdFont, FontError> {
        Ok(OsdFont::new(
            FontSheet::from_png(self.small)?,
            vec![FontSheet::from_png(self.large)?],
        ))
    }
}

impl fmt::Display for BundledFont {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

// The sheets are left out: they are hundreds of kilobytes of PNG.
impl fmt::Debug for BundledFont {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BundledFont")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for BundledFont {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for BundledFont {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::CharacterSize;

    #[test]
    fn every_font_loads_with_a_sheet_for_720p_and_one_for_1080p() {
        for bundled in BUNDLED_FONTS {
            let font = bundled
                .load()
                .unwrap_or_else(|e| panic!("{}: {e}", bundled.id));
            let sizes: Vec<CharacterSize> = font.sheets().map(|s| s.layout().size).collect();
            assert_eq!(
                sizes,
                [CharacterSize::Small, CharacterSize::Large],
                "{}",
                bundled.id
            );
            assert_eq!(font.sheet_for(720).layout().size, CharacterSize::Small);
            assert_eq!(font.sheet_for(1080).layout().size, CharacterSize::Large);
        }
    }

    #[test]
    fn both_sheets_of_a_font_have_the_same_glyphs() {
        for bundled in BUNDLED_FONTS {
            let font = bundled.load().unwrap();
            let counts: Vec<u32> = font.sheets().map(|s| s.layout().glyph_count()).collect();
            assert_eq!(counts[0], counts[1], "{}", bundled.id);
        }
    }

    #[test]
    fn the_inav_font_has_the_512_glyphs_the_firmware_uses() {
        let font = BundledFont::find("inav-default").unwrap().load().unwrap();
        assert_eq!(font.sheet_for(720).layout().glyph_count(), 512);
        assert_eq!(font.sheet_for(720).layout().pages, 1);
    }

    #[test]
    fn the_betaflight_font_has_one_page_of_256_glyphs() {
        let font = BundledFont::find("betaflight-material")
            .unwrap()
            .load()
            .unwrap();
        assert_eq!(font.sheet_for(720).layout().glyph_count(), 256);
    }

    #[test]
    fn the_ids_are_different_and_each_font_is_found_by_its_id() {
        for (index, font) in BUNDLED_FONTS.iter().enumerate() {
            assert_eq!(BundledFont::find(font.id), Some(font));
            assert!(
                BUNDLED_FONTS[index + 1..]
                    .iter()
                    .all(|other| other.id != font.id),
                "{} is used twice",
                font.id
            );
        }
        assert!(BundledFont::find("no-such-font").is_none());
    }

    #[test]
    fn each_font_names_its_firmware() {
        assert_eq!(
            BundledFont::find("inav-default").unwrap().firmware(),
            Firmware::Inav
        );
        assert_eq!(
            BundledFont::find("betaflight-material").unwrap().firmware(),
            Firmware::Betaflight
        );
    }

    #[test]
    fn the_debug_text_is_short() {
        let text = format!("{:?}", BUNDLED_FONTS[0]);
        assert!(text.len() < 100, "{text}");
    }
}
