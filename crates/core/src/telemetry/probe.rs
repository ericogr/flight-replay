// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/pipeline/probe.go (Apache-2.0, Copyright 2026 EricoGR).

//! Shows the OSD frame that the time mapping predicts for a video time, to check the synchronisation by eye:
//! compare the values with the OSD burned into the video frame at that time. If the mapping is right they agree
//! within one OSD frame (about 0.15 s).

use super::table::Table;
use crate::timeline::Timeline;

/// How long, in seconds, a reading is carried forward when an OSD frame has none (the OSD samples its fields
/// slower than it draws frames).
pub const CARRY_MAX_AGE_S: f64 = 2.0;

/// The columns shown by default.
pub const DEFAULT_PROBE_COLUMNS: [&str; 8] = [
    "osd_flight_time_s",
    "osd_throttle_pct",
    "osd_current_a",
    "osd_air_speed_kmh",
    "osd_altitude_m",
    "osd_mah_used",
    "osd_flight_mode",
    "osd_message",
];

/// Where a video time falls on the telemetry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProbePosition {
    /// Before the first OSD frame.
    BeforeStart,
    /// After the last OSD frame.
    AfterEnd {
        /// Time of the last frame, in seconds.
        data_end: f64,
    },
    /// Inside the recording: the nearest OSD frame.
    Inside {
        /// Index of the frame.
        row: usize,
        /// Time of the frame, in seconds.
        row_time: f64,
    },
}

/// One column of a probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeValue {
    /// Column name.
    pub column: String,
    /// The reading, or `None` if there is no recent one.
    pub value: Option<String>,
    /// Whether the reading was carried forward from an earlier frame.
    pub carried: bool,
}

/// What the telemetry shows at a video time.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeReport {
    /// The video time asked for, in seconds.
    pub video_time: f64,
    /// The telemetry time it maps to, in seconds.
    pub telemetry_time: f64,
    /// Where that falls on the recording.
    pub position: ProbePosition,
    /// The chosen columns that exist in the recording. Empty outside the recording.
    pub values: Vec<ProbeValue>,
}

/// Looks up the OSD frame that `timeline` predicts for `video_time` and reads `columns` from it.
#[must_use]
pub fn probe_at(
    table: &Table,
    timeline: &Timeline,
    video_time: f64,
    columns: &[&str],
) -> ProbeReport {
    let telemetry_time = timeline.telemetry_time(video_time);
    let (position, values) = if telemetry_time < 0.0 {
        (ProbePosition::BeforeStart, Vec::new())
    } else if telemetry_time > table.duration() {
        (
            ProbePosition::AfterEnd {
                data_end: table.duration(),
            },
            Vec::new(),
        )
    } else {
        let row = table.row_index_at(telemetry_time);
        let values = columns
            .iter()
            .filter(|column| table.has_column(column))
            .map(|column| {
                let cell = table.cell(row, column, CARRY_MAX_AGE_S);
                ProbeValue {
                    column: (*column).to_owned(),
                    carried: cell.as_ref().is_some_and(|(_, carried)| *carried),
                    value: cell.map(|(text, _)| text),
                }
            })
            .collect();
        (
            ProbePosition::Inside {
                row,
                row_time: table.times[row],
            },
            values,
        )
    };
    ProbeReport {
        video_time,
        telemetry_time,
        position,
        values,
    }
}

impl ProbeReport {
    /// The values on one line, as `column=value` pairs. A value carried forward ends with `*`, and a missing one
    /// is `-`.
    #[must_use]
    pub fn values_text(&self) -> String {
        if self.values.is_empty() {
            return "(none of the columns exists in the recording)".to_owned();
        }
        self.values
            .iter()
            .map(|v| {
                let mut text = v.value.clone().unwrap_or_else(|| "-".to_owned());
                if v.carried {
                    text.push('*');
                }
                format!("{}={}", v.column, text)
            })
            .collect::<Vec<_>>()
            .join("  ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::table::Row;
    use crate::timeline::{ScaleMode, SyncOptions};

    fn table() -> Table {
        let rows = (0..10)
            .map(|i| {
                let mut row = Row {
                    time: f64::from(i),
                    ..Row::default()
                };
                if i % 2 == 0 {
                    row.numbers
                        .insert("osd_throttle_pct".into(), f64::from(i) * 10.0);
                }
                row.texts.insert("osd_flight_mode".into(), "ACRO".into());
                row
            })
            .collect();
        Table::new(rows).unwrap()
    }

    fn straight() -> Timeline {
        Timeline::new(
            SyncOptions {
                scale: ScaleMode::Off,
                offset_s: 0.0,
            },
            10.0,
            10.0,
        )
    }

    #[test]
    fn reads_the_nearest_frame_and_marks_carried_values() {
        let report = probe_at(
            &table(),
            &straight(),
            3.0,
            &["osd_throttle_pct", "osd_flight_mode", "osd_missing"],
        );
        assert_eq!(
            report.position,
            ProbePosition::Inside {
                row: 3,
                row_time: 3.0
            }
        );
        // frame 3 has no throttle: carried from frame 2
        assert_eq!(
            report.values_text(),
            "osd_throttle_pct=20*  osd_flight_mode=ACRO"
        );
    }

    #[test]
    fn reports_times_outside_the_recording() {
        let before = probe_at(&table(), &straight(), -1.0, &["osd_flight_mode"]);
        assert_eq!(before.position, ProbePosition::BeforeStart);
        let after = probe_at(&table(), &straight(), 50.0, &["osd_flight_mode"]);
        assert!(matches!(after.position, ProbePosition::AfterEnd { .. }));
        assert_eq!(
            after.values_text(),
            "(none of the columns exists in the recording)"
        );
    }
}
