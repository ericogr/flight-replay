// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// The file layout follows fpv-motorsound's internal/osd/osd.go (Apache-2.0, same author) and walksnail-osd-tool's
// backend/src/osd/osd_file.rs (GPL-3.0-only). The grid size is read from the header, so both INAV and Betaflight
// recordings are handled by the same code.

use std::fs;
use std::io;
use std::path::Path;

use super::Firmware;

/// Bytes of the file header.
pub const HEADER_BYTES: usize = 40;
const COLUMNS_OFFSET: usize = 0x24;
const ROWS_OFFSET: usize = 0x26;
const TIMESTAMP_BYTES: usize = 4;

/// Glyph code of a blank cell (space).
pub const SPACE: u16 = 0x20;

/// One OSD screen: the time it was recorded and the glyph code of every cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Time of the screen, in milliseconds from the start of the recording.
    pub time_ms: u32,
    /// Number of columns of the grid.
    pub columns: u32,
    /// Glyph codes, row by row. Code 0 is an empty cell.
    pub cells: Vec<u16>,
}

impl Frame {
    /// Glyph code of the cell at column `x`, row `y`.
    #[must_use]
    pub fn cell(&self, x: u32, y: u32) -> u16 {
        self.cells[(y * self.columns + x) as usize]
    }

    /// Cells that hold a glyph to draw: `(column, row, code)`. Blank cells (code 0 and space) are skipped.
    pub fn glyphs(&self) -> impl Iterator<Item = (u32, u32, u16)> + '_ {
        let columns = self.columns;
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, code)| **code != 0 && **code != SPACE)
            .map(move |(index, &code)| {
                let index = index as u32;
                (index % columns, index / columns, code)
            })
    }
}

/// A whole `.osd` recording.
#[derive(Debug, Clone, PartialEq)]
pub struct OsdFile {
    /// Firmware named in the header.
    pub firmware: Firmware,
    /// Number of columns of the grid.
    pub columns: u32,
    /// Number of rows of the grid.
    pub rows: u32,
    /// The screens, in recording order.
    pub frames: Vec<Frame>,
    /// Bytes at the end of the file that do not make a whole frame. They are ignored.
    pub trailing_bytes: usize,
}

/// Errors when reading an `.osd` file.
#[derive(Debug, thiserror::Error)]
pub enum OsdError {
    /// The file could not be read.
    #[error("could not read the OSD file: {0}")]
    Io(#[from] io::Error),
    /// The file is shorter than its header.
    #[error("the OSD file is shorter than its {HEADER_BYTES}-byte header")]
    TooShort,
    /// The header describes a grid with no cells.
    #[error("the OSD header describes an empty grid ({columns}x{rows})")]
    EmptyGrid {
        /// Columns in the header.
        columns: u32,
        /// Rows in the header.
        rows: u32,
    },
    /// The file has no complete frame.
    #[error("the OSD file has no frames")]
    NoFrames,
}

impl OsdFile {
    /// Reads and parses an `.osd` file.
    pub fn open(path: &Path) -> Result<Self, OsdError> {
        Self::parse(&fs::read(path)?)
    }

    /// Parses the contents of an `.osd` file.
    pub fn parse(bytes: &[u8]) -> Result<Self, OsdError> {
        if bytes.len() < HEADER_BYTES {
            return Err(OsdError::TooShort);
        }
        let firmware = Firmware::from_code(&bytes[..4]);
        let columns = u32::from(u16::from_le_bytes([
            bytes[COLUMNS_OFFSET],
            bytes[COLUMNS_OFFSET + 1],
        ]));
        let rows = u32::from(u16::from_le_bytes([
            bytes[ROWS_OFFSET],
            bytes[ROWS_OFFSET + 1],
        ]));
        if columns == 0 || rows == 0 {
            return Err(OsdError::EmptyGrid { columns, rows });
        }

        let frame_bytes = TIMESTAMP_BYTES + (columns * rows) as usize * 2;
        let body = &bytes[HEADER_BYTES..];
        let trailing_bytes = body.len() % frame_bytes;
        let frames: Vec<Frame> = body
            .chunks_exact(frame_bytes)
            .map(|chunk| Frame {
                time_ms: u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
                columns,
                cells: chunk[TIMESTAMP_BYTES..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|cell| u16::from_le_bytes(*cell))
                    .collect(),
            })
            .collect();
        if frames.is_empty() {
            return Err(OsdError::NoFrames);
        }
        if trailing_bytes > 0 {
            tracing::warn!(
                trailing_bytes,
                "the OSD file ends with an incomplete frame; it is ignored"
            );
        }
        Ok(Self {
            firmware,
            columns,
            rows,
            frames,
            trailing_bytes,
        })
    }

