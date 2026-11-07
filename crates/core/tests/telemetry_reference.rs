// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Compares the decoder with the CSV that fpv-osd-decoder (the Go project) wrote from the same `.osd` file.
//!
//! The test needs `FLIGHT_REPLAY_DECODER_SAMPLES`, the folder of fpv-osd-decoder with `AvatarG0031.osd` and
//! `AvatarG0031.csv`. Without it, the test prints a message and passes.

use std::path::PathBuf;
use std::time::Instant;

use flight_replay_core::osd::OsdFile;
use flight_replay_core::telemetry::catalog::{CATALOG, Field, KEY_THROTTLE_AUTO, KEY_THROTTLE_PCT};
use flight_replay_core::telemetry::decoder::{
    Decoder, Options, Sample, Value, detect_flight_modes, detect_message_row,
};

/// Reads the number the way the Go exporter writes it: rounded to `decimals` places with the trailing zeros
/// removed, or the shortest form for a negative number of decimals.
fn format_number(value: f64, decimals: i32) -> String {
    if decimals < 0 {
        return format!("{value}");
    }
    let mut text = format!("{value:.*}", decimals as usize);
    if text.contains('.') {
        text = text.trim_end_matches('0').trim_end_matches('.').to_owned();
    }
    if text == "-0" || value.is_nan() {
        return "0".into();
    }
    text
}

fn cell(sample: &Sample, field: &Field) -> String {
    match sample.get(&field.key) {
        None => String::new(),
        Some(Value::Text(text)) => text.clone(),
        Some(Value::Num(value)) => format_number(*value, field.decimals),
    }
}

/// The deliberate differences from the Go decoder (see docs/telemetry/inav.md).
///
/// - Where INAV shows STOP, IDLE or DARM, or a negative percentage, in place of the throttle, Go reads nothing and
///   this decoder reads 0 (or the negative percentage, with 0 for "manual" in the auto-throttle column).
/// - The statistics screen of INAV 7 and earlier is titled `--- STATS ---`, which Go does not know, so it reads the
///   maximums and averages of the flight as readings. This decoder skips the screen: it reads nothing where Go reads
///   something. That is only allowed on a frame that shows the title, and nothing else is.
///
/// Each only applies where the other decoder reads nothing, or where a statistics screen is shown, so a reading that
/// both make and differ on is still a mismatch.
fn is_known_difference(key: &str, expected: &str, got: &str, statistics_screen: bool) -> bool {
    if statistics_screen {
        return got.is_empty() && !expected.is_empty();
    }
    expected.is_empty()
        && match key {
            KEY_THROTTLE_PCT => got == "0" || got.starts_with('-'),
            KEY_THROTTLE_AUTO => got == "0",
            _ => false,
        }
}

/// Whether the frame is the statistics screen of an INAV before 8, which Go reads as the flight.
fn is_old_statistics_screen(frame: &flight_replay_core::osd::Frame, rows: u32) -> bool {
    (0..rows).any(|y| {
        let row = frame.row(y);
        let title: Vec<u16> = "--- STATS".bytes().map(u16::from).collect();
        row.windows(title.len()).any(|window| window == title)
    })
}

/// A small CSV reader: `;` separated, with double-quoted fields.
fn read_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (';', false) => row.push(std::mem::take(&mut field)),
            ('\n', false) => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            ('\r', false) => {}
            (c, _) => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

#[test]
fn the_decoder_reproduces_the_reference_csv() {
    let Some(dir) = std::env::var_os("FLIGHT_REPLAY_DECODER_SAMPLES").map(PathBuf::from) else {
        eprintln!("skipping: FLIGHT_REPLAY_DECODER_SAMPLES is not set");
        return;
    };
    let osd = OsdFile::open(&dir.join("AvatarG0031.osd")).unwrap();
    let csv = read_csv(&std::fs::read_to_string(dir.join("AvatarG0031.csv")).unwrap());
    let header = &csv[0];
    let rows = &csv[1..];
    assert_eq!(rows.len(), osd.frames.len(), "one CSV row per OSD frame");

    let started = Instant::now();
    let modes = detect_flight_modes(&osd);
    let message_row = detect_message_row(&osd, &modes);
    eprintln!(
        "layout: modes {modes:?}, message row {message_row:?}, found in {:?}",
        started.elapsed()
    );
    let decoder = Decoder::new(Options {
        flight_modes: modes,
        message_row,
    });

    // The columns of the CSV that the decoder produces (the others come from the SRT file).
    let columns: Vec<(usize, &Field)> = header
        .iter()
        .enumerate()
        .filter(|(_, name)| name.starts_with("osd_"))
        .map(|(index, name)| {
            let field = CATALOG
                .iter()
                .find(|field| field.key == name.as_str())
                .unwrap_or_else(|| {
                    panic!("the CSV has the column {name}, which is not in the catalog")
                });
            (index, field)
        })
        .collect();
    assert!(columns.len() >= 30, "only {} columns", columns.len());

    let started = Instant::now();
    let mut mismatches: Vec<(String, usize, String, String)> = Vec::new();
    let mut per_column: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut compared = 0usize;
    let mut known_differences = 0usize;
    for (frame_index, frame) in osd.frames.iter().enumerate() {
        let (sample, _) = decoder.decode(&osd, frame);
        let statistics_screen = is_old_statistics_screen(frame, osd.rows);
        assert_eq!(
            rows[frame_index][0],
            frame.time_ms.to_string(),
            "frame {frame_index} time"
        );
        for &(index, field) in &columns {
            let expected = rows[frame_index][index].replace(',', ".");
            let expected = if field.text {
                rows[frame_index][index].clone()
            } else {
                expected
            };
            let got = cell(&sample, field);
            compared += usize::from(!expected.is_empty());
            if is_known_difference(&field.key, &expected, &got, statistics_screen) {
                known_differences += 1;
            } else if got != expected {
                *per_column.entry(field.key.to_string()).or_default() += 1;
                if mismatches.len() < 12 {
                    mismatches.push((field.key.to_string(), frame_index, expected, got));
                }
            }
        }
    }
    eprintln!(
        "decoded {} frames in {:?}; {compared} readings compared, {known_differences} that differ on purpose (see is_known_difference)",
        osd.frames.len(),
        started.elapsed()
    );
    assert!(
        compared > 100_000,
        "the CSV has too few readings ({compared}) for the test to mean anything"
    );
    for (key, frame, expected, got) in &mismatches {
        eprintln!("mismatch {key} at frame {frame}: expected {expected:?}, got {got:?}");
    }
    assert!(per_column.is_empty(), "mismatching columns: {per_column:?}");
}
