// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/inav/glyphs.go and inav.go (Apache-2.0, Copyright 2026 EricoGR).

//! The glyph codes of the INAV OSD font: which ones are digits, graphics or icons, and what they are called.
//!
//! Nothing outside this module repeats these ranges.

mod flight_mode;
mod symbols;

use std::borrow::Cow;

pub use flight_mode::{FLIGHT_MODE_WIDTH, is_flight_mode};
pub use symbols::*;

/// The last code of the OSD font, which has 512 characters.
pub const MAX_GLYPH: u16 = 0x1ff;

/// `0.` to `9.`: a digit with half a decimal point on its right.
pub const TRAILING_DOT_FIRST: u16 = 0xA1;
/// See [`TRAILING_DOT_FIRST`].
pub const TRAILING_DOT_LAST: u16 = 0xAA;
/// `.0` to `.9`: a digit with half a decimal point on its left.
pub const LEADING_DOT_FIRST: u16 = 0xB1;
/// See [`LEADING_DOT_FIRST`].
pub const LEADING_DOT_LAST: u16 = 0xBA;

/// Eight small arrows that give a direction (wind, fence).
pub const DIRECTION_ARROW_FIRST: u16 = 0x17;
/// See [`DIRECTION_ARROW_FIRST`].
pub const DIRECTION_ARROW_LAST: u16 = 0x1E;
/// Sixteen arrows that give the direction of home.
pub const HOME_ARROW_FIRST: u16 = 0x13C;
/// See [`HOME_ARROW_FIRST`].
pub const HOME_ARROW_LAST: u16 = 0x14B;
/// Forty pieces of the logo (4x10), drawn on the splash screen.
pub const LOGO_FIRST: u16 = 0x101;
/// See [`LOGO_FIRST`].
pub const LOGO_LAST: u16 = LOGO_FIRST + 40 - 1;

/// A readable name for a glyph code, or `None` for plain ASCII characters and unknown codes.
#[must_use]
pub fn name(code: u16) -> Option<Cow<'static, str>> {
    match code {
        TRAILING_DOT_FIRST..=TRAILING_DOT_LAST => {
            Some(format!("{}.", code - TRAILING_DOT_FIRST).into())
        }
        LEADING_DOT_FIRST..=LEADING_DOT_LAST => {
            Some(format!(".{}", code - LEADING_DOT_FIRST).into())
        }
        DIRECTION_ARROW_FIRST..=DIRECTION_ARROW_LAST => {
            Some(format!("DIRECTION_ARROW_{}", code - DIRECTION_ARROW_FIRST).into())
        }
        HOME_ARROW_FIRST..=HOME_ARROW_LAST => {
            Some(format!("HOME_ARROW_{}", code - HOME_ARROW_FIRST).into())
        }
        LOGO_FIRST..=LOGO_LAST => Some(format!("LOGO_{}", code - LOGO_FIRST).into()),
        _ => name_of(code).map(Cow::Borrowed),
    }
}

/// Turns a sequence of numeric and ASCII glyphs into text. A digit with a half point is combined with its
/// neighbour: `[3.][.6]9` becomes `3.69`. Glyphs that are not text become `?`.
#[must_use]
pub fn text(codes: &[u16]) -> String {
    let mut out = String::new();
    for &code in codes {
        match code {
            TRAILING_DOT_FIRST..=TRAILING_DOT_LAST => {
                out.push(digit(code - TRAILING_DOT_FIRST));
                out.push('.');
            }
            LEADING_DOT_FIRST..=LEADING_DOT_LAST => {
                // The point was already written if the previous glyph had a half point on its right.
                if !out.ends_with('.') {
                    out.push('.');
                }
                out.push(digit(code - LEADING_DOT_FIRST));
            }
            0x20..0x7f => out.push(char::from(code as u8)),
            _ => out.push('?'),
        }
    }
    out
}

fn digit(value: u16) -> char {
    char::from(b'0' + value as u8)
}

/// Whether the glyph is a digit with half a decimal point on its right (`3.`) or on its left (`.6`): the OSD
/// writes 3.69 as `[3.][.6]9`.
#[must_use]
pub fn is_half_dot(code: u16) -> bool {
    (TRAILING_DOT_FIRST..=TRAILING_DOT_LAST).contains(&code)
        || (LEADING_DOT_FIRST..=LEADING_DOT_LAST).contains(&code)
}

/// Whether the glyph is a piece of the logo, which the OSD draws on the splash screen shown while arming.
#[must_use]
pub fn is_logo(code: u16) -> bool {
    (LOGO_FIRST..=LOGO_LAST).contains(&code)
}

/// Whether the glyph belongs to a graphic element (artificial horizon, crosshair, variometer, HUD arrows), which
/// carries no value.
#[must_use]
pub fn is_graphic(code: u16) -> bool {
    matches!(code, 0x12c..=0x133 | 0x14c..=0x164 | 0x166 | 0x190..=0x1a6 | 0x1ae..=0x1b9)
}

/// Whether a cell holds an icon or a unit of the font, as opposed to a blank, a digit, a letter or punctuation, a
/// digit with half a decimal point, a graphic or a piece of the logo.
///
/// Besides the codes below the space and above ASCII, the font puts glyphs on the lowercase codes (the text itself
/// is all uppercase), such as the `v` that stands for metres. The glyphs that sit on other ASCII codes (the quote
/// of Ah/km, for one) cannot be told from punctuation; the rules that know them look for them by code.
#[must_use]
pub fn is_glyph(code: u16) -> bool {
    match code {
        0 | 0x20 => false,
        c if c < 0x20 => true,
        c if c < 0x60 => false, // digits, uppercase letters and punctuation
        c if c > MAX_GLYPH => false,
        c => !is_half_dot(c) && !is_graphic(c) && !is_logo(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_half_points_into_decimals() {
        // 3.69 is drawn as [3.][.6]9
        assert_eq!(
            text(&[
                TRAILING_DOT_FIRST + 3,
                LEADING_DOT_FIRST + 6,
                u16::from(b'9')
            ]),
            "3.69"
        );
        assert_eq!(
            text(&[u16::from(b'1'), u16::from(b'2'), LEADING_DOT_FIRST + 5]),
            "12.5"
        );
        assert_eq!(text(&[u16::from(b'-'), u16::from(b'7')]), "-7");
        assert_eq!(text(&[0x01]), "?");
    }

    #[test]
    fn names_ranges_and_table_entries() {
        assert_eq!(name(TRAILING_DOT_FIRST + 2).unwrap(), "2.");
        assert_eq!(name(HOME_ARROW_FIRST + 1).unwrap(), "HOME_ARROW_1");
        assert_eq!(name(ICON_RSSI).unwrap(), "ICON_RSSI");
        assert_eq!(name(u16::from(b'A')), None);
    }

    #[test]
    fn classifies_glyphs() {
        assert!(is_glyph(ICON_RSSI));
        assert!(!is_glyph(u16::from(b'5')));
        assert!(!is_glyph(0));
        assert!(!is_glyph(TRAILING_DOT_FIRST));
        assert!(is_logo(LOGO_FIRST));
        assert!(is_graphic(0x166));
    }
}
