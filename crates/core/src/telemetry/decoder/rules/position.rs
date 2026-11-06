// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/rules_position.go (Apache-2.0, Copyright 2026 EricoGR).

use super::super::Sample;
use super::super::rule::{
    NUM_RE, Rule, SPACE_RE, WORD_BOUNDARY, factor, first_glyph, int_field, num_field, parse_num,
    rule, scaled_reading, sym, sym_range, units_re,
};
use super::super::units::{ALT_TO_METERS, DIST_TO_METERS, ODOMETER_TO_KM};
use crate::telemetry::catalog::{
    KEY_ALTITUDE_M, KEY_CROSS_TRACK_ERROR_M, KEY_FENCE_DIST_H_M, KEY_FENCE_DIST_V_M,
    KEY_GLIDE_RANGE_M, KEY_GPS_HDOP, KEY_GPS_LAT, KEY_GPS_LON, KEY_GPS_SATS, KEY_HOME_DIST_M,
    KEY_ODOMETER_KM, KEY_RANGEFINDER_M, KEY_RTH_DIST_LEFT_M, KEY_TRIP_DIST_M,
};
use crate::telemetry::inav;

/// Reads where the aircraft is: the GPS fix, the altitude and the distances. Most distances are an icon, a number
/// and a unit glyph that changes with the magnitude (m and km, ft and mi).
pub(super) fn rules() -> Vec<Rule> {
    let fence_start = format!("{WORD_BOUNDARY}FD");
    let fence_vertical_tail = format!(
        "(?: *({}))?",
        sym_range(inav::DIRECTION_ARROW_FIRST, inav::DIRECTION_ARROW_FIRST + 7)
    );
    let altitude_units = units_re(ALT_TO_METERS);
    vec![
        rule(
            &[
                &sym(inav::ICON_SATELLITE_LEFT),
                &sym(inav::ICON_SATELLITE_RIGHT),
                SPACE_RE,
                "([0-9]+)",
            ],
            int_field(KEY_GPS_SATS, 1),
        ),
        rule(
            &[&sym(inav::LABEL_LATITUDE), NUM_RE],
            num_field(KEY_GPS_LAT, 1),
        ),
        rule(
            &[&sym(inav::LABEL_LONGITUDE), NUM_RE],
            num_field(KEY_GPS_LON, 1),
        ),
        rule(
            &[
                &sym(inav::LABEL_HDOP_LEFT),
                &sym(inav::LABEL_HDOP_RIGHT),
                SPACE_RE,
                NUM_RE,
            ],
            num_field(KEY_GPS_HDOP, 1),
        ),
        scaled_reading(
            KEY_RTH_DIST_LEFT_M,
            DIST_TO_METERS,
            &[&sym(inav::ICON_REMAINING_DISTANCE)],
        ),
        scaled_reading(
            KEY_GLIDE_RANGE_M,
            DIST_TO_METERS,
            &[&sym(inav::ICON_GLIDE_DISTANCE)],
        ),
        scaled_reading(
            KEY_CROSS_TRACK_ERROR_M,
            DIST_TO_METERS,
            &[&sym(inav::ICON_CROSS_TRACK_ERROR)],
        ),
        scaled_reading(
            KEY_ODOMETER_KM,
            ODOMETER_TO_KM,
            &[&sym(inav::ICON_ODOMETER)],
        ),
        // The geofence: the horizontal distance is "FD" and a distance, the vertical one is "FD" and an altitude
        // followed by an arrow, down when the aircraft is below the zone. The vertical one goes before the
        // altitude, which would take it.
        scaled_reading(KEY_FENCE_DIST_H_M, DIST_TO_METERS, &[&fence_start]),
        rule(
            &[
                &fence_start,
                SPACE_RE,
                NUM_RE,
                &altitude_units,
                &fence_vertical_tail,
            ],
            Box::new(fence_dist_v),
        ),
        scaled_reading(KEY_HOME_DIST_M, DIST_TO_METERS, &[&sym(inav::ICON_HOME)]),
        scaled_reading(
            KEY_TRIP_DIST_M,
            DIST_TO_METERS,
            &[&sym(inav::ICON_TRIP_DISTANCE)],
        ),
        scaled_reading(KEY_ALTITUDE_M, ALT_TO_METERS, &[]),
        // a distance with no icon: the rangefinder; every distance with an icon has been taken by now
        scaled_reading(KEY_RANGEFINDER_M, DIST_TO_METERS, &[]),
    ]
}

/// Reads the altitude in groups 1 (number) and 2 (unit) and the optional arrow in group 3, which says the aircraft
/// is below the zone when it points down.
fn fence_dist_v(groups: &[String], sample: &mut Sample) {
    let Some(mut value) = parse_num(&groups[1]) else {
        return;
    };
    value *= factor(ALT_TO_METERS, first_glyph(&groups[2]));
    if !groups[3].is_empty() && first_glyph(&groups[3]) == inav::DIRECTION_ARROW_FIRST + 4 {
        value = -value;
    }
    sample.num(KEY_FENCE_DIST_V_M, value);
}
