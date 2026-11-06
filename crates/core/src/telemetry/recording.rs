// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osdsource/osdsource.go (Apache-2.0, Copyright 2026 EricoGR).

//! Reads the telemetry straight from the `.osd` recording of the flight, so no CSV is needed.

use std::path::{Path, PathBuf};

use super::decoder::{Decoder, Options, Value, detect_flight_modes, detect_message_row};
use super::table::{Row, Table, TableError};
use crate::osd::{OsdError, OsdFile, shifts};

/// What to do when loading.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoadOptions {
    /// Also look for the flight mode and the status messages. They are text with no icon to anchor them, so finding
    /// them means scanning the whole recording. The sound model does not use them; only the probe shows them.
    pub detect_layout: bool,
}

/// One `.osd` file of a recording: where its frames sit on the telemetry timeline of the whole recording.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Path of the file, when it was read from disk.
    pub path: Option<PathBuf>,
    /// Time of the first OSD frame of the file, in seconds.
    pub start: f64,
    /// Time of the last OSD frame of the file, in seconds.
    pub end: f64,
    /// Number of OSD frames in the file.
    pub frames: usize,
}

/// Decoded `.osd` telemetry, on one timeline. A flight is one file; a video made by joining several clips has one
/// file per clip.
#[derive(Debug, Clone)]
pub struct Recording {
    /// The readings.
    pub table: Table,
    /// Columns of the OSD grid.
    pub columns: u32,
    /// Rows of the OSD grid.
    pub rows: u32,
    /// OSD frames in all the files.
    pub frames: usize,
    /// Where the flight mode and messages were found (empty unless layout detection was asked for).
    pub layout: String,
    /// One segment per `.osd` file, in the order given.
    pub segments: Vec<Segment>,
}

/// Errors when loading a recording.
#[derive(Debug, thiserror::Error)]
pub enum RecordingError {
    /// An `.osd` file could not be read.
    #[error("{path}: {source}")]
    Osd {
        /// The file.
        path: String,
        /// What went wrong.
        #[source]
        source: OsdError,
    },
    /// The readings could not be put on a timeline.
    #[error(transparent)]
    Table(#[from] TableError),
    /// No file was given.
    #[error("no .osd file given")]
    NoFiles,
    /// The files of one video have different grids.
    #[error(
        "{first} has an OSD of {first_columns}x{first_rows}, but {other} has one of {columns}x{rows}: the .osd files of one video must come from the same recording"
    )]
    GridMismatch {
        /// First file.
        first: String,
        /// Its columns.
        first_columns: u32,
        /// Its rows.
        first_rows: u32,
        /// The file that differs.
        other: String,
        /// Its columns.
        columns: u32,
        /// Its rows.
        rows: u32,
    },
}

impl Recording {
    /// Reads and decodes an `.osd` file.
    pub fn load(path: &Path, options: LoadOptions) -> Result<Self, RecordingError> {
        Self::load_joined(&[path.to_path_buf()], options, 0.0)
    }

