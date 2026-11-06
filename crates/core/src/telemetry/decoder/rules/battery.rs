// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/rules_battery.go (Apache-2.0, Copyright 2026 EricoGR).

use super::super::Sample;
use super::super::rule::{
    NUM_RE, Rule, SPACE_RE, first_glyph, int_field, num_field, parse_num, rule, scaled_reading,
    sym, sym_range, syms,
};
use super::super::units::{ENERGY_PER_DIST_TO_KM, KM_PER_MILE, KM_PER_NAUTICAL_MILE, POWER_TO_W};
use crate::telemetry::catalog::{
    KEY_BATT_CELL_V, KEY_BATT_PCT, KEY_BATT_V, KEY_CURRENT_A, KEY_IMPEDANCE_MOHM, KEY_MAH_PER_KM,
    KEY_MAH_USED, KEY_POWER_W, KEY_WH_PER_KM, KEY_WH_USED,
};
use crate::telemetry::inav;

/// No cell goes above about 4.4 V, so a larger battery reading is the whole pack.
const MAX_CELL_VOLTS: f64 = 4.5;
/// Converts the Ah the OSD shows for large values back to mAh.
const AH_TO_MAH: f64 = 1000.0;

/// Reads the voltage, charge, current, capacity, energy, power and efficiency of the main battery.
pub(super) fn rules() -> Vec<Rule> {
    let battery_icons = sym_range(inav::ICON_BATTERY_FULL, inav::ICON_BATTERY_EMPTY);
    let capacity_units = format!("({})", syms(&[inav::UNIT_MAH, inav::UNIT_AH]));
    let efficiency_units = format!("({})", efficiency_units_re());
    vec![
        rule(
            &[&battery_icons, SPACE_RE, NUM_RE, &sym(inav::UNIT_VOLT)],
            Box::new(battery_voltage),
        ),
        // the remaining charge: the battery icon and "nnn%"
        rule(
            &[&battery_icons, SPACE_RE, "([0-9]+)%"],
            int_field(KEY_BATT_PCT, 1),
        ),
        rule(&[NUM_RE, &sym(inav::UNIT_AMP)], num_field(KEY_CURRENT_A, 1)),
        rule(&[NUM_RE, &capacity_units], Box::new(capacity_used)),
        rule(&[NUM_RE, &efficiency_units], Box::new(efficiency_per_km)),
        rule(&[NUM_RE, &sym(inav::UNIT_WH)], num_field(KEY_WH_USED, 1)),
        scaled_reading(KEY_WH_PER_KM, ENERGY_PER_DIST_TO_KM, &[]),
        scaled_reading(KEY_POWER_W, POWER_TO_W, &[]),
        rule(
            &[NUM_RE, &sym(inav::UNIT_MILLIOHM)],
            num_field(KEY_IMPEDANCE_MOHM, 1),
        ),
    ]
}

/// Reads the voltage in group 1: a reading above what a cell can give is the whole pack.
fn battery_voltage(groups: &[String], sample: &mut Sample) {
    let Some(value) = parse_num(&groups[1]) else {
        return;
    };
    if value > MAX_CELL_VOLTS {
        sample.num(KEY_BATT_V, value);
    } else {
        sample.num(KEY_BATT_CELL_V, value);
    }
}

/// Reads the capacity in group 1; the OSD switches from mAh to Ah (the glyph in group 2) on large values.
fn capacity_used(groups: &[String], sample: &mut Sample) {
    let Some(mut value) = parse_num(&groups[1]) else {
        return;
    };
    if first_glyph(&groups[2]) == inav::UNIT_AH {
        value *= AH_TO_MAH;
    }
    sample.num(KEY_MAH_USED, value);
}

/// The glyphs that follow an efficiency in capacity per distance: two glyphs for mAh, and one for Ah per km. The
/// Ah per mile and per nautical mile glyphs sit on the `$` and `?` codes, which text uses too (a craft name such
/// as `X8 2$`), so those two are not read.
fn efficiency_units_re() -> String {
    [
        format!(
            "{}{}",
            sym(inav::UNIT_MAH_PER_KM_0),
            sym(inav::UNIT_MAH_PER_KM_1)
        ),
        format!(
            "{}{}",
            sym(inav::UNIT_MAH_PER_MI_0),
            sym(inav::UNIT_MAH_PER_MI_1)
        ),
        format!(
            "{}{}",
            sym(inav::UNIT_MAH_PER_NM_0),
            sym(inav::UNIT_MAH_PER_NM_1)
        ),
        sym(inav::UNIT_AH_PER_KM),
    ]
    .join("|")
}

/// Emits the efficiency in group 1 as mAh per km, whatever the units the OSD shows it in (group 2).
fn efficiency_per_km(groups: &[String], sample: &mut Sample) {
    let Some(mut value) = parse_num(&groups[1]) else {
        return;
    };
    match first_glyph(&groups[2]) {
        inav::UNIT_MAH_PER_MI_0 => value /= KM_PER_MILE,
        inav::UNIT_MAH_PER_NM_0 => value /= KM_PER_NAUTICAL_MILE,
        inav::UNIT_AH_PER_KM => value *= AH_TO_MAH,
        _ => {}
    }
    sample.num(KEY_MAH_PER_KM, value);
}
