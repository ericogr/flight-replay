// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/decode.go (Apache-2.0, Copyright 2026 EricoGR), whose decoding
// comes from fpv-osd-decoder (MIT, Copyright (c) 2026 EricoGR).

//! Extracts numeric values and texts from INAV OSD screens.
//!
//! The OSD does not store numbers. It stores the screen the pilot saw, frame by frame, as a grid of font glyphs.
//! The decoder recognizes each field by its icon and unit. A field that is not readable in a frame (blinking, no
//! GPS, splash screen) is absent from the sample: it never becomes zero.

mod generic;
mod layout;
mod leftover;
mod rule;
mod rules;
mod screen;
mod units;

use std::collections::HashMap;
use std::fmt;

pub use generic::{EXTRA_PREFIX, extra_fields, is_extra_key};
pub use layout::{detect_flight_modes, detect_message_row};
pub use leftover::Leftover;

use self::rule::{Rule, glyph_char};
use self::screen::{Screen, screen_of};
use super::catalog::{KEY_FLIGHT_MODE, KEY_MESSAGE};
use super::inav;
use crate::osd::{Frame, OsdFile};

/// A telemetry value read from a frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A number, in the unit of its column.
    Num(f64),
    /// A text.
    Text(String),
}

/// What was read from a frame, by column key. A field that was not readable is absent: it never becomes zero.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sample(HashMap<String, Value>);

impl Sample {
    /// Stores a number.
    pub fn num(&mut self, key: &str, value: f64) {
        self.0.insert(key.to_owned(), Value::Num(value));
    }

    /// Stores a value.
    pub fn insert(&mut self, key: String, value: Value) {
        self.0.insert(key, value);
    }

    /// The value of a column.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    /// The numeric value of a column.
    #[must_use]
    pub fn number(&self, key: &str) -> Option<f64> {
        match self.0.get(key) {
            Some(Value::Num(value)) => Some(*value),
            _ => None,
        }
    }

    /// Whether the column has a value.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Number of columns with a value.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no column has a value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The keys of the columns that have a value.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    /// The columns and their values.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }

    /// Whether the sample has any of the columns of `other`.
    pub(crate) fn has_any(&self, other: &Sample) -> bool {
        other.0.keys().any(|key| self.0.contains_key(key))
    }
}

impl IntoIterator for Sample {
    type Item = (String, Value);
    type IntoIter = std::collections::hash_map::IntoIter<String, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// A strip of cells on one row of the OSD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Region {
    /// First column.
    pub x: usize,
    /// Row.
    pub y: usize,
    /// Number of cells.
    pub width: usize,
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{}", self.x, self.y, self.width)
    }
}

/// Where to find the fields that have no anchor glyph and therefore depend on the layout configured in INAV.
/// [`detect_flight_modes`] and [`detect_message_row`] find them in a recording.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// The strips where the flight mode is shown, one for each OSD layout that shows it. With more than one, a
    /// strip only counts when it holds a text the flight mode element can show.
    pub flight_modes: Vec<Region>,
    /// The row of the status messages. `None` turns them off.
    pub message_row: Option<usize>,
}

/// Turns OSD frames into telemetry samples.
pub struct Decoder {
    options: Options,
    rules: Vec<Rule>,
}

impl Decoder {
    /// Creates a decoder.
    #[must_use]
    pub fn new(options: Options) -> Self {
        Self {
            options,
            rules: rules::build_rules(),
        }
    }

    /// Reads every recognized field in a frame, and also returns what was left on the screen unrecognized (not
    /// counting the artificial horizon, the crosshair and the variometer, which are graphics).
    ///
    /// The post-flight statistics screen is not read at all: its numbers are summaries of the flight, not what the
    /// aircraft was doing at that moment. The splash screen shown while arming is read by the rules, since it
    /// does show the position, but not by the generic pass, which would only find the version and the totals it
    /// prints.
    #[must_use]
    pub fn decode(&self, file: &OsdFile, frame: &Frame) -> (Sample, Vec<Leftover>) {
        let kind = screen_of(file, frame);
        let mut sample = Sample::default();
        let mut left = Vec::new();
        for y in 0..file.rows {
            let row = frame.row(y);
            let (mut line, blank) = row_text(row);
            if blank {
                continue;
            }
            if kind != Screen::Stats {
                line = self.read_row(y as usize, row, line, &mut sample, kind == Screen::Flight);
            }
            leftover::append_leftovers(&mut left, y as usize, &line);
        }
        (sample, left)
    }

