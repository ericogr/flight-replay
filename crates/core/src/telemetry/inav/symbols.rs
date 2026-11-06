// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/inav/symbols.go (Apache-2.0, Copyright 2026 EricoGR). Its glyph table was
// taken from fpv-osd-decoder (MIT, Copyright (c) 2026 EricoGR), which generated it from the `osd_symbols.h` header
// of INAV 9.1.0. The names are this project's: they say how the rules read the glyph (as a unit, a label or an
// icon), not what the firmware calls it. A code that no rule reads has no name here and is reported by its number.
//
// Converted to Rust once, by a script. This file is now the source of truth.

//! Glyph codes of the INAV OSD font that the rules read. A code is the position of a character in the
//! 512-character font.

#![allow(missing_docs)]

pub const ICON_RSSI: u16 = 0x01;
pub const ICON_LINK_QUALITY: u16 = 0x02;
pub const LABEL_LATITUDE: u16 = 0x03;
pub const LABEL_LONGITUDE: u16 = 0x04;
pub const ICON_AZIMUTH: u16 = 0x05;
pub const ICON_TELEMETRY_0: u16 = 0x06;
pub const ICON_TELEMETRY_1: u16 = 0x07;
pub const ICON_SATELLITE_LEFT: u16 = 0x08;
pub const ICON_SATELLITE_RIGHT: u16 = 0x09;
pub const UNIT_DEGREES: u16 = 0x0B;
pub const ICON_HEADING: u16 = 0x0C;
pub const ICON_SCALE: u16 = 0x0D;
pub const LABEL_HDOP_LEFT: u16 = 0x0E;
pub const LABEL_HDOP_RIGHT: u16 = 0x0F;
pub const ICON_HOME: u16 = 0x10;
pub const ICON_RSSI_ALT: u16 = 0x11;
pub const UNIT_DB: u16 = 0x12;
pub const UNIT_DBM: u16 = 0x13;
pub const LABEL_SNR: u16 = 0x14;
pub const ARROW_UP_SMALL: u16 = 0x15;
pub const ARROW_DOWN_SMALL: u16 = 0x16;
pub const UNIT_VOLT: u16 = 0x1F;
pub const UNIT_AH_PER_KM: u16 = 0x22;
pub const LABEL_VTX_POWER: u16 = 0x27;
pub const UNIT_MAH_PER_NM_0: u16 = 0x60;
pub const UNIT_MAH_PER_NM_1: u16 = 0x61;
pub const UNIT_MILLIOHM: u16 = 0x62;
pub const ICON_BATTERY_FULL: u16 = 0x63;
pub const ICON_BATTERY_LEVEL_5: u16 = 0x64;
pub const ICON_BATTERY_LEVEL_4: u16 = 0x65;
pub const ICON_BATTERY_LEVEL_3: u16 = 0x66;
pub const ICON_BATTERY_EMPTY: u16 = 0x69;
pub const UNIT_AMP: u16 = 0x6A;
pub const UNIT_MAH_PER_KM_0: u16 = 0x6B;
pub const UNIT_MAH_PER_KM_1: u16 = 0x6C;
pub const UNIT_WH: u16 = 0x6D;
pub const UNIT_WH_PER_KM: u16 = 0x6E;
pub const UNIT_WH_PER_MI: u16 = 0x6F;
pub const UNIT_WH_PER_NM: u16 = 0x70;
pub const UNIT_WATT: u16 = 0x71;
pub const UNIT_MILLIWATT: u16 = 0x72;
pub const UNIT_KILOWATT: u16 = 0x73;
pub const ICON_TRIP_DISTANCE: u16 = 0x75;
pub const UNIT_ALTITUDE_M: u16 = 0x76;
pub const UNIT_ALTITUDE_KM: u16 = 0x77;
pub const UNIT_ALTITUDE_FT: u16 = 0x78;
pub const UNIT_ALTITUDE_KFT: u16 = 0x79;
pub const UNIT_DISTANCE_M: u16 = 0x7A;
pub const UNIT_DISTANCE_KM: u16 = 0x7E;
pub const UNIT_DISTANCE_FT: u16 = 0x7F;
pub const UNIT_DISTANCE_MI: u16 = 0x80;
pub const UNIT_DISTANCE_NM: u16 = 0x81;
pub const UNIT_KM: u16 = 0x83;
pub const UNIT_MI: u16 = 0x84;
pub const UNIT_NM: u16 = 0x85;
pub const ICON_WIND_HORIZONTAL: u16 = 0x86;
pub const ICON_WIND_VERTICAL: u16 = 0x87;
pub const UNIT_KMH_3D: u16 = 0x88;
pub const UNIT_MPH_3D: u16 = 0x89;
pub const UNIT_KT_3D: u16 = 0x8A;
pub const ICON_RPM: u16 = 0x8B;
pub const ICON_AIR_SPEED: u16 = 0x8C;
pub const UNIT_FEET_PER_SECOND: u16 = 0x8D;
pub const UNIT_HUNDRED_FEET_PER_MINUTE: u16 = 0x8E;
pub const UNIT_METRES_PER_SECOND: u16 = 0x8F;
pub const UNIT_KMH: u16 = 0x90;
pub const UNIT_MPH: u16 = 0x91;
pub const UNIT_KT: u16 = 0x92;
pub const UNIT_MAH_PER_MI_0: u16 = 0x93;
pub const UNIT_MAH_PER_MI_1: u16 = 0x94;
pub const ICON_THROTTLE: u16 = 0x95;
pub const UNIT_FAHRENHEIT: u16 = 0x96;
pub const UNIT_CELSIUS: u16 = 0x97;
pub const UNIT_MAH: u16 = 0x99;
pub const ICON_ON_HOURS: u16 = 0x9A;
pub const ICON_FLIGHT_HOURS: u16 = 0x9B;
pub const ICON_GLIDESLOPE: u16 = 0x9C;
pub const ICON_ON_MINUTES: u16 = 0x9E;
pub const ICON_FLIGHT_MINUTES: u16 = 0x9F;
pub const ICON_CLOCK: u16 = 0xA0;
pub const LABEL_AUTO_THROTTLE_0: u16 = 0xAB;
pub const LABEL_AUTO_THROTTLE_1: u16 = 0xAC;
pub const ICON_ROLL_LEFT: u16 = 0xAD;
pub const ICON_ROLL_LEVEL: u16 = 0xAE;
pub const ICON_ROLL_RIGHT: u16 = 0xAF;
pub const ICON_PITCH_UP: u16 = 0xB0;
pub const ICON_PITCH_DOWN: u16 = 0xBB;
pub const ICON_GFORCE: u16 = 0xBC;
pub const ICON_GFORCE_X: u16 = 0xBD;
pub const ICON_GFORCE_Y: u16 = 0xBE;
pub const ICON_GFORCE_Z: u16 = 0xBF;
pub const ICON_BARO_TEMPERATURE: u16 = 0xC0;
pub const ICON_IMU_TEMPERATURE: u16 = 0xC1;
pub const ICON_TEMP_SENSOR_FIRST: u16 = 0xC2;
pub const ICON_ESC_TEMPERATURE: u16 = 0xC3;
pub const ICON_TEMP_SENSOR_LAST: u16 = 0xC7;
pub const LABEL_MAX: u16 = 0xCE;
pub const ICON_PROFILE: u16 = 0xCF;
pub const UNIT_AH: u16 = 0xD3;
pub const ICON_GLIDE_DISTANCE: u16 = 0xD4;
pub const ICON_GLIDE_MINUTES: u16 = 0xD5;
pub const UNIT_CLIMB_FT_0: u16 = 0xD6;
pub const UNIT_CLIMB_FT_1: u16 = 0xD7;
pub const UNIT_CLIMB_M_0: u16 = 0xD8;
pub const UNIT_CLIMB_M_1: u16 = 0xD9;
pub const ICON_REMAINING_MINUTES: u16 = 0xDA;
pub const ICON_REMAINING_HOURS: u16 = 0xDB;
pub const ICON_GROUND_COURSE: u16 = 0xDC;
pub const ICON_ALERT: u16 = 0xDD;
pub const ICON_MIN_GROUND_SPEED: u16 = 0xDE;
pub const ICON_CROSS_TRACK_ERROR: u16 = 0xFC;
pub const ICON_BLACKBOX: u16 = 0xFE;
pub const ARTIFICIAL_HORIZON_START: u16 = 0x14C;
pub const ICON_REMAINING_DISTANCE: u16 = 0x167;
pub const ICON_ODOMETER: u16 = 0x168;
pub const LABEL_RX_BAND: u16 = 0x169;
pub const LABEL_RX_MODE: u16 = 0x16A;
pub const CROSSHAIR_TYPE_7: u16 = 0x19C;

