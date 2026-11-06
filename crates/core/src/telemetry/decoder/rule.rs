// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/rule.go (Apache-2.0, Copyright 2026 EricoGR). The regular
// expressions run on a line where each OSD cell is one character holding the glyph code, so `\x{NN}` matches
// glyph NN. `\b` and `\B` are written `(?-u:\b)` and `(?-u:\B)`: Go's are ASCII-only, and the Unicode ones would
// treat the accented-letter codes of the font as word characters.

use regex::Regex;

use super::{Sample, Value};
use crate::telemetry::inav;

/// What a matched rule stores into the sample. `groups[0]` is the matched text and the others are the capture
/// groups of the rule's expression (empty when a group did not take part in the match).
pub(crate) type Emit = Box<dyn Fn(&[String], &mut Sample) + Send + Sync>;

/// Recognizes one field on a row: a regular expression over the glyph codes and the function that turns a match
/// into sample values.
pub(crate) struct Rule {
    re: Regex,
    emit: Emit,
}

impl Rule {
    pub(crate) fn new(re: Regex, emit: Emit) -> Self {
        Self { re, emit }
    }

    /// Runs the rule on every place of `line` where it matches. A match emits its values and is erased from the
    /// line that comes back; one that does not match leaves the line unchanged.
    ///
    /// A match that would store a column the sample already has is left alone: the first reading in screen order
    /// wins and the other one stays on the line for the generic pass. Some INAV elements look exactly alike on
    /// screen (the capacity used and the capacity left, for example), so glyphs alone cannot tell them apart.
    pub(crate) fn apply(&self, line: &str, sample: &mut Sample) -> String {
        let mut spans: Vec<(usize, usize)> = Vec::new();
        for captures in self.re.captures_iter(line) {
            let groups: Vec<String> = (0..captures.len())
                .map(|index| {
                    captures
                        .get(index)
                        .map_or_else(String::new, |m| m.as_str().to_owned())
                })
                .collect();
            let mut got = Sample::default();
            (self.emit)(&groups, &mut got);
            if sample.has_any(&got) {
                continue;
            }
            for (key, value) in got {
                sample.insert(key, value);
            }
            spans.push(whole_match(&captures));
        }
        erase(line, &spans)
    }
}

/// The byte offsets of the whole text a regex matched.
pub(crate) fn whole_match(captures: &regex::Captures<'_>) -> (usize, usize) {
    let whole = captures
        .get(0)
        .expect("group 0 always takes part in a match");
    (whole.start(), whole.end())
}

/// Replaces the spans (byte offsets, in order) of `line` with one blank per glyph.
pub(crate) fn erase(line: &str, spans: &[(usize, usize)]) -> String {
    let mut out = String::with_capacity(line.len());
    let mut last = 0;
    for &(from, to) in spans {
        out.push_str(&line[last..from]);
        out.extend(std::iter::repeat_n(' ', line[from..to].chars().count()));
        last = to;
    }
    out.push_str(&line[last..]);
    out
}

/// The glyph codes of a matched span.
pub(crate) fn codes(text: &str) -> Vec<u16> {
    text.chars().map(|c| c as u32 as u16).collect()
}

/// The code of the first glyph of a matched span.
pub(crate) fn first_glyph(text: &str) -> u16 {
    text.chars().next().map_or(0, |c| c as u32 as u16)
}

/// Reads an INAV number (digits, sign, point, and digits with half a point) from a matched span.
pub(crate) fn parse_num(text: &str) -> Option<f64> {
    inav::text(&codes(text)).parse::<f64>().ok()
}

/// The character that stands for a glyph code in a row line.
pub(crate) fn glyph_char(code: u16) -> char {
    char::from_u32(u32::from(code)).unwrap_or('\u{FFFD}')
}

// Regular expression fragments. Each OSD cell is a character holding the glyph code.

/// Optional blanks.
pub(crate) const SPACE_RE: &str = " *";

/// An INAV number: ASCII digits, sign, point and the digits with half a decimal point (`3.` and `.6`). It is one
/// capture group.
pub(crate) const NUM_RE: &str = r"(-?[0-9.\x{a1}-\x{aa}\x{b1}-\x{ba}]+)";

/// ASCII word boundary and its negation (Go semantics).
pub(crate) const WORD_BOUNDARY: &str = r"(?-u:\b)";
pub(crate) const NOT_WORD_BOUNDARY: &str = r"(?-u:\B)";

/// The expression of one glyph.
pub(crate) fn sym(code: u16) -> String {
    format!(r"\x{{{code:x}}}")
}

/// A character class of glyphs.
pub(crate) fn syms(codes: &[u16]) -> String {
    let mut class = String::from("[");
    for &code in codes {
        class.push_str(&sym(code));
    }
    class.push(']');
    class
}

/// A character class of the glyphs from `low` to `high`.
pub(crate) fn sym_range(low: u16, high: u16) -> String {
    format!(r"[\x{{{low:x}}}-\x{{{high:x}}}]")
}

/// A capture group matching any of the unit glyphs of a table.
pub(crate) fn units_re(units: &[(u16, f64)]) -> String {
    let mut codes: Vec<u16> = units.iter().map(|&(code, _)| code).collect();
    codes.sort_unstable();
    format!("({})", syms(&codes))
}

/// Joins the parts into one expression. The rules are fixed text, so a failure is a programming error.
pub(crate) fn rx(parts: &[&str]) -> Regex {
    let pattern = parts.concat();
    Regex::new(&pattern)
        .unwrap_or_else(|error| panic!("invalid rule expression {pattern:?}: {error}"))
}

