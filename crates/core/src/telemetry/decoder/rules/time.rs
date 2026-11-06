// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/rules_time.go (Apache-2.0, Copyright 2026 EricoGR).

use super::super::rule::{Rule, SPACE_RE, first_glyph, rule, sym, syms, text_field};
use super::super::units::SECONDS_PER_MINUTE;
use crate::telemetry::catalog::{
    KEY_CLOCK, KEY_FLIGHT_TIME_S, KEY_GLIDE_TIME_S, KEY_ON_TIME_S, KEY_RTH_TIME_LEFT_S,
};
use crate::telemetry::inav;

/// Reads the elements that show a time: the flight time, the time since power-on, the time left before the return
/// to home and for gliding, and the real-time clock.
pub(super) fn rules() -> Vec<Rule> {
    vec![
        time_rule(
            KEY_FLIGHT_TIME_S,
            inav::ICON_FLIGHT_MINUTES,
            Some(inav::ICON_FLIGHT_HOURS),
        ),
        time_rule(
            KEY_ON_TIME_S,
            inav::ICON_ON_MINUTES,
            Some(inav::ICON_ON_HOURS),
        ),
        time_rule(
            KEY_RTH_TIME_LEFT_S,
            inav::ICON_REMAINING_MINUTES,
            Some(inav::ICON_REMAINING_HOURS),
        ),
        time_rule(KEY_GLIDE_TIME_S, inav::ICON_GLIDE_MINUTES, None),
        rule(
            &[
                &sym(inav::ICON_CLOCK),
                SPACE_RE,
                "([0-9]{2}:[0-9]{2}:[0-9]{2})",
            ],
            text_field(KEY_CLOCK, 1),
        ),
    ]
}

/// Reads icon + MM:SS into `key`, in seconds. `hours` is the icon the OSD uses when the time passes 99:59 and shows
/// HH:MM instead; `None` if the element never switches.
fn time_rule(key: &'static str, minutes: u16, hours: Option<u16>) -> Rule {
    let mut icons = vec![minutes];
    icons.extend(hours);
    let icon_group = format!("({})", syms(&icons));
    rule(
        &[&icon_group, SPACE_RE, "([0-9]+):([0-9]+)"],
        Box::new(move |groups, sample| {
            let (Ok(a), Ok(b)) = (groups[2].parse::<i64>(), groups[3].parse::<i64>()) else {
                return;
            };
            let mut seconds = a * SECONDS_PER_MINUTE + b;
            if hours.is_some_and(|hours| first_glyph(&groups[1]) == hours) {
                seconds = a * 3600 + b * SECONDS_PER_MINUTE;
            }
            sample.num(key, seconds as f64);
        }),
    )
}
