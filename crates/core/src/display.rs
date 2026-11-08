// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Text for people: how times and sizes are written in messages and on screen.
//!
//! These live here so the messages of the core and the screens of an application write them the same way.

/// A length of time as `m:ss`, or `h:mm:ss` from one hour. Seconds are rounded.
#[must_use]
pub fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// A time as `m:ss.s`, with tenths of a second, for a position in a video. Minutes go past 59 instead of becoming
/// hours.
#[must_use]
pub fn clock_tenths(seconds: f64) -> String {
    let total = seconds.max(0.0);
    let minutes = (total / 60.0).floor();
    format!("{minutes:.0}:{:04.1}", total - minutes * 60.0)
}

/// A size in bytes for reading, in decimal units: `950 MB`, `5.2 GB`, `182 GB`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "a size for reading does not need every byte"
)]
pub fn size(bytes: u64) -> String {
    let bytes = bytes as f64;
    if bytes >= 10e9 {
        format!("{:.0} GB", bytes / 1e9)
    } else if bytes >= 1e9 {
        format!("{:.1} GB", bytes / 1e9)
    } else {
        format!("{:.0} MB", bytes / 1e6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_is_written_in_the_unit_that_reads_best() {
        assert_eq!(size(950_000_000), "950 MB");
        assert_eq!(size(114_768_076), "115 MB");
        assert_eq!(size(5_240_000_000), "5.2 GB");
        assert_eq!(size(182_400_000_000), "182 GB");
        assert_eq!(size(0), "0 MB");
    }

    #[test]
    fn a_length_is_written_in_minutes_and_hours() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(59.6), "1:00", "the seconds are rounded");
        assert_eq!(clock(754.0), "12:34");
        assert_eq!(clock(3600.0), "1:00:00");
        assert_eq!(clock(-5.0), "0:00", "a negative length is none");
    }

    #[test]
    fn a_position_has_tenths_and_no_hours() {
        assert_eq!(clock_tenths(0.0), "0:00.0");
        assert_eq!(clock_tenths(1.74), "0:01.7");
        assert_eq!(clock_tenths(83.25), "1:23.2");
        assert_eq!(clock_tenths(3725.0), "62:05.0");
    }
}
