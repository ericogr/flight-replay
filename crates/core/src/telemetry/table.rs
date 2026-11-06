// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/telemetry/table.go (Apache-2.0, Copyright 2026 EricoGR).

use std::collections::HashMap;

use super::series::{Series, SeriesError};

/// What could be read at one instant of the recording. A reading that was not readable at that instant (blinking
/// element, other screen) is absent from the maps: it is never stored as zero.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    /// Time, in seconds.
    pub time: f64,
    /// Numeric readings by column name.
    pub numbers: HashMap<String, f64>,
    /// Text readings by column name.
    pub texts: HashMap<String, String>,
}

/// What building a table had to repair.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildInfo {
    /// Rows folded into another row with the same timestamp.
    pub merged: usize,
    /// Whether the rows were not in time order and have been sorted.
    pub reordered: bool,
}

/// Errors when building or reading a table.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TableError {
    /// No rows were given.
    #[error("no samples")]
    NoSamples,
    /// A row has a time that is not a finite number.
    #[error("sample with an invalid time ({0})")]
    InvalidTime(f64),
    /// A column has both numbers and texts.
    #[error("column {0:?} has both numeric and text readings")]
    MixedColumn(String),
    /// The recording has no reading of the column.
    #[error("column {0:?} has no readings in the recording")]
    NoReadings(String),
    /// The column holds text.
    #[error("column {0:?} is text, not a number")]
    NotNumeric(String),
    /// The column has no numeric reading at all.
    #[error(transparent)]
    Series(#[from] SeriesError),
}

#[derive(Debug, Clone, PartialEq)]
enum Column {
    /// NaN means no reading.
    Numbers(Vec<f64>),
    /// An empty string means no reading.
    Texts(Vec<String>),
}

/// A recording as columns over a strictly increasing time axis.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// Names of the columns that have at least one reading, sorted.
    pub header: Vec<String>,
    /// Seconds, strictly increasing.
    pub times: Vec<f64>,
    /// What had to be repaired.
    pub info: BuildInfo,
    columns: HashMap<String, Column>,
}

impl Table {
    /// Builds a table from rows. They are sorted by time and rows with the same timestamp are merged (a later
    /// reading wins), so the time axis is strictly increasing and safe to interpolate on. A row without any
    /// reading is kept: the recording still lasted until then. A column only exists if at least one row has a
    /// reading for it.
    #[expect(
        clippy::float_cmp,
        reason = "rows with exactly the same timestamp are merged"
    )]
    pub fn new(mut rows: Vec<Row>) -> Result<Self, TableError> {
        if rows.is_empty() {
            return Err(TableError::NoSamples);
        }
        let mut info = BuildInfo::default();
        if !rows.windows(2).all(|pair| pair[0].time <= pair[1].time) {
            info.reordered = true;
            rows.sort_by(|a, b| a.time.total_cmp(&b.time));
        }

        let mut merged: Vec<Row> = Vec::with_capacity(rows.len());
        for row in rows {
            if !row.time.is_finite() {
                return Err(TableError::InvalidTime(row.time));
            }
            match merged.last_mut() {
                Some(previous) if previous.time == row.time => {
                    previous.numbers.extend(row.numbers);
                    previous.texts.extend(row.texts);
                    info.merged += 1;
                }
                _ => merged.push(row),
            }
        }

        let n = merged.len();
        let mut columns: HashMap<String, Column> = HashMap::new();
        let times: Vec<f64> = merged.iter().map(|row| row.time).collect();
        for (index, row) in merged.iter().enumerate() {
            for (key, &value) in &row.numbers {
                if !value.is_finite() {
                    continue;
                }
                match columns
                    .entry(key.clone())
                    .or_insert_with(|| Column::Numbers(vec![f64::NAN; n]))
                {
                    Column::Numbers(values) => values[index] = value,
                    Column::Texts(_) => return Err(TableError::MixedColumn(key.clone())),
                }
            }
            for (key, value) in &row.texts {
                if value.trim().is_empty() {
                    continue;
                }
                match columns
                    .entry(key.clone())
                    .or_insert_with(|| Column::Texts(vec![String::new(); n]))
                {
                    Column::Texts(values) => values[index].clone_from(value),
                    Column::Numbers(_) => return Err(TableError::MixedColumn(key.clone())),
                }
            }
        }
        let mut header: Vec<String> = columns.keys().cloned().collect();
        header.sort();
        Ok(Self {
            header,
            times,
            info,
            columns,
        })
    }

    /// Whether any row has a reading for this column.
    #[must_use]
    pub fn has_column(&self, name: &str) -> bool {
        self.columns.contains_key(name)
    }

    /// The number of rows kept.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.times.len()
    }

    /// The time of the last row, in seconds.
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.times[self.times.len() - 1]
    }

    /// The column as numbers, with NaN where there is no reading.
    pub fn numeric(&self, name: &str) -> Result<Vec<f64>, TableError> {
        match self.columns.get(name) {
            None => Err(TableError::NoReadings(name.to_owned())),
            Some(Column::Texts(_)) => Err(TableError::NotNumeric(name.to_owned())),
            Some(Column::Numbers(values)) => Ok(values.clone()),
        }
    }

    /// The named numeric column as a forward and back filled series.
    pub fn series(&self, name: &str) -> Result<Series, TableError> {
        let raw = self.numeric(name)?;
        Ok(Series::new(name, &self.times, &raw)?)
    }

    /// The index of the row whose timestamp is nearest to `seconds`.
    #[must_use]
    pub fn row_index_at(&self, seconds: f64) -> usize {
        let i = self.times.partition_point(|&t| t < seconds);
        if i == 0 {
            0
        } else if i >= self.times.len() {
            self.times.len() - 1
        } else if seconds - self.times[i - 1] <= self.times[i] - seconds {
            i - 1
        } else {
            i
        }
    }

    /// The text of one reading. When the row has none it walks back to the last row that does, but never further
    /// than `max_age` seconds (0 means no limit): some columns, like the OSD message, only exist while displayed, so
    /// carrying them for long would be wrong. The flag says whether the value came from an earlier row. `None`
    /// means there is no recent reading.
    #[must_use]
    pub fn cell(&self, row: usize, name: &str, max_age: f64) -> Option<(String, bool)> {
        let column = self.columns.get(name)?;
        if row >= self.times.len() {
            return None;
        }
        for r in (0..=row).rev() {
            if max_age > 0.0 && self.times[row] - self.times[r] > max_age {
                return None;
            }
            let text = match column {
                Column::Texts(values) => values[r].clone(),
                Column::Numbers(values) if !values[r].is_nan() => format_value(values[r]),
                Column::Numbers(_) => String::new(),
            };
            if !text.is_empty() {
                return Some((text, r != row));
            }
        }
        None
    }
}

