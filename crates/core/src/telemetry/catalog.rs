// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/catalog.go (Apache-2.0, Copyright 2026 EricoGR), which comes from
// fpv-osd-decoder (MIT, Copyright (c) 2026 EricoGR). Converted to Rust once, by a script. This file is now the
// source of truth.

//! The columns the decoder can produce, with their keys and descriptions.
//!
//! The unit of a column is the suffix of its key (`osd_altitude_m`).

#![allow(missing_docs)]

use std::borrow::Cow;

pub const KEY_RSSI_DBM: &str = "osd_rssi_dbm";
pub const KEY_RSSI_ANTENNA: &str = "osd_rssi_antenna";
pub const KEY_RF_MODE: &str = "osd_rf_mode";
pub const KEY_LQ_PCT: &str = "osd_lq_pct";
pub const KEY_FLIGHT_TIME_S: &str = "osd_flight_time_s";
pub const KEY_BATT_CELL_V: &str = "osd_batt_cell_v";
pub const KEY_BATT_V: &str = "osd_batt_v";
pub const KEY_CURRENT_A: &str = "osd_current_a";
pub const KEY_MAH_USED: &str = "osd_mah_used";
pub const KEY_MAH_PER_KM: &str = "osd_mah_per_km";
pub const KEY_THROTTLE_PCT: &str = "osd_throttle_pct";
pub const KEY_THROTTLE_AUTO: &str = "osd_throttle_auto";
pub const KEY_MOTOR_IDLE: &str = "osd_motor_idle";
pub const KEY_FLIGHT_MODE: &str = "osd_flight_mode";
pub const KEY_MESSAGE: &str = "osd_message";
pub const KEY_HEADING_DEG: &str = "osd_heading_deg";
pub const KEY_ROLL_DEG: &str = "osd_roll_deg";
pub const KEY_PITCH_DEG: &str = "osd_pitch_deg";
pub const KEY_GFORCE: &str = "osd_gforce";
pub const KEY_GPS_SATS: &str = "osd_gps_sats";
pub const KEY_GPS_LAT: &str = "osd_gps_lat";
pub const KEY_GPS_LON: &str = "osd_gps_lon";
pub const KEY_GPS_SPEED_KMH: &str = "osd_gps_speed_kmh";
pub const KEY_AIR_SPEED_KMH: &str = "osd_air_speed_kmh";
pub const KEY_ALTITUDE_M: &str = "osd_altitude_m";
pub const KEY_HOME_DIST_M: &str = "osd_home_dist_m";
pub const KEY_HOME_DIR_DEG: &str = "osd_home_dir_deg";
pub const KEY_TRIP_DIST_M: &str = "osd_trip_dist_m";
pub const KEY_WIND_SPEED_KMH: &str = "osd_wind_speed_kmh";
pub const KEY_WIND_DIR_DEG: &str = "osd_wind_dir_deg";
pub const KEY_RSSI_PCT: &str = "osd_rssi_pct";
pub const KEY_LQ_DOWN_PCT: &str = "osd_lq_down_pct";
pub const KEY_SNR_DB: &str = "osd_snr_db";
pub const KEY_TX_POWER_MW: &str = "osd_tx_power_mw";
pub const KEY_RX_POWER_MW: &str = "osd_rx_power_mw";
pub const KEY_RX_BAND: &str = "osd_rx_band";
pub const KEY_RX_MODE: &str = "osd_rx_mode";
pub const KEY_VTX_CHANNEL: &str = "osd_vtx_channel";
pub const KEY_VTX_POWER: &str = "osd_vtx_power";
pub const KEY_ON_TIME_S: &str = "osd_on_time_s";
pub const KEY_RTH_TIME_LEFT_S: &str = "osd_rth_time_left_s";
pub const KEY_GLIDE_TIME_S: &str = "osd_glide_time_s";
pub const KEY_CLOCK: &str = "osd_clock";
pub const KEY_BATT_PCT: &str = "osd_batt_pct";
pub const KEY_WH_USED: &str = "osd_wh_used";
pub const KEY_WH_PER_KM: &str = "osd_wh_per_km";
pub const KEY_POWER_W: &str = "osd_power_w";
pub const KEY_IMPEDANCE_MOHM: &str = "osd_impedance_mohm";
pub const KEY_GPS_HDOP: &str = "osd_gps_hdop";
pub const KEY_HOME_HEADING_ERROR_DEG: &str = "osd_home_heading_error_deg";
pub const KEY_GROUND_COURSE_DEG: &str = "osd_ground_course_deg";
pub const KEY_AZIMUTH_DEG: &str = "osd_azimuth_deg";
pub const KEY_RTH_DIST_LEFT_M: &str = "osd_rth_dist_left_m";
pub const KEY_GLIDE_RANGE_M: &str = "osd_glide_range_m";
pub const KEY_CROSS_TRACK_ERROR_M: &str = "osd_cross_track_error_m";
pub const KEY_FENCE_DIST_H_M: &str = "osd_fence_dist_h_m";
pub const KEY_FENCE_DIST_V_M: &str = "osd_fence_dist_v_m";
pub const KEY_ODOMETER_KM: &str = "osd_odometer_km";
pub const KEY_RANGEFINDER_M: &str = "osd_rangefinder_m";
pub const KEY_GLIDESLOPE: &str = "osd_glideslope";
pub const KEY_WIND_VSPEED_KMH: &str = "osd_wind_vspeed_kmh";
pub const KEY_AIR_MAX_SPEED_KMH: &str = "osd_air_max_speed_kmh";
pub const KEY_GPS_MAX_SPEED_KMH: &str = "osd_gps_max_speed_kmh";
pub const KEY_3D_SPEED_KMH: &str = "osd_3d_speed_kmh";
pub const KEY_3D_MAX_SPEED_KMH: &str = "osd_3d_max_speed_kmh";
pub const KEY_MIN_GROUND_SPEED_KMH: &str = "osd_min_ground_speed_kmh";
pub const KEY_VSPEED_MS: &str = "osd_vspeed_ms";
pub const KEY_IMU_TEMP_C: &str = "osd_imu_temp_c";
pub const KEY_BARO_TEMP_C: &str = "osd_baro_temp_c";
pub const KEY_ESC_TEMP_C: &str = "osd_esc_temp_c";
pub const KEY_TEMP_SENSOR_C: &str = "osd_temp_sensor_c";
pub const KEY_ESC_RPM: &str = "osd_esc_rpm";
pub const KEY_GFORCE_X: &str = "osd_gforce_x";
pub const KEY_GFORCE_Y: &str = "osd_gforce_y";
pub const KEY_GFORCE_Z: &str = "osd_gforce_z";
pub const KEY_PROFILE: &str = "osd_profile";
pub const KEY_GVAR_0: &str = "osd_gvar_0";
pub const KEY_GVAR_1: &str = "osd_gvar_1";
pub const KEY_GVAR_2: &str = "osd_gvar_2";
pub const KEY_GVAR_3: &str = "osd_gvar_3";
pub const KEY_BLACKBOX_LOG: &str = "osd_blackbox_log";
pub const KEY_CLIMB_EFF_MAH_PER_M: &str = "osd_climb_eff_mah_per_m";

