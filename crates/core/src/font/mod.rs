// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! OSD font sheets.
//!
//! The goggles' fonts are PNG images with the glyphs laid out in columns: 256 or 512 characters, one, two, three
//! or four colour pages. The sheet size tells the layout. See `docs/formats/font.md`.
//!
//! A [`FontSheet`] is one such image. An [`OsdFont`] is what is drawn: a sheet, or the sheets of a font for several
//! sizes. The program carries some (see [`BUNDLED_FONTS`]), and the user can also open a file.

mod bundled;
mod layout;
mod osd_font;
mod sheet;

pub use bundled::{BUNDLED_FONTS, BundledFont};
pub use layout::{CharacterSize, FontLayout, LayoutError, detect};
pub use osd_font::OsdFont;
pub use sheet::{FontError, FontSheet, ScaledFont};
