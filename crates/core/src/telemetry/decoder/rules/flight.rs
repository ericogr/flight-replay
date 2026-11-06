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
    KEY_HEADING_DEG, KEY_HOME_DIR_DEG, KEY_HOME_HEADING_ERROR_DEG, KEY_PITCH_DEG, KEY_ROLL_DEG,
    KEY_THROTTLE_AUTO, KEY_THROTTLE_PCT,
};
use crate::telemetry::inav;

/// The angle between two home-direction arrow glyphs (16 of them).
const ARROW_STEP_DEG: f64 = 22.5;

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
        rule(&[&throttle_icons, SPACE_RE, "([0-9]+)"], Box::new(throttle)),
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
fn throttle(groups: &[String], sample: &mut Sample) {
    int_field(KEY_THROTTLE_PCT, 2)(groups, sample);
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