    /// Reads the `.osd` files of one video, in the order they play, and puts them on one telemetry timeline. The
    /// first file keeps its own times; each following file starts `gap` seconds after the last frame of the one
    /// before it, so the telemetry of a joined video is one continuous timeline. With a single file the result is
    /// that of [`Recording::load`].
    pub fn load_joined(
        paths: &[PathBuf],
        options: LoadOptions,
        gap: f64,
    ) -> Result<Self, RecordingError> {
        if paths.is_empty() {
            return Err(RecordingError::NoFiles);
        }
        let mut files = Vec::with_capacity(paths.len());
        for path in paths {
            files.push(OsdFile::open(path).map_err(|source| RecordingError::Osd {
                path: path.display().to_string(),
                source,
            })?);
        }
        let shifts = shifts(&files.iter().collect::<Vec<_>>(), gap);
        let mut rows: Vec<Row> = Vec::new();
        let mut segments = Vec::new();
        let mut layouts = Vec::new();
        let mut grid: Option<(u32, u32)> = None;
        for ((path, file), shift) in paths.iter().zip(&files).zip(shifts) {
            match grid {
                None => grid = Some((file.columns, file.rows)),
                Some((columns, rows_)) if (columns, rows_) != (file.columns, file.rows) => {
                    return Err(RecordingError::GridMismatch {
                        first: base_name(&paths[0]),
                        first_columns: columns,
                        first_rows: rows_,
                        other: base_name(path),
                        columns: file.columns,
                        rows: file.rows,
                    });
                }
                Some(_) => {}
            }
            let (part, layout) = decode_rows(file, options);
            let first = part.iter().map(|r| r.time).fold(f64::INFINITY, f64::min);
            let last = part
                .iter()
                .map(|r| r.time)
                .fold(f64::NEG_INFINITY, f64::max);
            segments.push(Segment {
                path: Some(path.clone()),
                start: first + shift,
                end: last + shift,
                frames: part.len(),
            });
            rows.extend(part.into_iter().map(|mut row| {
                row.time += shift;
                row
            }));
            if !layout.is_empty() {
                layouts.push(if paths.len() > 1 {
                    format!("{}: {layout}", base_name(path))
                } else {
                    layout
                });
            }
        }
        let (columns, grid_rows) = grid.expect("at least one file was read");
        let frames = rows.len();
        Ok(Self {
            table: Table::new(rows)?,
            columns,
            rows: grid_rows,
            frames,
            layout: layouts.join(" | "),
            segments,
        })
    }

    /// Turns a parsed `.osd` file into a recording, one row per frame. A reading that is not legible in a frame
    /// (blinking element, splash or statistics screen) is absent from its row, never zero.
    pub fn decode(file: &OsdFile, options: LoadOptions) -> Result<Self, RecordingError> {
        let (rows, layout) = decode_rows(file, options);
        let table = Table::new(rows)?;
        let segment = Segment {
            path: None,
            start: table.times[0],
            end: table.duration(),
            frames: file.frames.len(),
        };
        Ok(Self {
            table,
            columns: file.columns,
            rows: file.rows,
            frames: file.frames.len(),
            layout,
            segments: vec![segment],
        })
    }
}

fn base_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Reads every frame of an `.osd` file into one telemetry row each.
fn decode_rows(file: &OsdFile, options: LoadOptions) -> (Vec<Row>, String) {
    let mut decoder_options = Options::default();
    let mut layout = String::new();
    if options.detect_layout {
        decoder_options.flight_modes = detect_flight_modes(file);
        decoder_options.message_row = detect_message_row(file, &decoder_options.flight_modes);
        layout = describe_layout(&decoder_options);
    }
    let decoder = Decoder::new(decoder_options);
    let rows = file
        .frames
        .iter()
        .map(|frame| {
            let (sample, _) = decoder.decode(file, frame);
            let mut row = Row {
                time: f64::from(frame.time_ms) / 1000.0,
                ..Row::default()
            };
            for (key, value) in sample {
                match value {
                    Value::Text(text) if !text.is_empty() => {
                        row.texts.insert(key, text);
                    }
                    Value::Text(_) => {}
                    Value::Num(number) => {
                        row.numbers.insert(key, number);
                    }
                }
            }
            row
        })
        .collect();
    (rows, layout)
}

fn describe_layout(options: &Options) -> String {
    let mode = if options.flight_modes.is_empty() {
        "flight mode not found".to_owned()
    } else {
        let regions: Vec<String> = options
            .flight_modes
            .iter()
            .map(ToString::to_string)
            .collect();
        format!("flight mode at {}", regions.join(" and "))
    };
    let messages = options.message_row.map_or_else(
        || "status messages not found".to_owned(),
        |row| format!("status messages on row {row}"),
    );
    format!("{mode}; {messages}")
}
