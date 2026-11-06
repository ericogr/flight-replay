// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osdsource/osdsource.go (Apache-2.0, Copyright 2026 EricoGR).

//! Reads the telemetry straight from the `.osd` recording of the flight, so no CSV is needed.

use std::path::Path;

use super::decoder::{Decoder, Options, Value, detect_flight_modes, detect_message_row};
use super::table::{Row, Table, TableError};
use crate::osd::{OsdError, OsdFile};
use crate::paths::file_name;

/// What to do when loading.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoadOptions {
    /// Also look for the flight mode and the status messages. They are text with no icon to anchor them, so finding
    /// them means scanning the whole recording. The sound model does not use them; only the probe shows them.
    pub detect_layout: bool,
}

/// Decoded `.osd` telemetry, on one timeline. A video made of several recordings is decoded from the joined file
/// (see [`crate::osd::join`]), so it is one recording like any other.
#[derive(Debug, Clone)]
pub struct Recording {
    /// The readings.
    pub table: Table,
    /// Columns of the OSD grid.
    pub columns: u32,
    /// Rows of the OSD grid.
    pub rows: u32,
    /// OSD frames in the file.
    pub frames: usize,
    /// Where the flight mode and messages were found (empty unless layout detection was asked for).
    pub layout: String,
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
}

impl Recording {
    /// Reads and decodes an `.osd` file.
    pub fn load(path: &Path, options: LoadOptions) -> Result<Self, RecordingError> {
        let file = OsdFile::open(path).map_err(|source| RecordingError::Osd {
            path: file_name(path),
            source,
        })?;
        Self::decode(&file, options)
    }

    /// Turns a parsed `.osd` file into a recording, one row per frame. A reading that is not legible in a frame
    /// (blinking element, splash or statistics screen) is absent from its row, never zero.
    pub fn decode(file: &OsdFile, options: LoadOptions) -> Result<Self, RecordingError> {
        let (rows, layout) = decode_rows(file, options);
        Ok(Self {
            table: Table::new(rows)?,
            columns: file.columns,
            rows: file.rows,
            frames: file.frames.len(),
            layout,
        })
    }
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
