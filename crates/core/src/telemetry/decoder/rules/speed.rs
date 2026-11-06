// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/rules_speed.go (Apache-2.0, Copyright 2026 EricoGR).

use super::super::Sample;
use super::super::rule::{
    NUM_RE, Rule, SPACE_RE, factor, first_glyph, no_reading, num_field, parse_num, rule, scaled,
    scaled_reading, sym, sym_range, syms, units_re,
};
use super::super::units::{
    METERS_PER_FOOT, SPEED_3D_TO_KMH, SPEED_TO_KMH, VSPEED_TO_MS, WIND_TO_KMH,
};
use crate::telemetry::catalog::{
    KEY_3D_MAX_SPEED_KMH, KEY_3D_SPEED_KMH, KEY_AIR_MAX_SPEED_KMH, KEY_AIR_SPEED_KMH,
    KEY_CLIMB_EFF_MAH_PER_M, KEY_GLIDESLOPE, KEY_GPS_MAX_SPEED_KMH, KEY_GPS_SPEED_KMH,
    KEY_MIN_GROUND_SPEED_KMH, KEY_VSPEED_MS, KEY_WIND_DIR_DEG, KEY_WIND_SPEED_KMH,
    KEY_WIND_VSPEED_KMH,
};
use crate::telemetry::inav;

/// The angle between two wind-direction arrow glyphs (8 of them).
const WIND_STEP_DEG: f64 = 45.0;

/// From the most specific rule to the generic one: the speed without an icon is the GPS ground speed, so it must
/// run after every other speed has erased its own (they share the unit glyphs).
pub(super) fn rules() -> Vec<Rule> {
    let wind_arrow = sym_range(inav::DIRECTION_ARROW_FIRST, inav::DIRECTION_ARROW_FIRST + 7);
    let wind_vertical_arrow = syms(&[inav::ARROW_UP_SMALL, inav::ARROW_DOWN_SMALL]);
    let wind_arrow_group = format!("({wind_arrow})");
    let wind_vertical_group = format!("({wind_vertical_arrow})");
    let wind_units = units_re(WIND_TO_KMH);
    vec![
        rule(
            &[
                &sym(inav::ICON_WIND_HORIZONTAL),
                &wind_arrow_group,
                SPACE_RE,
                NUM_RE,
                &wind_units,
            ],
            Box::new(wind_horizontal),
        ),
        rule(
            &[
                &sym(inav::ICON_WIND_VERTICAL),
                &wind_vertical_group,
                SPACE_RE,
                NUM_RE,
                &wind_units,
            ],
            Box::new(wind_vertical),
        ),
        // A wind estimate that is not valid yet shows a * in place of the unit every other second. It is not a
        // reading, but it must not reach the generic pass either.
        rule(
            &[
                &sym(inav::ICON_WIND_HORIZONTAL),
                &wind_arrow,
                SPACE_RE,
                NUM_RE,
                r"\*",
            ],
            no_reading(),
        ),
        rule(
            &[
                &sym(inav::ICON_WIND_VERTICAL),
                &wind_vertical_arrow,
                SPACE_RE,
                NUM_RE,
                r"\*",
            ],
            no_reading(),
        ),
        // The maximum speeds start with the MAX glyph and must go before the plain ones, which would read them as
        // the current speed.
        scaled_reading(
            KEY_AIR_MAX_SPEED_KMH,
            SPEED_TO_KMH,
            &[&sym(inav::LABEL_MAX), &sym(inav::ICON_AIR_SPEED)],
        ),
        scaled_reading(
            KEY_AIR_SPEED_KMH,
            SPEED_TO_KMH,
            &[&sym(inav::ICON_AIR_SPEED)],
        ),
        scaled_reading(
            KEY_3D_MAX_SPEED_KMH,
            SPEED_3D_TO_KMH,
            &[&sym(inav::LABEL_MAX)],
        ),
        scaled_reading(KEY_3D_SPEED_KMH, SPEED_3D_TO_KMH, &[]),
        scaled_reading(
            KEY_MIN_GROUND_SPEED_KMH,
            SPEED_TO_KMH,
            &[&sym(inav::ICON_MIN_GROUND_SPEED)],
        ),
        scaled_reading(
            KEY_GPS_MAX_SPEED_KMH,
            SPEED_TO_KMH,
            &[&sym(inav::LABEL_MAX)],
        ),
        scaled_reading(KEY_VSPEED_MS, VSPEED_TO_MS, &[]),
        scaled_reading(KEY_GPS_SPEED_KMH, SPEED_TO_KMH, &[]),
    ]
}

/// Reads how efficiently the aircraft is flying.
pub(super) fn performance_rules() -> Vec<Rule> {
    let climb_units = format!(
        "({}{}|{}{})",
        sym(inav::UNIT_CLIMB_M_0),
        sym(inav::UNIT_CLIMB_M_1),
        sym(inav::UNIT_CLIMB_FT_0),
        sym(inav::UNIT_CLIMB_FT_1)
    );
    vec![
        rule(
            &[&sym(inav::ICON_GLIDESLOPE), SPACE_RE, NUM_RE],
            num_field(KEY_GLIDESLOPE, 1),
        ),
        rule(&[NUM_RE, &climb_units], Box::new(climb_efficiency)),
    ]
}

/// Reads the direction arrow in group 1 (8 glyphs, 45 degrees apart) and the speed in groups 2 (number) and 3
/// (unit).
fn wind_horizontal(groups: &[String], sample: &mut Sample) {
    let step = f64::from(first_glyph(&groups[1]) - inav::DIRECTION_ARROW_FIRST);
    sample.num(KEY_WIND_DIR_DEG, step * WIND_STEP_DEG);
    scaled(KEY_WIND_SPEED_KMH, 2, 3, WIND_TO_KMH)(groups, sample);
}

/// Reads the arrow in group 1 (up is positive) and the speed in groups 2 (number) and 3 (unit).
fn wind_vertical(groups: &[String], sample: &mut Sample) {
    let Some(mut value) = parse_num(&groups[2]) else {
        return;
    };
    value *= factor(WIND_TO_KMH, first_glyph(&groups[3]));
    if first_glyph(&groups[1]) == inav::ARROW_DOWN_SMALL {
        value = -value;
    }
    sample.num(KEY_WIND_VSPEED_KMH, value);
}

/// Reads the number in group 1 and converts the per-foot variant (group 2) to the per-metre one.
fn climb_efficiency(groups: &[String], sample: &mut Sample) {
    let Some(mut value) = parse_num(&groups[1]) else {
        return;
    };
    if first_glyph(&groups[2]) == inav::UNIT_CLIMB_FT_0 {
        value /= METERS_PER_FOOT;
    }
    sample.num(KEY_CLIMB_EFF_MAH_PER_M, value);
}