    /// Reads what a row of the screen holds, in order: the fields the rules know; then, on the pilot's own screen
    /// (flight), the numbers next to glyphs no rule knows and last the text, which is what is left. The flight mode
    /// and the messages are not on the other screens: what a splash screen shows in those places is not them.
    /// Returns the line without what was read.
    pub(crate) fn read_row(
        &self,
        y: usize,
        cells: &[u16],
        line: String,
        sample: &mut Sample,
        flight: bool,
    ) -> String {
        let line = self.decode_row(line, sample);
        if !flight {
            return line;
        }
        let line = generic::read_generic(&line, sample);
        self.read_text_fields(y, cells, line, sample)
    }

    /// Applies the rules in order to one row and returns the row with everything recognized erased. Each
    /// recognized span disappears before the next rule runs, so generic rules (a speed without an icon, for
    /// example) cannot reuse what a more specific field already consumed.
    fn decode_row(&self, mut line: String, sample: &mut Sample) -> String {
        for rule in &self.rules {
            line = rule.apply(&line, sample);
        }
        line
    }

    /// Reads the fields that have no anchor glyph (flight mode and status messages) by position, and erases them
    /// from the line.
    fn read_text_fields(
        &self,
        y: usize,
        row: &[u16],
        mut line: String,
        sample: &mut Sample,
    ) -> String {
        for region in &self.options.flight_modes {
            if y != region.y || region.width == 0 || region.x >= row.len() {
                continue;
            }
            let end = (region.x + region.width).min(row.len());
            line = blank_chars(&line, region.x, end);
            if sample.contains(KEY_FLIGHT_MODE) {
                continue;
            }
            let (strip, _) = row_text(&row[region.x..end]);
            if self.options.flight_modes.len() > 1 && !inav::is_flight_mode(&strip) {
                continue;
            }
            let text = plain_text(&row[region.x..end]);
            if !text.is_empty() {
                sample.insert(KEY_FLIGHT_MODE.to_owned(), Value::Text(text));
            }
        }
        if self.options.message_row == Some(y) {
            // From what the rules left, so that another element on the row does not end up in the message.
            let (text, rest) = take_text(&line);
            line = rest;
            if !text.is_empty() {
                sample.insert(KEY_MESSAGE.to_owned(), Value::Text(text));
            }
        }
        line
    }
}

/// Turns the cells of a row into a string with one character per cell holding the glyph code (an empty cell
/// becomes a blank), and says whether the row is empty.
pub(crate) fn row_text(row: &[u16]) -> (String, bool) {
    let mut line = String::with_capacity(row.len());
    let mut blank = true;
    for &cell in row {
        let cell = if cell == 0 { u16::from(b' ') } else { cell };
        if cell != u16::from(b' ') {
            blank = false;
        }
        line.push(glyph_char(cell));
    }
    (line, blank)
}

fn blank_chars(line: &str, from: usize, to: usize) -> String {
    line.chars()
        .enumerate()
        .map(|(index, c)| if index >= from && index < to { ' ' } else { c })
        .collect()
}

/// Whether a cell holds a real ASCII character: a letter, digit or punctuation. Font glyphs, including those INAV
/// puts in ASCII codes such as `k`, `v` or `~`, are not text.
fn is_text(code: u16) -> bool {
    code > 0x20 && code < 0x7f && inav::name(code).is_none()
}

/// The ASCII text of the cells, with the blanks squeezed.
fn plain_text(cells: &[u16]) -> String {
    let bytes: String = cells
        .iter()
        .map(|&code| {
            if is_text(code) {
                char::from(code as u8)
            } else {
                ' '
            }
        })
        .collect();
    bytes.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The ASCII text on the line, with the blanks squeezed, and the line without that text. Whatever is not text
/// stays on the line.
pub(crate) fn take_text(line: &str) -> (String, String) {
    let mut found = String::new();
    let mut rest = String::new();
    for character in line.chars() {
        let code = character as u32;
        if code <= 0xffff && is_text(code as u16) {
            found.push(character);
            rest.push(' ');
        } else {
            found.push(' ');
            rest.push(character);
        }
    }
    (found.split_whitespace().collect::<Vec<_>>().join(" "), rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squeezes_text_and_leaves_glyphs() {
        let glyph = glyph_char(inav::ICON_RSSI);
        let line = format!("EN  ROUTE{glyph}TO");
        let (text, rest) = take_text(&line);
        assert_eq!(text, "EN ROUTE TO");
        assert!(rest.contains(glyph));
        assert!(!rest.contains('E'));
    }

    #[test]
    fn plain_text_ignores_font_glyphs() {
        // 'v' is the metres glyph of the font, not text
        assert_eq!(
            plain_text(&[u16::from(b'A'), u16::from(b'C'), 0, u16::from(b'v')]),
            "AC"
        );
    }

    #[test]
    fn row_text_marks_blank_rows() {
        assert!(row_text(&[0, 0x20, 0]).1);
        assert!(!row_text(&[0, u16::from(b'A')]).1);
    }
}