/// Shows a reading with at most three decimals and no trailing zeros.
#[must_use]
pub fn format_value(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" || text.is_empty() {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(time: f64, numbers: &[(&str, f64)], texts: &[(&str, &str)]) -> Row {
        Row {
            time,
            numbers: numbers.iter().map(|&(k, v)| (k.to_owned(), v)).collect(),
            texts: texts
                .iter()
                .map(|&(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
        }
    }

    #[test]
    fn sorts_and_merges_rows_with_the_same_time() {
        let rows = vec![
            row(2.0, &[("a", 3.0)], &[]),
            row(1.0, &[("a", 1.0)], &[]),
            row(1.0, &[("a", 2.0), ("b", 9.0)], &[]),
        ];
        let table = Table::new(rows).unwrap();
        assert!(table.info.reordered);
        assert_eq!(table.info.merged, 1);
        assert_eq!(table.times, vec![1.0, 2.0]);
        assert_eq!(table.numeric("a").unwrap(), vec![2.0, 3.0]);
        let b = table.numeric("b").unwrap();
        assert_eq!(b[0], 9.0);
        assert!(b[1].is_nan());
    }

    #[test]
    fn columns_exist_only_with_a_reading() {
        let table = Table::new(vec![row(0.0, &[], &[]), row(1.0, &[("a", 1.0)], &[])]).unwrap();
        assert_eq!(table.header, vec!["a".to_owned()]);
        assert!(!table.has_column("zzz"));
        assert!(matches!(
            table.numeric("zzz"),
            Err(TableError::NoReadings(_))
        ));
    }

    #[test]
    fn rejects_mixed_columns_empty_input_and_bad_times() {
        let mixed = vec![row(0.0, &[("a", 1.0)], &[]), row(1.0, &[], &[("a", "x")])];
        assert!(matches!(Table::new(mixed), Err(TableError::MixedColumn(_))));
        assert_eq!(Table::new(vec![]), Err(TableError::NoSamples));
        assert!(matches!(
            Table::new(vec![row(f64::NAN, &[], &[])]),
            Err(TableError::InvalidTime(_))
        ));
    }

    #[test]
    fn cells_carry_back_within_the_age_limit() {
        let table = Table::new(vec![
            row(0.0, &[], &[("msg", "HELLO")]),
            row(1.0, &[], &[]),
            row(5.0, &[], &[]),
        ])
        .unwrap();
        assert_eq!(table.cell(0, "msg", 0.0), Some(("HELLO".into(), false)));
        assert_eq!(table.cell(1, "msg", 2.0), Some(("HELLO".into(), true)));
        assert_eq!(table.cell(2, "msg", 2.0), None);
        assert_eq!(table.cell(2, "msg", 0.0), Some(("HELLO".into(), true)));
    }

    #[test]
    fn nearest_row_and_value_formatting() {
        let table = Table::new(vec![
            row(0.0, &[], &[]),
            row(1.0, &[], &[]),
            row(3.0, &[], &[]),
        ])
        .unwrap();
        assert_eq!(table.row_index_at(0.4), 0);
        assert_eq!(table.row_index_at(0.6), 1);
        assert_eq!(table.row_index_at(100.0), 2);
        assert_eq!(format_value(1.5), "1.5");
        assert_eq!(format_value(2.0), "2");
        assert_eq!(format_value(-0.0004), "0");
        assert_eq!(format_value(1.23456), "1.235");
    }
}
