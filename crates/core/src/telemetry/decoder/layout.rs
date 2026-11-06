// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/layout.go (Apache-2.0, Copyright 2026 EricoGR).

//! The flight mode and the status messages are text with no glyph to anchor them, so where they are depends on
//! the layout the pilot set up in INAV. They can be found in the recording itself, which is what this file does.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::LazyLock;

use regex::Regex;

use super::screen::{Screen, screen_of};
use super::{Decoder, Options, Region, row_text, take_text};
use crate::osd::OsdFile;
use crate::telemetry::inav;

/// How much of the best candidate's support a flight mode position needs to count as a layout of its own: a
/// message that happens to hold a mode name (`(HOLD)`) shows up far less often than the element does.
const MIN_SHARE: usize = 5; // one in five

/// A message has a word of at least three letters, which leaves out labels such as `M0` and the two-letter codes
/// of the radio.
static WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("[A-Z]{3}").expect("valid expression"));

/// How many different texts a row must show to be taken for the messages. Text that never changes is a name, not a
/// message.
const MIN_MESSAGE_TEXTS: usize = 2;

/// Finds where the recording shows the flight mode. The element is always four characters and always one of a
/// short list of names, and it is on screen in nearly every frame, so the positions where those names appear most
/// often are it. The best supported position comes first. Returns nothing when no frame shows one.
#[must_use]
pub fn detect_flight_modes(file: &OsdFile) -> Vec<Region> {
    let mut hits: HashMap<Region, usize> = HashMap::new();
    for frame in &file.frames {
        if screen_of(file, frame) != Screen::Flight {
            continue;
        }
        for y in 0..file.rows {
            let row = frame.row(y);
            for x in 0..=row.len().saturating_sub(inav::FLIGHT_MODE_WIDTH) {
                if row.len() >= inav::FLIGHT_MODE_WIDTH && mode_at(row, x) {
                    *hits
                        .entry(Region {
                            x,
                            y: y as usize,
                            width: inav::FLIGHT_MODE_WIDTH,
                        })
                        .or_default() += 1;
                }
            }
        }
    }
    let mut regions: Vec<Region> = hits.keys().copied().collect();
    regions.sort_by(|a, b| {
        hits[b]
            .cmp(&hits[a])
            .then(a.y.cmp(&b.y))
            .then(a.x.cmp(&b.x))
    });
    if let Some(best) = regions.first().map(|region| hits[region]) {
        for (index, region) in regions.iter().enumerate() {
            if hits[region] * MIN_SHARE < best {
                regions.truncate(index);
                break;
            }
        }
    }
    regions
}

/// Whether the cells from `x` on are the name of a flight mode, set apart from the text around it: a mode name
/// inside a longer word is not the element.
fn mode_at(row: &[u16], x: usize) -> bool {
    let end = x + inav::FLIGHT_MODE_WIDTH;
    if x > 0 && is_alphanumeric(row[x - 1]) || end < row.len() && is_alphanumeric(row[end]) {
        return false;
    }
    let mut name = String::with_capacity(inav::FLIGHT_MODE_WIDTH);
    for &cell in &row[x..end] {
        let cell = if cell == 0 { u16::from(b' ') } else { cell };
        if cell > 0x7e {
            return false;
        }
        name.push(char::from(cell as u8));
    }
    inav::is_flight_mode(&name)
}

/// Whether a cell holds a digit or a letter of INAV's text, which is uppercase: the lowercase codes are font
/// glyphs.
fn is_alphanumeric(code: u16) -> bool {
    matches!(code, 0x30..=0x39 | 0x41..=0x5a)
}

/// Finds the row of the status messages, given where the flight mode is. Messages are free text, so there is no
/// list of names to look for: they are the text that changes. Of the text left on the screen after the rules have
/// read their elements, the row that shows the most different texts is the messages row. It is only taken when no
/// other row shows as many different texts and when the texts really change, since a wrong row is worse than none.
#[must_use]
pub fn detect_message_row(file: &OsdFile, modes: &[Region]) -> Option<usize> {
    let decoder = Decoder::new(Options {
        flight_modes: modes.to_vec(),
        message_row: None,
    });
    let mut texts: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
    for frame in &file.frames {
        if screen_of(file, frame) != Screen::Flight {
            continue;
        }
        for y in 0..file.rows {
            let cells = frame.row(y);
            let (line, blank) = row_text(cells);
            if blank || !WORD_RE.is_match(&line) {
                continue;
            }
            let mut scratch = super::Sample::default();
            let (text, _) =
                take_text(&decoder.read_row(y as usize, cells, line, &mut scratch, true));
            // a plus code (8FRP4JQ7+M5) changes with every move
            if !WORD_RE.is_match(&text) || text.contains('+') {
                continue;
            }
            texts.entry(y as usize).or_default().insert(text);
        }
    }

    let mut best: Option<usize> = None;
    let mut runner_up = 0;
    for (&y, set) in &texts {
        let count = set.len();
        match best {
            None => best = Some(y),
            Some(current) if count > texts[&current].len() => {
                runner_up = texts[&current].len();
                best = Some(y);
            }
            Some(_) if count > runner_up => runner_up = count,
            Some(_) => {}
        }
    }
    let best = best?;
    let best_count = texts[&best].len();
    (best_count >= MIN_MESSAGE_TEXTS && best_count != runner_up).then_some(best)
}