/// A telemetry column that can come out of the OSD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Column name, with the unit as suffix. The columns of the catalog have a static name; the ones the generic
    /// pass makes (`osd_raw_...`) have an owned one.
    pub key: Cow<'static, str>,
    /// What the value represents.
    pub desc: &'static str,
    /// Decimal places shown on screen. A negative number keeps all the digits.
    pub decimals: i32,
    /// True for a text value instead of a number.
    pub text: bool,
}

const fn field(key: &'static str, desc: &'static str, decimals: i32, text: bool) -> Field {
    Field {
        key: Cow::Borrowed(key),
        desc,
        decimals,
        text,
    }
}

/// Every column the decoder can produce, in the order they appear in a CSV.
pub const CATALOG: &[Field] = &[
    field(KEY_RSSI_DBM, "radio link RSSI (dBm)", 0, false),
    field(
        KEY_RSSI_ANTENNA,
        "active receiver antenna: 1 (RSSI icon) or 2 (2RSS icon)",
        0,
        false,
    ),
    field(KEY_RF_MODE, "CRSF link RF mode (the n in n:LQ)", 0, false),
    field(KEY_LQ_PCT, "CRSF link quality, uplink (%)", 0, false),
    field(
        KEY_FLIGHT_TIME_S,
        "armed flight time, counted by the flight controller (s)",
        0,
        false,
    ),
    field(
        KEY_BATT_CELL_V,
        "average cell voltage of the main battery (V)",
        2,
        false,
    ),
    field(
        KEY_BATT_V,
        "total voltage of the main battery (V)",
        2,
        false,
    ),
    field(KEY_CURRENT_A, "current draw (A)", 2, false),
    field(KEY_MAH_USED, "capacity used (mAh)", 0, false),
    field(KEY_MAH_PER_KM, "energy efficiency (mAh/km)", 0, false),
    field(KEY_THROTTLE_PCT, "throttle position (%)", 0, false),
    field(
        KEY_THROTTLE_AUTO,
        "1 if the throttle is controlled by navigation (auto-throttle), 0 if manual",
        0,
        false,
    ),
    field(
        KEY_MOTOR_IDLE,
        "1 if INAV shows IDLE in place of the throttle (armed, motor idling), 0 for any other throttle reading",
        0,
        false,
    ),
    field(
        KEY_FLIGHT_MODE,
        "flight mode (ACRO, ANGL, RTH, CRUZ...)",
        0,
        true,
    ),
    field(
        KEY_MESSAGE,
        "INAV status message (EN ROUTE TO HOME...)",
        0,
        true,
    ),
    field(
        KEY_HEADING_DEG,
        "aircraft heading (degrees, 0 = north)",
        0,
        false,
    ),
    field(
        KEY_ROLL_DEG,
        "roll (degrees; positive = right wing down)",
        1,
        false,
    ),
    field(
        KEY_PITCH_DEG,
        "pitch (degrees; positive = nose up)",
        1,
        false,
    ),
    field(KEY_GFORCE, "total G-force", 2, false),
    field(KEY_GPS_SATS, "GPS satellites in use", 0, false),
    field(KEY_GPS_LAT, "GPS latitude (decimal degrees)", 7, false),
    field(KEY_GPS_LON, "GPS longitude (decimal degrees)", 7, false),
    field(KEY_GPS_SPEED_KMH, "ground speed from GPS (km/h)", 1, false),
    field(
        KEY_AIR_SPEED_KMH,
        "airspeed from the pitot tube (km/h)",
        1,
        false,
    ),
    field(KEY_ALTITUDE_M, "altitude (m)", 1, false),
    field(KEY_HOME_DIST_M, "distance to the home point (m)", 1, false),
    field(
        KEY_HOME_DIR_DEG,
        "direction of home relative to the direction of flight (degrees, 0 = ahead; 22.5 steps)",
        1,
        false,
    ),
    field(
        KEY_TRIP_DIST_M,
        "total distance travelled in the flight (m)",
        1,
        false,
    ),
    field(
        KEY_WIND_SPEED_KMH,
        "estimated horizontal wind speed (km/h)",
        1,
        false,
    ),
    field(
        KEY_WIND_DIR_DEG,
        "wind direction relative to the nose (degrees; 45 steps)",
        0,
        false,
    ),
    field(
        KEY_RSSI_PCT,
        "RSSI as a percentage of the receiver's range (%)",
        0,
        false,
    ),
    field(KEY_LQ_DOWN_PCT, "CRSF link quality, downlink (%)", 0, false),
    field(KEY_SNR_DB, "link signal-to-noise ratio (dB)", 0, false),
    field(
        KEY_TX_POWER_MW,
        "radio transmitter power, uplink (mW)",
        0,
        false,
    ),
    field(
        KEY_RX_POWER_MW,
        "power of the downlink transmitter on the aircraft (mW)",
        0,
        false,
    ),
    field(KEY_RX_BAND, "receiver band (text)", 0, true),
    field(KEY_RX_MODE, "receiver mode (text)", 0, true),
    field(
        KEY_VTX_CHANNEL,
        "video transmitter band and channel (text, such as R1)",
        0,
        true,
    ),
    field(
        KEY_VTX_POWER,
        "video transmitter power level, as the letter or digit INAV shows (text)",
        0,
        true,
    ),
    field(
        KEY_ON_TIME_S,
        "time since the flight controller was powered (s)",
        0,
        false,
    ),
    field(
        KEY_RTH_TIME_LEFT_S,
        "estimated flight time left before the return to home must start (s)",
        0,
        false,
    ),
    field(
        KEY_GLIDE_TIME_S,
        "estimated time the aircraft can still glide (s)",
        0,
        false,
    ),
    field(KEY_CLOCK, "real-time clock (HH:MM:SS text)", 0, true),
    field(KEY_BATT_PCT, "remaining battery charge (%)", 0, false),
    field(KEY_WH_USED, "energy used (Wh)", 2, false),
    field(KEY_WH_PER_KM, "energy efficiency (Wh/km)", 2, false),
    field(KEY_POWER_W, "power draw (W)", 2, false),
    field(
        KEY_IMPEDANCE_MOHM,
        "power supply impedance (mOhm)",
        0,
        false,
    ),
    field(
        KEY_GPS_HDOP,
        "GPS horizontal dilution of precision",
        1,
        false,
    ),
    field(
        KEY_HOME_HEADING_ERROR_DEG,
        "angle between the direction of flight and the direction of home (degrees)",
        0,
        false,
    ),
    field(
        KEY_GROUND_COURSE_DEG,
        "GPS course over the ground (degrees)",
        0,
        false,
    ),
    field(
        KEY_AZIMUTH_DEG,
        "azimuth of the aircraft seen from home (degrees)",
        0,
        false,
    ),
    field(
        KEY_RTH_DIST_LEFT_M,
        "distance that can still be flown before the return to home must start (m)",
        1,
        false,
    ),
    field(
        KEY_GLIDE_RANGE_M,
        "distance the aircraft can still glide (m)",
        1,
        false,
    ),
    field(
        KEY_CROSS_TRACK_ERROR_M,
        "cross-track error from the waypoint leg (m)",
        1,
        false,
    ),
    field(
        KEY_FENCE_DIST_H_M,
        "horizontal distance to the nearest geofence zone (m)",
        1,
        false,
    ),
    field(
        KEY_FENCE_DIST_V_M,
        "vertical distance to the nearest geofence zone (m; negative = below it)",
        1,
        false,
    ),
    field(
        KEY_ODOMETER_KM,
        "distance travelled in all flights, as kept by the flight controller (km)",
        2,
        false,
    ),
    field(KEY_RANGEFINDER_M, "rangefinder distance (m)", 1, false),
    field(
        KEY_GLIDESLOPE,
        "glide ratio (horizontal distance per unit of height lost)",
        2,
        false,
    ),
    field(
        KEY_WIND_VSPEED_KMH,
        "estimated vertical wind speed (km/h; positive = up)",
        1,
        false,
    ),
    field(
        KEY_AIR_MAX_SPEED_KMH,
        "maximum airspeed of the flight (km/h)",
        1,
        false,
    ),
    field(
        KEY_GPS_MAX_SPEED_KMH,
        "maximum ground speed of the flight (km/h)",
        1,
        false,
    ),
    field(
        KEY_3D_SPEED_KMH,
        "3D speed, including the vertical component (km/h)",
        1,
        false,
    ),
    field(
        KEY_3D_MAX_SPEED_KMH,
        "maximum 3D speed of the flight (km/h)",
        1,
        false,
    ),
    field(
        KEY_MIN_GROUND_SPEED_KMH,
        "minimum ground speed set for navigation (km/h)",
        1,
        false,
    ),
    field(
        KEY_VSPEED_MS,
        "vertical speed (m/s; positive = climbing)",
        2,
        false,
    ),
    field(
        KEY_IMU_TEMP_C,
        "flight controller IMU temperature (Celsius)",
        1,
        false,
    ),
    field(KEY_BARO_TEMP_C, "barometer temperature (Celsius)", 1, false),
    field(KEY_ESC_TEMP_C, "ESC temperature (Celsius)", 1, false),
    field(
        KEY_TEMP_SENSOR_C,
        "external temperature sensor (Celsius)",
        1,
        false,
    ),
    field(
        KEY_ESC_RPM,
        "motor speed reported by the ESC (rpm)",
        0,
        false,
    ),
    field(KEY_GFORCE_X, "G-force on the X axis", 2, false),
    field(KEY_GFORCE_Y, "G-force on the Y axis", 2, false),
    field(KEY_GFORCE_Z, "G-force on the Z axis", 2, false),
    field(KEY_PROFILE, "active configuration profile", 0, false),
    field(
        KEY_GVAR_0,
        "global variable 0 of the INAV programming framework",
        0,
        false,
    ),
    field(
        KEY_GVAR_1,
        "global variable 1 of the INAV programming framework",
        0,
        false,
    ),
    field(
        KEY_GVAR_2,
        "global variable 2 of the INAV programming framework",
        0,
        false,
    ),
    field(
        KEY_GVAR_3,
        "global variable 3 of the INAV programming framework",
        0,
        false,
    ),
    field(
        KEY_BLACKBOX_LOG,
        "number of the blackbox log being recorded",
        0,
        false,
    ),
    field(
        KEY_CLIMB_EFF_MAH_PER_M,
        "climb efficiency (mAh per vertical metre, as the OSD shows it)",
        2,
        false,
    ),
];
