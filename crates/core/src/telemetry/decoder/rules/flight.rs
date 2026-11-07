// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/rules_flight.go (Apache-2.0, Copyright 2026 EricoGR).

use super::super::Sample;
use super::super::rule::{
    NUM_RE, Rule, SPACE_RE, WORD_BOUNDARY, first_glyph, int_field, num_field, parse_num, rule,
    signed_by_icon, sym, sym_range, syms,
};
use crate::telemetry::catalog::{
    KEY_AZIMUTH_DEG, KEY_GFORCE, KEY_GFORCE_X, KEY_GFORCE_Y, KEY_GFORCE_Z, KEY_GROUND_COURSE_DEG,
    KEY_HEADING_DEG, KEY_HOME_DIR_DEG, KEY_HOME_HEADING_ERROR_DEG, KEY_MOTOR_IDLE, KEY_PITCH_DEG,
    KEY_ROLL_DEG, KEY_THROTTLE_AUTO, KEY_THROTTLE_PCT,
};
use crate::telemetry::inav;

/// The angle between two home-direction arrow glyphs (16 of them).
const ARROW_STEP_DEG: f64 = 22.5;

/// What INAV writes after the throttle icon, in place of the number, when the throttle is not above zero: `STOP`
/// (armed, motor stopped), `IDLE` (armed, motor idling) or `DARM` (disarmed). See `osdFormatThrottlePosition` in
/// the INAV source.
const THROTTLE_OFF_WORDS: [&str; 3] = ["STOP", "IDLE", "DARM"];

/// In the order of the glyphs of the three axes.
const GFORCE_AXIS_KEYS: [&str; 3] = [KEY_GFORCE_X, KEY_GFORCE_Y, KEY_GFORCE_Z];

/// Reads how the aircraft is flying: the throttle, the heading and the other angles, the attitude, the G-force and
/// the direction of home.
pub(super) fn rules() -> Vec<Rule> {
    // auto-throttle uses two glyphs, manual uses THR
    let throttle_icons = format!(
        "({}{}|{})",
        sym(inav::LABEL_AUTO_THROTTLE_0),
        sym(inav::LABEL_AUTO_THROTTLE_1),
        sym(inav::ICON_THROTTLE)
    );
    // a percentage (negative when the stick is a little below the end of its range) or one of the words
    let throttle_value = format!("(-?[0-9]+|{})", THROTTLE_OFF_WORDS.join("|"));
    let roll_icons = format!(
        "({})",
        syms(&[
            inav::ICON_ROLL_LEFT,
            inav::ICON_ROLL_LEVEL,
            inav::ICON_ROLL_RIGHT
        ])
    );
    // INAV shows 'P' in place of the glyph when |pitch| < 0.1 degrees. That 'P' does not end a word, or the "P 3"
    // of the text "TO WP 3/5" would be a pitch. The glyph shows what the pilot sees: down arrow = nose down.
    let pitch_icons = format!(
        "({WORD_BOUNDARY}P|{})",
        syms(&[inav::ICON_PITCH_UP, inav::ICON_PITCH_DOWN])
    );
    let gforce_axes = format!("({})", sym_range(inav::ICON_GFORCE_X, inav::ICON_GFORCE_Z));
    let home_arrows = format!(
        "({})",
        sym_range(inav::HOME_ARROW_FIRST, inav::HOME_ARROW_LAST)
    );
    vec![
        rule(
            &[&throttle_icons, SPACE_RE, &throttle_value],
            Box::new(throttle),
        ),
        // The angles that end with the degrees glyph. The home heading error goes before the heading, whose rule
        // would take it.
        rule(
            &[
                &sym(inav::ICON_HOME),
                &sym(inav::ICON_HEADING),
                SPACE_RE,
                "(-?[0-9]+)",
                &sym(inav::UNIT_DEGREES),
            ],
            int_field(KEY_HOME_HEADING_ERROR_DEG, 1),
        ),
        rule(
            &[
                &sym(inav::ICON_GROUND_COURSE),
                SPACE_RE,
                "([0-9]+)",
                &sym(inav::UNIT_DEGREES),
            ],
            int_field(KEY_GROUND_COURSE_DEG, 1),
        ),
        rule(
            &[
                &sym(inav::ICON_AZIMUTH),
                SPACE_RE,
                "([0-9]+)",
                &sym(inav::UNIT_DEGREES),
            ],
            int_field(KEY_AZIMUTH_DEG, 1),
        ),
        rule(
            &[
                &sym(inav::ICON_HEADING),
                SPACE_RE,
                "([0-9]+)",
                &sym(inav::UNIT_DEGREES),
            ],
            int_field(KEY_HEADING_DEG, 1),
        ),
        rule(
            &[&roll_icons, SPACE_RE, NUM_RE],
            signed_by_icon(KEY_ROLL_DEG, inav::ICON_ROLL_LEFT),
        ),
        rule(
            &[&pitch_icons, SPACE_RE, NUM_RE],
            signed_by_icon(KEY_PITCH_DEG, inav::ICON_PITCH_DOWN),
        ),
        rule(
            &[&sym(inav::ICON_GFORCE), SPACE_RE, NUM_RE],
            num_field(KEY_GFORCE, 1),
        ),
        rule(&[&gforce_axes, SPACE_RE, NUM_RE], Box::new(gforce_axis)),
        // the direction-of-home arrow: 16 glyphs, 22.5 degrees apart
        rule(&[&home_arrows], Box::new(home_arrow)),
    ]
}

