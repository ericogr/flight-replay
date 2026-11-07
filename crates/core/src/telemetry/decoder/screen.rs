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

/// Starts the title line of the statistics screen. INAV 8 and later write `*** STATS`, earlier versions `--- STATS`
/// (and, when the screen has two pages, the page number after it).
const STATS_TITLES: [&str; 2] = ["*** STATS", "--- STATS"];

/// Which screen a frame shows.
pub(crate) fn screen_of(file: &OsdFile, frame: &Frame) -> Screen {
    let mut kind = Screen::Flight;
    for y in 0..file.rows {
        let row = frame.row(y);
        if STATS_TITLES.iter().any(|title| has_text(row, title)) {
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
    use crate::osd::Firmware;
    use crate::telemetry::decoder::{Decoder, Options};

    /// A 53 x 20 INAV recording of one frame whose rows hold the given lines: each is `(row, column, cells)`.
    fn file_with(lines: &[(u32, usize, &[u16])]) -> OsdFile {
        let (columns, rows) = (53, 20);
        let mut cells = vec![0u16; (columns * rows) as usize];
        for &(row, column, text) in lines {
            let start = row as usize * columns as usize + column;
            cells[start..start + text.len()].copy_from_slice(text);
        }
        OsdFile {
            firmware: Firmware::Inav,
            columns,
            rows,
            frames: vec![Frame {
                time_ms: 0,
                columns,
                cells,
            }],
            trailing_bytes: 0,
        }
    }

    fn cells(text: &str) -> Vec<u16> {
        text.bytes().map(u16::from).collect()
    }

    fn screen(title: &str) -> Screen {
        let file = file_with(&[(1, 11, &cells(title))]);
        screen_of(&file, &file.frames[0])
    }

    #[test]
    fn finds_text_in_a_row() {
        let row: Vec<u16> = "  *** STATS ***".bytes().map(u16::from).collect();
        assert!(has_text(&row, "*** STATS"));
        assert!(!has_text(&row[..8], "*** STATS"));
    }

    #[test]
    fn the_statistics_screen_is_known_by_the_title_of_every_inav_version() {
        // INAV 8 and later
        assert_eq!(screen("*** STATS "), Screen::Stats);
        assert_eq!(screen("*** STATS   1/2 -> ***"), Screen::Stats);
        assert_eq!(screen("*** STATS   <- 2/2 ***"), Screen::Stats);
        // INAV 7 and earlier
        assert_eq!(screen("--- STATS ---"), Screen::Stats);
        assert_eq!(screen("--- STATS ---      1/2 ->"), Screen::Stats);
        assert_eq!(screen("--- STATS ---   <- 2/2"), Screen::Stats);
    }

    #[test]
    fn other_text_is_not_the_statistics_screen() {
        assert_eq!(screen("ACRO"), Screen::Flight);
        assert_eq!(screen("--- STAT"), Screen::Flight);
        assert_eq!(screen("STATS"), Screen::Flight);
    }

    /// The speed line of the statistics screen is a maximum and an average, not the speed of the aircraft.
    #[test]
    fn the_numbers_of_the_statistics_screen_are_not_readings() {
        let mut speed = cells("MAX/AVG SPEED    : 114");
        speed.push(inav::UNIT_KMH_3D);
        speed.extend(cells("/72"));
        speed.push(inav::UNIT_KMH);
        let file = file_with(&[(1, 11, &cells("--- STATS ---")), (2, 11, &speed)]);
        let (sample, _) = Decoder::new(Options::default()).decode(&file, &file.frames[0]);
        assert!(sample.is_empty(), "{sample:?}");
    }
}