/// Names of the glyph codes that the rules read, sorted by code.
pub const NAMES: &[(u16, &str)] = &[
    (0x01, "ICON_RSSI"),
    (0x02, "ICON_LQ"),
    (0x03, "LABEL_LAT"),
    (0x04, "LABEL_LON"),
    (0x05, "ICON_AZIMUTH"),
    (0x06, "ICON_TELEMETRY_0"),
    (0x07, "ICON_TELEMETRY_1"),
    (0x08, "ICON_SAT_L"),
    (0x09, "ICON_SAT_R"),
    (0x0B, "UNIT_DEG"),
    (0x0C, "ICON_HEADING"),
    (0x0D, "ICON_SCALE"),
    (0x0E, "LABEL_HDOP_L"),
    (0x0F, "LABEL_HDOP_R"),
    (0x10, "ICON_HOME"),
    (0x11, "ICON_RSSI_ALT"),
    (0x12, "UNIT_DB"),
    (0x13, "UNIT_DBM"),
    (0x14, "LABEL_SNR"),
    (0x15, "ARROW_UP_SMALL"),
    (0x16, "ARROW_DOWN_SMALL"),
    (0x1F, "UNIT_V"),
    (0x22, "UNIT_AH_PER_KM"),
    (0x27, "LABEL_VTX_POWER"),
    (0x60, "UNIT_MAH_PER_NM_0"),
    (0x61, "UNIT_MAH_PER_NM_1"),
    (0x62, "UNIT_MOHM"),
    (0x63, "ICON_BATT_FULL"),
    (0x64, "ICON_BATT_5"),
    (0x65, "ICON_BATT_4"),
    (0x66, "ICON_BATT_3"),
    (0x69, "ICON_BATT_EMPTY"),
    (0x6A, "UNIT_A"),
    (0x6B, "UNIT_MAH_PER_KM_0"),
    (0x6C, "UNIT_MAH_PER_KM_1"),
    (0x6D, "UNIT_WH"),
    (0x6E, "UNIT_WH_PER_KM"),
    (0x6F, "UNIT_WH_PER_MI"),
    (0x70, "UNIT_WH_PER_NM"),
    (0x71, "UNIT_W"),
    (0x72, "UNIT_MW"),
    (0x73, "UNIT_KW"),
    (0x75, "ICON_TRIP_DIST"),
    (0x76, "UNIT_ALT_M"),
    (0x77, "UNIT_ALT_KM"),
    (0x78, "UNIT_ALT_FT"),
    (0x79, "UNIT_ALT_KFT"),
    (0x7A, "UNIT_DIST_M"),
    (0x7E, "UNIT_KM"),
    (0x7F, "UNIT_FT"),
    (0x80, "UNIT_DIST_MI"),
    (0x81, "UNIT_DIST_NM"),
    (0x83, "UNIT_KM_LONG"),
    (0x84, "UNIT_MI_LONG"),
    (0x85, "UNIT_NM_LONG"),
    (0x86, "ICON_WIND_H"),
    (0x87, "ICON_WIND_V"),
    (0x88, "UNIT_KMH_3D"),
    (0x89, "UNIT_MPH_3D"),
    (0x8A, "UNIT_KT_3D"),
    (0x8B, "ICON_RPM"),
    (0x8C, "ICON_AIR_SPEED"),
    (0x8D, "UNIT_FT_PER_S"),
    (0x8E, "UNIT_100FT_PER_MIN"),
    (0x8F, "UNIT_M_PER_S"),
    (0x90, "UNIT_KMH"),
    (0x91, "UNIT_MPH"),
    (0x92, "UNIT_KT"),
    (0x93, "UNIT_MAH_PER_MI_0"),
    (0x94, "UNIT_MAH_PER_MI_1"),
    (0x95, "ICON_THROTTLE"),
    (0x96, "UNIT_F"),
    (0x97, "UNIT_C"),
    (0x99, "UNIT_MAH"),
    (0x9A, "ICON_ON_H"),
    (0x9B, "ICON_FLIGHT_H"),
    (0x9C, "ICON_GLIDESLOPE"),
    (0x9E, "ICON_ON_MIN"),
    (0x9F, "ICON_FLIGHT_MIN"),
    (0xA0, "ICON_CLOCK"),
    (0xAB, "LABEL_AUTO_THR_0"),
    (0xAC, "LABEL_AUTO_THR_1"),
    (0xAD, "ICON_ROLL_LEFT"),
    (0xAE, "ICON_ROLL_LEVEL"),
    (0xAF, "ICON_ROLL_RIGHT"),
    (0xB0, "ICON_PITCH_UP"),
    (0xBB, "ICON_PITCH_DOWN"),
    (0xBC, "ICON_GFORCE"),
    (0xBD, "ICON_GFORCE_X"),
    (0xBE, "ICON_GFORCE_Y"),
    (0xBF, "ICON_GFORCE_Z"),
    (0xC0, "ICON_BARO_TEMP"),
    (0xC1, "ICON_IMU_TEMP"),
    (0xC2, "ICON_TEMP_SENSOR_FIRST"),
    (0xC3, "ICON_ESC_TEMP"),
    (0xC7, "ICON_TEMP_SENSOR_LAST"),
    (0xCE, "LABEL_MAX"),
    (0xCF, "ICON_PROFILE"),
    (0xD3, "UNIT_AH"),
    (0xD4, "ICON_GLIDE_DIST"),
    (0xD5, "ICON_GLIDE_MIN"),
    (0xD6, "UNIT_CLIMB_FT_0"),
    (0xD7, "UNIT_CLIMB_FT_1"),
    (0xD8, "UNIT_CLIMB_M_0"),
    (0xD9, "UNIT_CLIMB_M_1"),
    (0xDA, "ICON_REMAIN_MIN"),
    (0xDB, "ICON_REMAIN_H"),
    (0xDC, "ICON_GROUND_COURSE"),
    (0xDD, "ICON_ALERT"),
    (0xDE, "ICON_MIN_GS"),
    (0xFC, "ICON_XTRACK"),
    (0xFE, "ICON_BLACKBOX"),
    (0x14C, "AH_START"),
    (0x167, "ICON_REMAIN_DIST"),
    (0x168, "ICON_ODOMETER"),
    (0x169, "LABEL_RX_BAND"),
    (0x16A, "LABEL_RX_MODE"),
    (0x19C, "CROSSHAIR_TYPE_7"),
];

/// The name of a glyph code, or `None` for codes with no name in the table.
#[must_use]
pub fn name_of(code: u16) -> Option<&'static str> {
    NAMES
        .binary_search_by_key(&code, |&(c, _)| c)
        .ok()
        .map(|index| NAMES[index].1)
}