/// Reads the throttle in group 2; the glyphs in group 1 say whether navigation is controlling it.
///
/// A word (`STOP`, `IDLE`, `DARM`) is a throttle of zero. It takes the place of the auto-throttle glyphs too, so
/// whether navigation was in control cannot be told, and no auto-throttle reading is stored.
///
/// Whenever the throttle is read, `osd_motor_idle` says whether the word was `IDLE`: a fixed wing is silent then, but
/// the motors of a drone spin at their idle speed, and the throttle alone cannot tell `IDLE` from `STOP` and `DARM`.
fn throttle(groups: &[String], sample: &mut Sample) {
    let word = groups[2].as_str();
    if THROTTLE_OFF_WORDS.contains(&word) {
        sample.num(KEY_THROTTLE_PCT, 0.0);
        sample.num(KEY_MOTOR_IDLE, if word == "IDLE" { 1.0 } else { 0.0 });
        return;
    }
    int_field(KEY_THROTTLE_PCT, 2)(groups, sample);
    sample.num(KEY_MOTOR_IDLE, 0.0);
    let auto = if first_glyph(&groups[1]) == inav::LABEL_AUTO_THROTTLE_0 {
        1.0
    } else {
        0.0
    };
    sample.num(KEY_THROTTLE_AUTO, auto);
}

/// Reads the G-force of the axis whose glyph is in group 1.
fn gforce_axis(groups: &[String], sample: &mut Sample) {
    if let Some(value) = parse_num(&groups[2]) {
        let axis = usize::from(first_glyph(&groups[1]) - inav::ICON_GFORCE_X);
        sample.num(GFORCE_AXIS_KEYS[axis], value);
    }
}

fn home_arrow(groups: &[String], sample: &mut Sample) {
    let step = f64::from(first_glyph(&groups[1]) - inav::HOME_ARROW_FIRST);
    sample.num(KEY_HOME_DIR_DEG, step * ARROW_STEP_DEG);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::decoder::rule::glyph_char;

    /// Runs the rules of this module on a line with one character per cell.
    fn read(line: &str) -> Sample {
        let mut sample = Sample::default();
        let mut line = line.to_owned();
        for rule in rules() {
            line = rule.apply(&line, &mut sample);
        }
        sample
    }

    fn manual(text: &str) -> String {
        format!("{}{text}", glyph_char(inav::ICON_THROTTLE))
    }

    fn auto(text: &str) -> String {
        format!(
            "{}{}{text}",
            glyph_char(inav::LABEL_AUTO_THROTTLE_0),
            glyph_char(inav::LABEL_AUTO_THROTTLE_1)
        )
    }

    #[test]
    fn the_throttle_is_read_as_a_percentage() {
        let sample = read(&manual("  8"));
        assert_eq!(sample.number(KEY_THROTTLE_PCT), Some(8.0));
        assert_eq!(sample.number(KEY_THROTTLE_AUTO), Some(0.0));
        assert_eq!(sample.number(KEY_MOTOR_IDLE), Some(0.0));
        assert_eq!(read(&manual("100")).number(KEY_THROTTLE_PCT), Some(100.0));

        let sample = read(&auto(" 42"));
        assert_eq!(sample.number(KEY_THROTTLE_PCT), Some(42.0));
        assert_eq!(sample.number(KEY_THROTTLE_AUTO), Some(1.0));
    }

    #[test]
    fn the_words_inav_shows_for_a_throttle_of_zero_read_as_zero() {
        // INAV writes the icon and then STOP, IDLE or DARM, with no blank between
        for word in ["STOP", "IDLE", "DARM"] {
            let sample = read(&manual(word));
            assert_eq!(sample.number(KEY_THROTTLE_PCT), Some(0.0), "{word}");
            // only IDLE has the motor spinning
            let idle = if word == "IDLE" { 1.0 } else { 0.0 };
            assert_eq!(sample.number(KEY_MOTOR_IDLE), Some(idle), "{word}");
            // the word replaces the auto-throttle glyphs, so who was in control is unknown
            assert!(!sample.contains(KEY_THROTTLE_AUTO), "{word}");
        }
    }

    #[test]
    fn a_throttle_a_little_below_zero_is_read_as_it_is_shown() {
        // " -1": the stick is just under the end of its range
        let sample = read(&manual(" -1"));
        assert_eq!(sample.number(KEY_THROTTLE_PCT), Some(-1.0));
        assert_eq!(sample.number(KEY_THROTTLE_AUTO), Some(0.0));
    }

    #[test]
    fn the_icon_with_some_other_text_is_not_a_throttle() {
        assert!(!read(&manual("GOOD")).contains(KEY_THROTTLE_PCT));
        assert!(!read(&manual("   ")).contains(KEY_THROTTLE_PCT));
        assert!(!read(&manual("GOOD")).contains(KEY_MOTOR_IDLE));
    }
}
