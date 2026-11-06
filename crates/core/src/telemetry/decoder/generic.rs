// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/generic.go (Apache-2.0, Copyright 2026 EricoGR).

//! The generic pass takes the readings no rule recognized: a number anchored by glyphs. Each becomes a column
//! named after those glyphs, with the value as shown on screen.

use std::fmt::Write as _;
use std::sync::LazyLock;

use regex::Regex;

use super::rule::{NUM_RE, erase, parse_num, whole_match};

use super::Sample;
use crate::telemetry::inav;

/// Starts the key of every column made by the generic pass.
pub const EXTRA_PREFIX: &str = "osd_raw_";

/// Joins the names of the glyphs in a column name. A glyph name has single underscores inside it and none at its
/// ends, so a double one cannot be mistaken for part of a name.
const GLYPH_SEPARATOR: &str = "__";

/// The character class of the glyph cells.
fn glyph_class() -> String {
    let mut class = String::from("[");
    let mut code = 1u16;
    while code <= inav::MAX_GLYPH {
        if !inav::is_glyph(code) {
            code += 1;
            continue;
        }
        let first = code;
        while code <= inav::MAX_GLYPH && inav::is_glyph(code) {
            code += 1;
        }
        let _ = write!(class, r"\x{{{first:x}}}-\x{{{:x}}}", code - 1);
    }
    class.push(']');
    class
}

/// A number with glyphs before it (an icon, perhaps with blanks after it), after it (a unit), or both. Groups:
/// 1 the glyphs before, 2 the number, 3 the glyphs after.
static GENERIC_RE: LazyLock<Regex> = LazyLock::new(|| {
    let class = glyph_class();
    Regex::new(&format!("({class}*) *{NUM_RE}({class}*)")).expect("valid generic expression")
});

/// Takes the readings no rule recognized and returns the line with them erased. A number with no glyph next to it
/// is not taken (it could be anything), nor is text, since there is no telling what it means. If the same glyphs
/// show a number twice, the first keeps the column and the other stays on the line.
pub(super) fn read_generic(line: &str, sample: &mut Sample) -> String {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for captures in GENERIC_RE.captures_iter(line) {
        let (before, number, after) = (&captures[1], &captures[2], &captures[3]);
        if before.is_empty() && after.is_empty() {
            continue;
        }
        let Some(value) = parse_num(number) else {
            continue;
        };
        let key = extra_key(&format!("{before}{after}"));
        if sample.contains(&key) {
            continue;
        }
        sample.num(&key, value);
        spans.push(whole_match(&captures));
    }
    erase(line, &spans)
}

/// The name of a reading's column after the glyphs around the number: `osd_raw_` and the names of the glyphs
/// joined by [`GLYPH_SEPARATOR`], such as `osd_raw_label_hdop_l__label_hdop_r`.
fn extra_key(glyphs: &str) -> String {
    let names: Vec<String> = glyphs.chars().map(|c| glyph_id(c as u32 as u16)).collect();
    format!("{EXTRA_PREFIX}{}", names.join(GLYPH_SEPARATOR))
}

/// The name of a glyph in lowercase letters, digits and single underscores, or its code when the glyph has no
/// name in the table.
fn glyph_id(code: u16) -> String {
    match inav::name(code) {
        None => format!("x{code:x}"),
        Some(name) => name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_lowercase().next().unwrap_or(c)
                } else {
                    '_'
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::decoder::rule::glyph_char;

    #[test]
    fn names_a_column_after_its_glyphs() {
        let key = extra_key(&format!(
            "{}{}",
            glyph_char(inav::LABEL_HDOP_LEFT),
            glyph_char(inav::LABEL_HDOP_RIGHT)
        ));
        assert_eq!(key, "osd_raw_label_hdop_l__label_hdop_r");
        assert_eq!(glyph_id(0x1fe), "x1fe");
    }

    #[test]
    fn reads_a_number_next_to_a_glyph_and_erases_it() {
        let line = format!(
            "{}  42{}",
            glyph_char(inav::ICON_PROFILE),
            glyph_char(inav::UNIT_AMP)
        );
        let mut sample = Sample::default();
        let rest = read_generic(&line, &mut sample);
        assert_eq!(sample.len(), 1);
        assert_eq!(rest.trim(), "");
    }

    #[test]
    fn a_bare_number_is_not_taken() {
        let mut sample = Sample::default();
        let rest = read_generic("  123  ", &mut sample);
        assert!(sample.is_empty());
        assert!(rest.contains("123"));
    }
}
