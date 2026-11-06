// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! OSD font sheets.
//!
//! The goggles' fonts are PNG images with the glyphs laid out in columns: 256 or 512 characters, one, two, three
//! or four colour pages. The sheet size tells the layout. See `docs/formats/font.md`.

mod layout;
mod sheet;

pub use layout::{CharacterSize, FontLayout, LayoutError, detect};
pub use sheet::{FontError, FontSheet, ScaledFont};
