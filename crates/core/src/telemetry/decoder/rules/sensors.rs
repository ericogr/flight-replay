// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/rules_sensors.go (Apache-2.0, Copyright 2026 EricoGR).

use super::super::Sample;
use super::super::rule::{
    Emit, NUM_RE, Rule, SPACE_RE, first_glyph, parse_num, rule, sym, sym_range, syms,
};
use crate::telemetry::catalog::{
    KEY_BARO_TEMP_C, KEY_ESC_RPM, KEY_ESC_TEMP_C, KEY_IMU_TEMP_C, KEY_TEMP_SENSOR_C,
};
use crate::telemetry::inav;

/// Reads the temperatures and the motor speed.
pub(super) fn rules() -> Vec<Rule> {
    let unit = format!("({})", syms(&[inav::UNIT_CELSIUS, inav::UNIT_FAHRENHEIT]));
    let icon_temperature = |key: &'static str, icon: u16| {
        rule(
            &[&sym(icon), SPACE_RE, NUM_RE, &unit],
            temperature(key, 1, 2),
        )
    };
    let sensor_icons = format!(
        "({})",
        sym_range(inav::ICON_TEMP_SENSOR_FIRST, inav::ICON_TEMP_SENSOR_LAST)
    );
    vec![
        icon_temperature(KEY_IMU_TEMP_C, inav::ICON_IMU_TEMPERATURE),
        icon_temperature(KEY_BARO_TEMP_C, inav::ICON_BARO_TEMPERATURE),
        icon_temperature(KEY_ESC_TEMP_C, inav::ICON_ESC_TEMPERATURE),
        // The sensors that use one of the other icons. Those with a text label instead of an icon have neither,
        // and are read by the next rule.
        rule(
            &[&sensor_icons, SPACE_RE, NUM_RE, &unit],
            temperature(KEY_TEMP_SENSOR_C, 2, 3),
        ),
        rule(&[NUM_RE, &unit], temperature(KEY_TEMP_SENSOR_C, 1, 2)),
        // "12.3K" is 12300 rpm when the number does not fit
        rule(
            &[&sym(inav::ICON_RPM), SPACE_RE, NUM_RE, "(K)?"],
            Box::new(motor_speed),
        ),
    ]
}

/// Emits the number in group `number`, converted to Celsius if the unit glyph in group `unit` says Fahrenheit.
fn temperature(key: &'static str, number: usize, unit: usize) -> Emit {
    Box::new(move |groups, sample| {
        let Some(mut value) = parse_num(&groups[number]) else {
            return;
        };
        if first_glyph(&groups[unit]) == inav::UNIT_FAHRENHEIT {
            value = (value - 32.0) * 5.0 / 9.0;
        }
        sample.num(key, value);
    })
}

/// Reads the rpm in group 1; the K in group 2 means thousands.
fn motor_speed(groups: &[String], sample: &mut Sample) {
    let Some(mut value) = parse_num(&groups[1]) else {
        return;
    };
    if !groups[2].is_empty() {
        value *= 1000.0;
    }
    sample.num(KEY_ESC_RPM, value);
}
