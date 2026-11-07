// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/telemetry/series.go (Apache-2.0, Copyright 2026 EricoGR).

/// Summary of the rows that actually had a reading, before filling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    /// Rows with a reading.
    pub valid: usize,
    /// Rows.
    pub total: usize,
    /// Smallest reading.
    pub min: f64,
    /// Largest reading.
    pub max: f64,
    /// Mean of the readings.
    pub mean: f64,
    /// Time of the first row that had a reading, in seconds.
    pub first_time: f64,
    /// Time of the last row that had a reading, in seconds.
    pub last_time: f64,
}

impl Stats {
    /// The share of rows that had a reading, in percent.
    #[must_use]
    pub fn coverage_pct(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            100.0 * self.valid as f64 / self.total as f64
        }
    }
}

/// Errors when building a series.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SeriesError {
    /// The column has no numeric reading at all.
    #[error("series {0}: no numeric reading at all")]
    NoData(String),
    /// The number of times differs from the number of values.
    #[error("series {name}: {times} times for {values} values")]
    LengthMismatch {
        /// Series name.
        name: String,
        /// Number of times.
        times: usize,
        /// Number of values.
        values: usize,
    },
}

/// A time-ordered signal, already forward-filled (and back-filled at the start), so every row has a value. Times are
/// in seconds and strictly increasing.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// Column name.
    pub name: String,
    /// Times, in seconds.
    pub times: Vec<f64>,
    /// Values, one per time.
    pub values: Vec<f64>,
    /// Summary before filling.
    pub stats: Stats,
}

impl Series {
    /// Builds a filled series. `raw` uses NaN for "no data". `times` must be strictly increasing (a table
    /// guarantees it).
    pub fn new(name: &str, times: &[f64], raw: &[f64]) -> Result<Self, SeriesError> {
        if times.len() != raw.len() {
            return Err(SeriesError::LengthMismatch {
                name: name.to_owned(),
                times: times.len(),
                values: raw.len(),
            });
        }
        let mut stats = Stats {
            valid: 0,
            total: raw.len(),
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            mean: 0.0,
            first_time: 0.0,
            last_time: 0.0,
        };
        let mut sum = 0.0;
        for (index, &value) in raw.iter().enumerate() {
            if value.is_nan() {
                continue;
            }
            if stats.valid == 0 {
                stats.first_time = times[index];
            }
            stats.valid += 1;
            stats.last_time = times[index];
            sum += value;
            stats.min = stats.min.min(value);
            stats.max = stats.max.max(value);
        }
        if stats.valid == 0 {
            return Err(SeriesError::NoData(name.to_owned()));
        }
        stats.mean = sum / stats.valid as f64;
        Ok(Self {
            name: name.to_owned(),
            times: times.to_vec(),
            values: fill_forward(raw),
            stats,
        })
    }

    /// The value at `time` seconds, linearly interpolated. Outside the recorded range the first or last value is
    /// held.
    #[must_use]
    pub fn at(&self, time: f64) -> f64 {
        let n = self.times.len();
        if n == 0 {
            return f64::NAN;
        }
        if time <= self.times[0] {
            return self.values[0];
        }
        if time >= self.times[n - 1] {
            return self.values[n - 1];
        }
        // first index with times[i] >= time, so i >= 1 here
        let i = self.times.partition_point(|&t| t < time);
        lerp(
            self.times[i - 1],
            self.values[i - 1],
            self.times[i],
            self.values[i],
            time,
        )
    }

    /// Sets the series to zero from `max_age` seconds after its last reading. Until then the last reading is held,
    /// as a blinking element is. After that the value is gone (the OSD no longer shows it), so holding it forever
    /// would be wrong. Call it before the series is read.
    pub fn zero_after_last_reading(&mut self, max_age: f64) {
        let limit = self.stats.last_time + max_age;
        for (time, value) in self.times.iter().zip(self.values.iter_mut()) {
            if *time > limit {
                *value = 0.0;
            }
        }
    }

    /// Sets the series to zero until its first reading. Before that the OSD did not show the value (a splash screen,
    /// an element that is not drawn yet), and [`fill_forward`] would otherwise copy the first reading back over all
    /// of it. Call it before the series is read.
    pub fn zero_before_first_reading(&mut self) {
        let first = self.stats.first_time;
        for (time, value) in self.times.iter().zip(self.values.iter_mut()) {
            if *time < first {
                *value = 0.0;
            }
        }
    }

    /// A cursor that evaluates the series at mostly increasing times, in amortised constant time.
    #[must_use]
    pub fn cursor(&self) -> Cursor<'_> {
        Cursor {
            series: self,
            index: 0,
        }
    }
}

/// Returns a copy of `raw` where NaN cells repeat the last valid value; leading NaNs are back-filled with the first
/// valid value. If there is no valid value at all the result is all NaN.
#[must_use]
pub fn fill_forward(raw: &[f64]) -> Vec<f64> {
    let mut out = raw.to_vec();
    let Some(first) = out.iter().position(|v| !v.is_nan()) else {
        return out;
    };
    let first_value = out[first];
    out[..first].fill(first_value);
    for i in first + 1..out.len() {
        if out[i].is_nan() {
            out[i] = out[i - 1];
        }
    }
    out
}