    /// Length of the recording in seconds: the last screen's time plus the average interval between screens.
    #[must_use]
    pub fn duration_s(&self) -> f64 {
        let first = f64::from(self.frames[0].time_ms);
        let last = f64::from(self.frames[self.frames.len() - 1].time_ms);
        let interval = if self.frames.len() > 1 {
            (last - first) / (self.frames.len() - 1) as f64
        } else {
            0.0
        };
        (last + interval) / 1000.0
    }

    /// Index of the screen that is current at `time_ms`: the last one recorded at or before that time.
    /// Before the first screen, the first one is used.
    #[must_use]
    pub fn frame_index_at(&self, time_ms: f64) -> usize {
        self.frames
            .partition_point(|frame| f64::from(frame.time_ms) <= time_ms)
            .saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an `.osd` file with a 2x2 grid and the given frames, each given as (time, four cell codes).
    fn build(firmware: [u8; 4], frames: &[(u32, [u16; 4])], trailing: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; HEADER_BYTES];
        bytes[..4].copy_from_slice(&firmware);
        bytes[COLUMNS_OFFSET..COLUMNS_OFFSET + 2].copy_from_slice(&2u16.to_le_bytes());
        bytes[ROWS_OFFSET..ROWS_OFFSET + 2].copy_from_slice(&2u16.to_le_bytes());
        for (time, cells) in frames {
            bytes.extend_from_slice(&time.to_le_bytes());
            for cell in cells {
                bytes.extend_from_slice(&cell.to_le_bytes());
            }
        }
        bytes.extend(std::iter::repeat_n(0xAB, trailing));
        bytes
    }

    #[test]
    fn parses_frames_and_the_header() {
        let bytes = build(*b"INAV", &[(0, [1, 2, 3, 4]), (150, [0, 0x20, 0x41, 0])], 0);
        let file = OsdFile::parse(&bytes).unwrap();
        assert_eq!(file.firmware, Firmware::Inav);
        assert_eq!((file.columns, file.rows), (2, 2));
        assert_eq!(file.frames.len(), 2);
        assert_eq!(file.frames[1].time_ms, 150);
        assert_eq!(file.frames[1].cell(0, 1), 0x41);
    }

    #[test]
    fn lists_only_the_cells_with_a_glyph() {
        let bytes = build(*b"BTFL", &[(0, [0, 0x20, 7, 0])], 0);
        let file = OsdFile::parse(&bytes).unwrap();
        let glyphs: Vec<_> = file.frames[0].glyphs().collect();
        assert_eq!(glyphs, vec![(0, 1, 7)]);
        assert_eq!(file.firmware, Firmware::Betaflight);
    }

    #[test]
    fn ignores_an_incomplete_last_frame() {
        let bytes = build(*b"INAV", &[(0, [1, 1, 1, 1])], 5);
        let file = OsdFile::parse(&bytes).unwrap();
        assert_eq!(file.frames.len(), 1);
        assert_eq!(file.trailing_bytes, 5);
    }

    #[test]
    fn rejects_short_files_empty_grids_and_files_without_frames() {
        assert!(matches!(OsdFile::parse(&[0; 10]), Err(OsdError::TooShort)));
        let mut bytes = build(*b"INAV", &[], 0);
        bytes[COLUMNS_OFFSET] = 0;
        assert!(matches!(
            OsdFile::parse(&bytes),
            Err(OsdError::EmptyGrid { .. })
        ));
        assert!(matches!(
            OsdFile::parse(&build(*b"INAV", &[], 0)),
            Err(OsdError::NoFrames)
        ));
    }

    #[test]
    fn finds_the_current_frame_and_the_duration() {
        let bytes = build(*b"INAV", &[(0, [0; 4]), (100, [0; 4]), (200, [0; 4])], 0);
        let file = OsdFile::parse(&bytes).unwrap();
        assert_eq!(file.frame_index_at(-5.0), 0);
        assert_eq!(file.frame_index_at(99.9), 0);
        assert_eq!(file.frame_index_at(100.0), 1);
        assert_eq!(file.frame_index_at(10_000.0), 2);
        // last (200 ms) + average interval (100 ms)
        assert!((file.duration_s() - 0.3).abs() < 1e-9);
    }
}
