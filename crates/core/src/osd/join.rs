// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Several `.osd` files of one video, put on one timeline.
//!
//! The goggles write one `.osd` file for each recording. When the pilot joins several recordings into one video,
//! the video has one timeline and the `.osd` files must follow one another on it, in the order they were recorded.
//! The overlay and the sound read the joined file, so they cannot disagree about where a file starts.

use super::{Firmware, Frame, OsdFile};

/// Where one file of a joined recording sits on the joined timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct JoinPart {
    /// What the file is called, for messages.
    pub name: String,
    /// Time of its first screen on the joined timeline, in seconds.
    pub start_s: f64,
    /// Time at which its last screen stops being current (its last screen plus the average interval), in seconds.
    pub end_s: f64,
    /// Number of screens.
    pub frames: usize,
}

/// Why `.osd` files cannot be joined.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JoinError {
    /// No file was given.
    #[error("there are no .osd files to join")]
    Empty,
    /// The grids differ, so the files were not recorded by the same setup.
    #[error(
        "{first} has a {columns}x{rows} grid, but {other} has {other_columns}x{other_rows}: they do not belong to the same video"
    )]
    GridMismatch {
        /// The first file.
        first: String,
        /// Its columns.
        columns: u32,
        /// Its rows.
        rows: u32,
        /// The file that differs.
        other: String,
        /// Its columns.
        other_columns: u32,
        /// Its rows.
        other_rows: u32,
    },
    /// The firmwares differ.
    #[error(
        "{first} is from {first_firmware} but {other} is from {other_firmware}: they do not belong together"
    )]
    FirmwareMismatch {
        /// The first file.
        first: String,
        /// Its firmware.
        first_firmware: Firmware,
        /// The file that differs.
        other: String,
        /// Its firmware.
        other_firmware: Firmware,
    },
}

/// The joined recording and where each file went.
#[derive(Debug, Clone, PartialEq)]
pub struct Joined {
    /// The screens of all the files on one timeline.
    pub file: OsdFile,
    /// One entry for each file, in order.
    pub parts: Vec<JoinPart>,
}

/// Seconds to add to the times of each file so that the files follow one another: the first keeps its times, and
/// each following file starts `gap_s` seconds after the end of the one before it.
#[must_use]
pub fn shifts(files: &[&OsdFile], gap_s: f64) -> Vec<f64> {
    let mut shifts = Vec::with_capacity(files.len());
    let mut end = 0.0;
    for (index, file) in files.iter().enumerate() {
        let first = f64::from(file.frames[0].time_ms) / 1000.0;
        let shift = if index == 0 { 0.0 } else { end + gap_s - first };
        shifts.push(shift);
        end = file.duration_s() + shift;
    }
    shifts
}

/// Puts the files, in the order they play, on one timeline. With a single file the result is that file.
pub fn join(files: Vec<(String, OsdFile)>, gap_s: f64) -> Result<Joined, JoinError> {
    let Some((first_name, first)) = files.first() else {
        return Err(JoinError::Empty);
    };
    for (name, file) in &files[1..] {
        if (file.columns, file.rows) != (first.columns, first.rows) {
            return Err(JoinError::GridMismatch {
                first: first_name.clone(),
                columns: first.columns,
                rows: first.rows,
                other: name.clone(),
                other_columns: file.columns,
                other_rows: file.rows,
            });
        }
        if file.firmware != first.firmware {
            return Err(JoinError::FirmwareMismatch {
                first: first_name.clone(),
                first_firmware: first.firmware.clone(),
                other: name.clone(),
                other_firmware: file.firmware.clone(),
            });
        }
    }

    let offsets = shifts(&files.iter().map(|(_, f)| f).collect::<Vec<_>>(), gap_s);
    let (columns, rows, firmware) = (first.columns, first.rows, first.firmware.clone());
    let mut frames: Vec<Frame> = Vec::new();
    let mut parts = Vec::with_capacity(files.len());
    for ((name, file), shift) in files.into_iter().zip(offsets) {
        let shift_ms = (shift * 1000.0).round() as i64;
        parts.push(JoinPart {
            name,
            start_s: f64::from(file.frames[0].time_ms) / 1000.0 + shift,
            end_s: file.duration_s() + shift,
            frames: file.frames.len(),
        });
        frames.extend(file.frames.into_iter().map(|mut frame| {
            frame.time_ms =
                (i64::from(frame.time_ms) + shift_ms).clamp(0, i64::from(u32::MAX)) as u32;
            frame
        }));
    }
    Ok(Joined {
        file: OsdFile {
            firmware,
            columns,
            rows,
            frames,
            trailing_bytes: 0,
        },
        parts,
    })
}

/// How well the files of a joined recording cover the video.
#[derive(Debug, Clone, PartialEq)]
pub struct Coverage {
    /// Length of the video, in seconds.
    pub video_s: f64,
    /// Length of the joined recording, in seconds.
    pub osd_s: f64,
}

/// The least difference that is reported: the larger of this many seconds and [`TOLERANCE_PERCENT`] of the video.
const TOLERANCE_SECONDS: f64 = 2.0;
/// Percent of the video that the recording may differ from it before it is reported. Goggles and video clocks drift
/// a little, so a small difference is normal.
const TOLERANCE_PERCENT: f64 = 1.5;