/// Evaluates a [`Series`] at mostly increasing times in amortised constant time.
#[derive(Debug, Clone)]
pub struct Cursor<'a> {
    series: &'a Series,
    /// `times[index] <= t < times[index + 1]` for the last query.
    index: usize,
}

impl Cursor<'_> {
    /// Gives the same value as [`Series::at`], but seeks from the previous position.
    pub fn at(&mut self, time: f64) -> f64 {
        let s = self.series;
        let n = s.times.len();
        if n == 0 {
            return f64::NAN;
        }
        if time <= s.times[0] {
            self.index = 0;
            return s.values[0];
        }
        if time >= s.times[n - 1] {
            self.index = n - 1;
            return s.values[n - 1];
        }
        if self.index >= n - 1 || s.times[self.index] > time {
            // random access or a jump backwards
            self.index = s.times.partition_point(|&t| t < time).saturating_sub(1);
        }
        while s.times[self.index + 1] < time {
            self.index += 1;
        }
        lerp(
            s.times[self.index],
            s.values[self.index],
            s.times[self.index + 1],
            s.values[self.index + 1],
            time,
        )
    }
}

/// The value at `time` of a signal that may be absent: `default` when there is no cursor.
#[must_use]
pub fn at_or(cursor: Option<&mut Cursor<'_>>, time: f64, default: f64) -> f64 {
    cursor.map_or(default, |c| c.at(time))
}

fn lerp(t0: f64, v0: f64, t1: f64, v1: f64, t: f64) -> f64 {
    if t1 <= t0 {
        return v1;
    }
    v0 + (v1 - v0) * (t - t0) / (t1 - t0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(times: &[f64], raw: &[f64]) -> Series {
        Series::new("test", times, raw).unwrap()
    }

    #[test]
    fn fills_forward_and_back() {
        let filled = fill_forward(&[f64::NAN, f64::NAN, 3.0, f64::NAN, 5.0, f64::NAN]);
        assert_eq!(filled, vec![3.0, 3.0, 3.0, 3.0, 5.0, 5.0]);
        assert!(fill_forward(&[f64::NAN]).iter().all(|v| v.is_nan()));
    }

    #[test]
    fn interpolates_and_holds_the_ends() {
        let s = series(&[0.0, 1.0, 2.0], &[0.0, 10.0, 30.0]);
        assert!((s.at(0.5) - 5.0).abs() < 1e-12);
        assert!((s.at(1.5) - 20.0).abs() < 1e-12);
        assert_eq!(s.at(-3.0), 0.0);
        assert_eq!(s.at(9.0), 30.0);
    }

    #[test]
    fn the_cursor_matches_direct_evaluation_going_forward_and_back() {
        let times: Vec<f64> = (0..50).map(f64::from).collect();
        let raw: Vec<f64> = times.iter().map(|t| (t * 0.37).sin()).collect();
        let s = series(&times, &raw);
        let mut cursor = s.cursor();
        for &t in &[0.2, 0.9, 3.3, 3.31, 20.5, 49.0, 7.7, 0.1, 30.0, 12.25] {
            assert!((cursor.at(t) - s.at(t)).abs() < 1e-12, "t = {t}");
        }
    }

    #[test]
    fn reports_stats_of_the_real_readings() {
        let s = series(&[0.0, 1.0, 2.0, 3.0], &[f64::NAN, 4.0, f64::NAN, 8.0]);
        assert_eq!(s.stats.valid, 2);
        assert_eq!(s.stats.min, 4.0);
        assert_eq!(s.stats.max, 8.0);
        assert!((s.stats.mean - 6.0).abs() < 1e-12);
        assert_eq!(s.stats.first_time, 1.0);
        assert_eq!(s.stats.last_time, 3.0);
        assert!((s.stats.coverage_pct() - 50.0).abs() < 1e-12);
    }

    #[test]
    fn a_column_without_readings_is_an_error() {
        assert!(matches!(
            Series::new("x", &[0.0, 1.0], &[f64::NAN, f64::NAN]),
            Err(SeriesError::NoData(_))
        ));
    }

    #[test]
    fn zeroes_the_series_after_its_last_reading() {
        let mut s = series(
            &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            &[5.0, 6.0, f64::NAN, f64::NAN, f64::NAN, f64::NAN],
        );
        s.zero_after_last_reading(2.0);
        // last reading at t = 1; held until t = 3, zero after
        assert_eq!(s.values, vec![5.0, 6.0, 6.0, 6.0, 0.0, 0.0]);
    }

    #[test]
    fn zeroes_the_series_before_its_first_reading() {
        let mut s = series(
            &[0.0, 1.0, 2.0, 3.0, 4.0],
            &[f64::NAN, f64::NAN, 8.0, f64::NAN, 9.0],
        );
        // back-filled by default
        assert_eq!(s.values, vec![8.0, 8.0, 8.0, 8.0, 9.0]);
        s.zero_before_first_reading();
        // the first reading is at t = 2: nothing before it, and the gap after it is still carried
        assert_eq!(s.values, vec![0.0, 0.0, 8.0, 8.0, 9.0]);
        assert_eq!(s.stats.first_time, 2.0);
    }

    #[test]
    fn a_series_that_starts_with_a_reading_is_not_changed_by_zeroing_the_start() {
        let mut s = series(&[0.0, 1.0, 2.0], &[4.0, f64::NAN, 6.0]);
        let before = s.values.clone();
        s.zero_before_first_reading();
        assert_eq!(s.values, before);
    }
}
