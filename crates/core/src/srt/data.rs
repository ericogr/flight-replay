// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/srt/frame.rs (GPL-3.0-only), Copyright (C) Alexander van Saase.
// Modified for Flight Replay: the fields are read one by one, so a missing field does not reject the line.

/// The data a Walksnail SRT line holds for one moment of the video.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SrtData {
    /// Signal level reported by the goggles.
    pub signal: u8,
    /// Video channel.
    pub channel: u8,
    /// Flight time, in seconds.
    pub flight_time_s: u32,
    /// Battery voltage of the aircraft, in volts.
    pub sky_battery_v: f32,
    /// Battery voltage of the goggles, in volts.
    pub ground_battery_v: f32,
    /// Video latency, in milliseconds.
    pub latency_ms: u32,
    /// Video bitrate, in megabits per second.
    pub bitrate_mbps: f32,
    /// Distance from the goggles to the aircraft, in metres.
    pub distance_m: u32,
}

impl SrtData {
    /// Reads a data line such as `Signal:4 CH:7 FlightTime:0 SBat:15.9V GBat:7.8V Delay:37ms Bitrate:25.0Mbps
    /// Distance:2m`.
    ///
    /// Returns `None` if the line has no `Signal` field, which marks lines that are not data (such as the
    /// debug lines of newer firmware). Other missing fields are zero.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut data = Self::default();
        let mut has_signal = false;
        for field in text.split_whitespace() {
            let Some((key, value)) = field.split_once(':') else {
                continue;
            };
            match key {
                "Signal" => {
                    data.signal = value.parse().unwrap_or(0);
                    has_signal = true;
                }
                "CH" => data.channel = value.parse().unwrap_or(0),
                "FlightTime" => data.flight_time_s = value.parse().unwrap_or(0),
                "SBat" => data.sky_battery_v = number(value, "V"),
                "GBat" => data.ground_battery_v = number(value, "V"),
                "Delay" => data.latency_ms = number(value, "ms") as u32,
                "Bitrate" => data.bitrate_mbps = number(value, "Mbps"),
                "Distance" => data.distance_m = number(value, "m") as u32,
                _ => {}
            }
        }
        has_signal.then_some(data)
    }
}

/// Reads a number with its unit, such as `15.9V`. Returns 0 if it cannot be read.
fn number(value: &str, unit: &str) -> f32 {
    value
        .strip_suffix(unit)
        .unwrap_or(value)
        .parse()
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_older_data_line() {
        let line =
            "Signal:4 CH:8 FlightTime:0 SBat:4.7V GBat:7.2V Delay:32ms Bitrate:25Mbps Distance:7m";
        let data = SrtData::parse(line).unwrap();
        assert_eq!(data.signal, 4);
        assert_eq!(data.channel, 8);
        assert!((data.sky_battery_v - 4.7).abs() < 1e-6);
        assert!((data.ground_battery_v - 7.2).abs() < 1e-6);
        assert_eq!(data.latency_ms, 32);
        assert!((data.bitrate_mbps - 25.0).abs() < 1e-6);
        assert_eq!(data.distance_m, 7);
    }

    #[test]
    fn parses_the_newer_data_line() {
        let line = "Signal:4 CH:7 FlightTime:0 SBat:16.7V GBat:12.5V Delay:25ms Bitrate:25.0Mbps Distance:1m";
        let data = SrtData::parse(line).unwrap();
        assert_eq!(data.channel, 7);
        assert_eq!(data.latency_ms, 25);
        assert_eq!(data.distance_m, 1);
    }

    #[test]
    fn debug_lines_are_not_data() {
        let line = "CH:1 MCS:4 SP[ 45 152  47 149] GP[ 49  48  45  47] Delay:31ms Frame:60";
        assert_eq!(SrtData::parse(line), None);
    }
}
