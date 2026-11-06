// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/leftover.go (Apache-2.0, Copyright 2026 EricoGR).

use crate::telemetry::inav;

/// A piece of the screen that no rule recognized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leftover {
    /// Column of the first cell.
    pub x: usize,
    /// Row.
    pub y: usize,
    /// The text of the cells. Named glyphs appear as `{NAME}`.
    pub text: String,
}

fn glyph_text(code: u16) -> String {
    let name = inav::name(code);
    if (0x20..0x7f).contains(&code) && name.is_none() {
        return char::from(code as u8).to_string();
    }
    match name {
        Some(name) => format!("{{{name}}}"),
        None => format!("{{{code:#x}}}"),
    }
}

/// Splits a row into contiguous runs of occupied cells and appends those that are not purely graphic.
pub(super) fn append_leftovers(out: &mut Vec<Leftover>, y: usize, line: &str) {
    let mut text = String::new();
    let (mut start, mut graphic, mut count) = (0, true, 0);
    let mut flush = |text: &mut String, start: usize, graphic: bool, count: usize| {
        if count > 0 && !graphic {
            out.push(Leftover {
                x: start,
                y,
                text: std::mem::take(text),
            });
        }
        text.clear();
    };
    for (x, character) in line.chars().enumerate() {
        let code = character as u32 as u16;
        if character == ' ' {
            flush(&mut text, start, graphic, count);
            graphic = true;
            count = 0;
        } else {
            if count == 0 {
                start = x;
            }
            count += 1;
            text.push_str(&glyph_text(code));
            graphic = graphic && inav::is_graphic(code);
        }
    }
    flush(&mut text, start, graphic, count);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_runs_that_are_not_graphics() {
        let mut out = Vec::new();
        let line = format!(
            "  AB  {}  ",
            char::from_u32(u32::from(inav::ICON_RSSI)).unwrap()
        );
        append_leftovers(&mut out, 3, &line);
        assert_eq!(out.len(), 2);
        assert_eq!(
            out[0],
            Leftover {
                x: 2,
                y: 3,
                text: "AB".into()
            }
        );
        assert_eq!(out[1].text, "{ICON_RSSI}");
        assert_eq!(out[1].x, 6);
    }

    #[test]
    fn skips_purely_graphic_runs() {
        let mut out = Vec::new();
        let line: String = [0x12c_u32, 0x12d]
            .iter()
            .map(|&c| char::from_u32(c).unwrap())
            .collect();
        append_leftovers(&mut out, 0, &line);
        assert_eq!(out, Vec::new());
    }
}
