// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/screen.go (Apache-2.0, Copyright 2026 EricoGR).

use crate::osd::{Frame, OsdFile};
use crate::telemetry::inav;

/// What an OSD frame is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Screen {
    /// The pilot's own layout.
    Flight,
    /// The INAV logo, shown while arming.
    Splash,
    /// The statistics, shown after disarming.
    Stats,
}

/// Starts the title line of the statistics screen.
const STATS_TITLE: &str = "*** STATS";

/// Which screen a frame shows.
pub(crate) fn screen_of(file: &OsdFile, frame: &Frame) -> Screen {
    let mut kind = Screen::Flight;
    for y in 0..file.rows {
        let row = frame.row(y);
        if has_text(row, STATS_TITLE) {
            return Screen::Stats;
        }
        if row.iter().any(|&code| inav::is_logo(code)) {
            kind = Screen::Splash;
        }
    }
    kind
}

/// Whether the ASCII text appears in the cells of a row.
fn has_text(row: &[u16], text: &str) -> bool {
    let bytes = text.as_bytes();
    row.windows(bytes.len()).any(|window| {
        window
            .iter()
            .zip(bytes)
            .all(|(&cell, &byte)| cell == u16::from(byte))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_text_in_a_row() {
        let row: Vec<u16> = "  *** STATS ***".bytes().map(u16::from).collect();
        assert!(has_text(&row, STATS_TITLE));
        assert!(!has_text(&row[..8], STATS_TITLE));
    }
}
