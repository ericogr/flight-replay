// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/units.go (Apache-2.0, Copyright 2026 EricoGR).

//! The unit glyph of an element says what its number is measured in, and it changes with the magnitude (m and
//! km, ft and mi) and with INAV's unit setting. These tables give the factor that takes the number to the unit
//! of the column, which is always the metric one.

use crate::telemetry::inav;

// Exact conversion factors, written as the decimals they are.
pub(super) const METERS_PER_FOOT: f64 = 0.3048;
const METERS_PER_MILE: f64 = 1_609.344;
const METERS_PER_NAUTICAL_MILE: f64 = 1852.0;
pub(super) const KM_PER_MILE: f64 = 1.609_344;
pub(super) const KM_PER_NAUTICAL_MILE: f64 = 1.852;
const KMH_PER_METERS_PER_SECOND: f64 = 3.6;
pub(super) const SECONDS_PER_MINUTE: i64 = 60;

/// A table of unit glyphs and the factor of each.
pub(super) type Units = &'static [(u16, f64)];

pub(super) const DIST_TO_METERS: Units = &[
    (inav::UNIT_DISTANCE_M, 1.0),
    (inav::UNIT_DISTANCE_KM, 1000.0),
    (inav::UNIT_DISTANCE_FT, METERS_PER_FOOT),
    (inav::UNIT_DISTANCE_MI, METERS_PER_MILE),
    (inav::UNIT_DISTANCE_NM, METERS_PER_NAUTICAL_MILE),
];

pub(super) const ALT_TO_METERS: Units = &[
    (inav::UNIT_ALTITUDE_M, 1.0),
    (inav::UNIT_ALTITUDE_KM, 1000.0),
    (inav::UNIT_ALTITUDE_FT, METERS_PER_FOOT),
    (inav::UNIT_ALTITUDE_KFT, 304.8),
];

/// The odometer has its own unit glyphs, different from the distance ones.
pub(super) const ODOMETER_TO_KM: Units = &[
    (inav::UNIT_KM, 1.0),
    (inav::UNIT_MI, KM_PER_MILE),
    (inav::UNIT_NM, KM_PER_NAUTICAL_MILE),
];

/// Horizontal speed units only: m/s and ft/s belong to the vertical speed indicator.
pub(super) const SPEED_TO_KMH: Units = &[
    (inav::UNIT_KMH, 1.0),
    (inav::UNIT_MPH, KM_PER_MILE),
    (inav::UNIT_KT, KM_PER_NAUTICAL_MILE),
];

pub(super) const SPEED_3D_TO_KMH: Units = &[
    (inav::UNIT_KMH_3D, 1.0),
    (inav::UNIT_MPH_3D, KM_PER_MILE),
    (inav::UNIT_KT_3D, KM_PER_NAUTICAL_MILE),
];

/// Wind speeds can also be shown in m/s.
pub(super) const WIND_TO_KMH: Units = &[
    (inav::UNIT_KMH, 1.0),
    (inav::UNIT_MPH, KM_PER_MILE),
    (inav::UNIT_KT, KM_PER_NAUTICAL_MILE),
    (inav::UNIT_METRES_PER_SECOND, KMH_PER_METERS_PER_SECOND),
];

/// The vertical speed indicator; 100 ft/min is the aviation unit.
pub(super) const VSPEED_TO_MS: Units = &[
    (inav::UNIT_METRES_PER_SECOND, 1.0),
    (inav::UNIT_FEET_PER_SECOND, METERS_PER_FOOT),
    (inav::UNIT_HUNDRED_FEET_PER_MINUTE, 0.508),
];

pub(super) const POWER_TO_W: Units = &[(inav::UNIT_WATT, 1.0), (inav::UNIT_KILOWATT, 1000.0)];

/// A value per mile or per nautical mile, as a value per km.
pub(super) const ENERGY_PER_DIST_TO_KM: Units = &[
    (inav::UNIT_WH_PER_KM, 1.0),
    (inav::UNIT_WH_PER_MI, 1.0 / KM_PER_MILE),
    (inav::UNIT_WH_PER_NM, 1.0 / KM_PER_NAUTICAL_MILE),
];