pub(crate) fn rule(parts: &[&str], emit: Emit) -> Rule {
    Rule::new(rx(parts), emit)
}

/// The conversion factor of a unit glyph, from a table.
pub(crate) fn factor(units: &[(u16, f64)], glyph: u16) -> f64 {
    units
        .iter()
        .find(|&&(code, _)| code == glyph)
        .map_or(0.0, |&(_, factor)| factor)
}

/// A number followed by a unit glyph, with an optional icon before it: the icon, blanks, the number and the unit.
/// The value is the number times the factor the table gives for the unit glyph. `prefix` is the expression of the
/// icon and has no capture group of its own. With no prefix the reading has no icon, which makes it the generic
/// one for its units.
pub(crate) fn scaled_reading(
    key: &'static str,
    units: &'static [(u16, f64)],
    prefix: &[&str],
) -> Rule {
    let units_group = units_re(units);
    let mut parts: Vec<&str> = prefix.to_vec();
    if !prefix.is_empty() {
        parts.push(SPACE_RE);
    }
    parts.push(NUM_RE);
    parts.push(&units_group);
    rule(&parts, scaled(key, 1, 2, units))
}

// Emitters shared by several rules. `index` is the capture group they read.

/// Emits the INAV number in group `index`.
pub(crate) fn num_field(key: &'static str, index: usize) -> Emit {
    Box::new(move |groups, sample| {
        if let Some(value) = parse_num(&groups[index]) {
            sample.num(key, value);
        }
    })
}

/// Emits the plain integer in group `index`.
pub(crate) fn int_field(key: &'static str, index: usize) -> Emit {
    Box::new(move |groups, sample| {
        if let Ok(value) = groups[index].parse::<i64>() {
            sample.num(key, value as f64);
        }
    })
}

/// Emits the text in group `index`, without the padding blanks.
pub(crate) fn text_field(key: &'static str, index: usize) -> Emit {
    Box::new(move |groups, sample| {
        let text = groups[index]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !text.is_empty() {
            sample.insert(key.to_owned(), Value::Text(text));
        }
    })
}

/// For the rules that recognize an element only to take it off the screen: it is there, but shows nothing to read.
pub(crate) fn no_reading() -> Emit {
    Box::new(|_, _| {})
}

/// Runs several emitters on the same match.
pub(crate) fn each(emitters: Vec<Emit>) -> Emit {
    Box::new(move |groups, sample| {
        for emit in &emitters {
            emit(groups, sample);
        }
    })
}

/// Emits `key = number * unit factor`, where `groups[number]` is the number and `groups[unit]` the unit glyph.
pub(crate) fn scaled(
    key: &'static str,
    number: usize,
    unit: usize,
    units: &'static [(u16, f64)],
) -> Emit {
    Box::new(move |groups, sample| {
        if let Some(value) = parse_num(&groups[number]) {
            sample.num(key, value * factor(units, first_glyph(&groups[unit])));
        }
    })
}

/// Emits the number in group 2, negated when the icon in group 1 is the given one.
pub(crate) fn signed_by_icon(key: &'static str, negative: u16) -> Emit {
    Box::new(move |groups, sample| {
        let Some(value) = parse_num(&groups[2]) else {
            return;
        };
        let value = if first_glyph(&groups[1]) == negative {
            -value
        } else {
            value
        };
        sample.num(key, value);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erases_one_blank_per_glyph() {
        // ASCII cells: byte offsets are character positions.
        assert_eq!(erase("ab\u{2}\u{3}cd", &[(2, 4)]), "ab  cd");
        // A glyph code above 0x7f takes two bytes in the line, but it is still one blank.
        let line = "a\u{1a1}b\u{1a2}c";
        let second = line.find('\u{1a2}').unwrap();
        assert_eq!(erase(line, &[(1, 3), (second, second + 2)]), "a b c");
    }

    #[test]
    fn builds_glyph_expressions() {
        assert_eq!(sym(0x13), r"\x{13}");
        assert_eq!(syms(&[1, 0xa]), r"[\x{1}\x{a}]");
        assert_eq!(sym_range(0x10, 0x1f), r"[\x{10}-\x{1f}]");
    }

    #[test]
    fn reads_numbers_with_half_points() {
        // 3.69 is [3.][.6]9
        let text: String = [
            inav::TRAILING_DOT_FIRST + 3,
            inav::LEADING_DOT_FIRST + 6,
            u16::from(b'9'),
        ]
        .into_iter()
        .map(glyph_char)
        .collect();
        assert_eq!(parse_num(&text), Some(3.69));
        assert_eq!(parse_num("-12"), Some(-12.0));
        assert_eq!(parse_num("-"), None);
    }

    #[test]
    fn a_rule_takes_the_first_reading_and_leaves_the_second() {
        let rule = rule(
            &[NUM_RE, &sym(inav::UNIT_AMP)],
            num_field("osd_current_a", 1),
        );
        let line: String = format!(
            "{}{}   {}{}",
            "5",
            glyph_char(inav::UNIT_AMP),
            "7",
            glyph_char(inav::UNIT_AMP)
        );
        let mut sample = Sample::default();
        let rest = rule.apply(&line, &mut sample);
        assert_eq!(sample.get("osd_current_a"), Some(&Value::Num(5.0)));
        // the first span is erased, the second stays on the line
        assert!(rest.contains('7'));
        assert!(!rest.contains('5'));
    }
}