impl Coverage {
    /// Compares the joined files with the video.
    #[must_use]
    pub fn new(parts: &[JoinPart], video_s: f64) -> Self {
        Self {
            video_s,
            osd_s: parts.last().map_or(0.0, |p| p.end_s),
        }
    }

    /// What is worth telling the user, if the files do not cover the video. When they fall short, a file is probably
    /// missing or out of order; when they run past, a file belongs to another video.
    #[must_use]
    pub fn warning(&self) -> Option<String> {
        let delta = self.video_s - self.osd_s;
        let allowed = TOLERANCE_SECONDS.max(self.video_s * TOLERANCE_PERCENT / 100.0);
        if delta.abs() <= allowed {
            return None;
        }
        Some(if delta > 0.0 {
            format!(
                "The .osd files add up to {} but the video is {}: {} of the video has no OSD. A file may be missing.",
                clock(self.osd_s),
                clock(self.video_s),
                clock(delta)
            )
        } else {
            format!(
                "The .osd files add up to {} but the video is only {}: they run {} past its end. A file may belong to another video.",
                clock(self.osd_s),
                clock(self.video_s),
                clock(-delta)
            )
        })
    }
}

/// `m:ss`, or `h:mm:ss`.
fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(firmware: Firmware, columns: u32, times: &[u32]) -> OsdFile {
        OsdFile {
            firmware,
            columns,
            rows: 1,
            frames: times
                .iter()
                .map(|&time_ms| Frame {
                    time_ms,
                    columns,
                    cells: vec![u16::try_from(time_ms % 100).unwrap(); columns as usize],
                })
                .collect(),
            trailing_bytes: 0,
        }
    }

    fn inav(times: &[u32]) -> (String, OsdFile) {
        ("a.osd".into(), file(Firmware::Inav, 2, times))
    }

    #[test]
    fn one_file_is_left_as_it_is() {
        let joined = join(vec![inav(&[0, 100, 200])], 0.0).unwrap();
        assert_eq!(
            joined
                .file
                .frames
                .iter()
                .map(|f| f.time_ms)
                .collect::<Vec<_>>(),
            [0, 100, 200]
        );
        assert_eq!(joined.parts.len(), 1);
        assert!((joined.parts[0].end_s - 0.3).abs() < 1e-9);
    }

    #[test]
    fn the_files_follow_one_another() {
        // Each lasts 0.3 s: three screens, 100 ms apart.
        let joined = join(
            vec![
                inav(&[0, 100, 200]),
                ("b.osd".into(), file(Firmware::Inav, 2, &[5000, 5100, 5200])),
            ],
            0.0,
        )
        .unwrap();
        let times: Vec<_> = joined.file.frames.iter().map(|f| f.time_ms).collect();
        assert_eq!(times, [0, 100, 200, 300, 400, 500]);
        assert!(
            (joined.file.duration_s() - 0.6).abs() < 1e-9,
            "the duration is the sum"
        );
        assert_eq!(joined.parts[1].name, "b.osd");
        assert!((joined.parts[1].start_s - 0.3).abs() < 1e-9);
        assert!((joined.parts[1].end_s - 0.6).abs() < 1e-9);
    }

    #[test]
    fn a_gap_is_left_between_the_files() {
        let joined = join(vec![inav(&[0, 100]), inav(&[0, 100])], 1.0).unwrap();
        let times: Vec<_> = joined.file.frames.iter().map(|f| f.time_ms).collect();
        assert_eq!(times, [0, 100, 1200, 1300]);
    }

    #[test]
    fn the_shifts_are_the_ones_the_join_applies() {
        let a = file(Firmware::Inav, 2, &[0, 100, 200]);
        let b = file(Firmware::Inav, 2, &[700, 800]);
        let shifts = shifts(&[&a, &b], 0.0);
        assert_eq!(shifts[0], 0.0);
        assert!((shifts[1] - (0.3 - 0.7)).abs() < 1e-9);
    }

    #[test]
    fn files_that_do_not_belong_together_are_refused() {
        assert_eq!(join(Vec::new(), 0.0), Err(JoinError::Empty));
        let wide = ("wide.osd".to_owned(), file(Firmware::Inav, 3, &[0, 100]));
        let error = join(vec![inav(&[0, 100]), wide], 0.0).unwrap_err();
        assert!(matches!(error, JoinError::GridMismatch { .. }), "{error}");
        let other = (
            "btfl.osd".to_owned(),
            file(Firmware::Betaflight, 2, &[0, 100]),
        );
        let error = join(vec![inav(&[0, 100]), other], 0.0).unwrap_err();
        assert!(
            matches!(error, JoinError::FirmwareMismatch { .. }),
            "{error}"
        );
        assert!(error.to_string().contains("btfl.osd"));
    }

    #[test]
    fn coverage_reports_a_missing_or_an_extra_file() {
        let part = |end_s| JoinPart {
            name: "a.osd".into(),
            start_s: 0.0,
            end_s,
            frames: 1,
        };
        assert_eq!(Coverage::new(&[part(300.0)], 300.5).warning(), None);
        let short = Coverage::new(&[part(300.0)], 600.0).warning().unwrap();
        assert!(
            short.contains("5:00") && short.contains("10:00") && short.contains("missing"),
            "{short}"
        );
        let long = Coverage::new(&[part(600.0)], 300.0).warning().unwrap();
        assert!(long.contains("another video"), "{long}");
    }
}
