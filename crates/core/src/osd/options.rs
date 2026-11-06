// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/osd/options.rs and ui/src/osd_preview.rs (GPL-3.0-only),
// Copyright (C) Alexander van Saase. Modified for Flight Replay: centring works on any frame, and the
// options are plain data with serde support.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::Frame;
use crate::font::CharacterSize;

/// How the OSD is placed on the picture.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsdOptions {
    /// Horizontal offset of the grid, in pixels from the left edge of the picture.
    pub x_offset: i32,
    /// Vertical offset of the grid, in pixels from the top edge of the picture.
    pub y_offset: i32,
    /// Grid cells (column, row) that are hidden. Use this to hide an element of the OSD.
    pub masked_cells: BTreeSet<(u32, u32)>,
}

impl OsdOptions {
    /// Whether the cell at column `x`, row `y` is hidden.
    #[must_use]
    pub fn is_masked(&self, x: u32, y: u32) -> bool {
        self.masked_cells.contains(&(x, y))
    }

    /// Hides a cell that is shown, or shows a cell that is hidden.
    pub fn toggle_mask(&mut self, x: u32, y: u32) {
        if !self.masked_cells.remove(&(x, y)) {
            self.masked_cells.insert((x, y));
        }
    }

    /// Centres the OSD on a picture of `width` x `height` pixels, using the cells that `frame` draws.
    ///
    /// The offset is computed from the bounding box of the drawn cells, so that the visible text is centred and
    /// not the whole grid. Does nothing if `frame` draws no glyph.
    pub fn center_on(&mut self, frame: &Frame, width: u32, height: u32, size: &CharacterSize) {
        let cells: Vec<(u32, u32)> = frame.glyphs().map(|(x, y, _)| (x, y)).collect();
        let (Some(min_x), Some(max_x)) = (
            cells.iter().map(|c| c.0).min(),
            cells.iter().map(|c| c.0).max(),
        ) else {
            return;
        };
        let (Some(min_y), Some(max_y)) = (
            cells.iter().map(|c| c.1).min(),
            cells.iter().map(|c| c.1).max(),
        ) else {
            return;
        };
        let range_w = i64::from(max_x - min_x + 1) * i64::from(size.width());
        let range_h = i64::from(max_y - min_y + 1) * i64::from(size.height());
        self.x_offset =
            ((i64::from(width) - range_w) / 2 - i64::from(min_x) * i64::from(size.width())) as i32;
        self.y_offset = ((i64::from(height) - range_h) / 2
            - i64::from(min_y) * i64::from(size.height())) as i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_with(glyphs: &[(u32, u32)]) -> Frame {
        let columns = 10;
        let mut cells = vec![0u16; 10 * 4];
        for &(x, y) in glyphs {
            cells[(y * columns + x) as usize] = 5;
        }
        Frame {
            time_ms: 0,
            columns,
            cells,
        }
    }

    #[test]
    fn masks_toggle() {
        let mut options = OsdOptions::default();
        options.toggle_mask(3, 1);
        assert!(options.is_masked(3, 1));
        options.toggle_mask(3, 1);
        assert!(!options.is_masked(3, 1));
    }

    #[test]
    fn centres_the_drawn_cells() {
        // Drawn cells span columns 2..=7 (6 cells) and rows 1..=2 (2 cells) of a 720p 24x36 grid.
        let frame = frame_with(&[(2, 1), (7, 2)]);
        let mut options = OsdOptions::default();
        options.center_on(&frame, 1280, 720, &CharacterSize::Small);
        // (1280 - 6*24) / 2 - 2*24 = 568 - 48 = 520; (720 - 2*36) / 2 - 1*36 = 324 - 36 = 288
        assert_eq!((options.x_offset, options.y_offset), (520, 288));
    }

    #[test]
    fn centring_on_an_empty_frame_changes_nothing() {
        let mut options = OsdOptions {
            x_offset: 7,
            y_offset: 9,
            masked_cells: BTreeSet::new(),
        };
        options.center_on(&frame_with(&[]), 1280, 720, &CharacterSize::Small);
        assert_eq!((options.x_offset, options.y_offset), (7, 9));
    }
}
