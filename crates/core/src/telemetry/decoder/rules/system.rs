// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/rules_system.go (Apache-2.0, Copyright 2026 EricoGR).

use super::super::Sample;
use super::super::rule::{NUM_RE, Rule, SPACE_RE, WORD_BOUNDARY, int_field, parse_num, rule, sym};
use crate::telemetry::catalog::{
    KEY_BLACKBOX_LOG, KEY_GVAR_0, KEY_GVAR_1, KEY_GVAR_2, KEY_GVAR_3, KEY_PROFILE,
};
use crate::telemetry::inav;

/// In the order of the variables.
const GVAR_KEYS: [&str; 4] = [KEY_GVAR_0, KEY_GVAR_1, KEY_GVAR_2, KEY_GVAR_3];

/// Reads the state of the flight controller: the configuration profile, the blackbox log and the global variables
/// of the programming framework.
pub(super) fn rules() -> Vec<Rule> {
    let gvar = format!(r"{WORD_BOUNDARY}G([0-3]):");
    vec![
        rule(
            &[&sym(inav::ICON_PROFILE), SPACE_RE, "([0-9]+)"],
            int_field(KEY_PROFILE, 1),
        ),
        rule(
            &[&sym(inav::ICON_BLACKBOX), "([0-9]{5})"],
            int_field(KEY_BLACKBOX_LOG, 1),
        ),
        // "G0:" to "G3:" and a number
        rule(&[&gvar, SPACE_RE, NUM_RE], Box::new(global_variable)),
    ]
}

/// Reads the number in group 2 into the variable named in group 1, which the expression limits to a digit from 0
/// to 3.
fn global_variable(groups: &[String], sample: &mut Sample) {
    if let Some(value) = parse_num(&groups[2]) {
        let index = usize::from(groups[1].as_bytes()[0] - b'0');
        sample.num(GVAR_KEYS[index], value);
    }
}
