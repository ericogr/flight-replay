// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Mapping between video time and telemetry time.
//!
//! The OSD recording and the video start together but run at slightly different speeds. The scale is what
//! corrects the difference. Telemetry time is `(video_time - offset) / scale`, the same formula as
//! fpv-motorsound's `internal/timesync`.
//!
//! The automatic scale is the video duration divided by the duration of the OSD recording, as
//! walksnail-osd-tool computes it. fpv-motorsound uses the frame count instead. The two agree on its sample
//! flights (within 0.02 %), but on a raw goggle recording the frame count is wrong by about 5 %, because the
//! OSD is stamped in real time there. The OSD duration is therefore the one used.

use serde::{Deserialize, Serialize};

use crate::osd::{Frame, OsdFile};

/// How the scale between video and telemetry time is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ScaleMode {
    /// From the durations of the video and of the OSD recording.
    Auto,
    /// Given by the user.
    Manual {
        /// Scale factor (1.0 means the same speed).
        scale: f64,
    },
    /// No correction: the OSD runs at the speed of the video.
    Off,
}

/// Synchronisation between the video and the telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SyncOptions {
    /// How the scale is chosen.
    pub scale: ScaleMode,
    /// Seconds of video before the telemetry starts. A positive value delays the telemetry.
    pub offset_s: f64,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            scale: ScaleMode::Auto,
            offset_s: 0.0,
        }
    }
}

/// The resolved mapping between video time and telemetry time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timeline {
    scale: f64,
    offset_s: f64,
}

impl Timeline {
    /// Resolves the options for a video of `video_duration_s` seconds and an OSD recording of `osd_duration_s`.
    ///
    /// An invalid scale (zero, negative or not finite) is replaced by 1.0.
    #[must_use]
    pub fn new(options: SyncOptions, video_duration_s: f64, osd_duration_s: f64) -> Self {
        let scale = match options.scale {
            ScaleMode::Auto if video_duration_s > 0.0 && osd_duration_s > 0.0 => {
                video_duration_s / osd_duration_s
            }
            ScaleMode::Auto | ScaleMode::Off => 1.0,
            ScaleMode::Manual { scale } => scale,
        };
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        Self {
            scale,
            offset_s: options.offset_s,
        }
    }

    /// The scale that is used: telemetry seconds per video second is `1 / scale`.
    #[must_use]
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Telemetry time, in seconds from the start of the OSD recording, shown at `video_time_s`.
    #[must_use]
    pub fn telemetry_time(&self, video_time_s: f64) -> f64 {
        (video_time_s - self.offset_s) / self.scale
    }

    /// The OSD screen that is current at `video_time_s`.
    #[must_use]
    pub fn osd_frame<'a>(&self, osd: &'a OsdFile, video_time_s: f64) -> &'a Frame {
        let telemetry_ms = self.telemetry_time(video_time_s) * 1000.0;
        &osd.frames[osd.frame_index_at(telemetry_ms)]
    }

    /// Video time at which the telemetry shows `telemetry_time_s`. The inverse of [`Timeline::telemetry_time`].
    #[must_use]
    pub fn video_time(&self, telemetry_time_s: f64) -> f64 {
        telemetry_time_s * self.scale + self.offset_s
    }

    /// How much the mapped telemetry falls short of (positive) or overruns (negative) the video: the video duration
    /// minus the video time of the last telemetry frame.
    #[must_use]
    pub fn coverage_delta(&self, video_duration_s: f64, telemetry_end_s: f64) -> f64 {
        video_duration_s - self.video_time(telemetry_end_s)
    }

    /// Checks that the mapped telemetry spans the video, and that the scale is plausible. Returns what is worth
    /// telling the user. `telemetry_end_s` is the time of the last OSD frame.
    #[must_use]
    pub fn check(
        &self,
        video_duration_s: f64,
        telemetry_end_s: f64,
        tolerance: SyncTolerance,
    ) -> Vec<SyncWarning> {
        let mut warnings = Vec::new();
        if !(0.8..=1.25).contains(&self.scale) {
            warnings.push(SyncWarning::ImplausibleScale { scale: self.scale });
        }
        let delta = self.coverage_delta(video_duration_s, telemetry_end_s);
        let allowed = tolerance
            .min_seconds
            .max(video_duration_s * tolerance.percent / 100.0);
        if delta.abs() > allowed && telemetry_end_s > 0.0 {
            let mapped_end = self.video_time(telemetry_end_s);
            let suggested_scale = video_duration_s / telemetry_end_s;
            warnings.push(if delta > 0.0 {
                SyncWarning::TelemetryEndsEarly {
                    seconds: delta,
                    mapped_end,
                    video_duration: video_duration_s,
                    suggested_scale,
                }
            } else {
                SyncWarning::TelemetryRunsPast {
                    seconds: -delta,
                    mapped_end,
                    video_duration: video_duration_s,
                    suggested_scale,
                }
            });
        }
        warnings
    }
}

/// How far the mapped telemetry may differ from the end of the video before [`Timeline::check`] warns: the larger
/// of a percentage of the video and a number of seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyncTolerance {
    /// Percent of the video duration.
    pub percent: f64,
    /// Seconds.
    pub min_seconds: f64,
}

impl Default for SyncTolerance {
    fn default() -> Self {
        Self {
            percent: 0.1,
            min_seconds: 2.0,
        }
    }
}

/// Something the user should know about the synchronisation.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncWarning {
    /// The scale is far from 1, so the video and the OSD probably do not belong together.
    ImplausibleScale {
        /// The scale.
        scale: f64,
    },
    /// The mapped telemetry ends before the video does.
    TelemetryEndsEarly {
        /// How many seconds before.
        seconds: f64,
        /// Video time of the last telemetry frame.
        mapped_end: f64,
        /// Length of the video.
        video_duration: f64,
        /// The scale that would make the telemetry cover the whole video.
        suggested_scale: f64,
    },
    /// The mapped telemetry runs past the end of the video.
    TelemetryRunsPast {
        /// How many seconds past.
        seconds: f64,
        /// Video time of the last telemetry frame.
        mapped_end: f64,
        /// Length of the video.
        video_duration: f64,
        /// The scale that would make the telemetry cover the whole video.
        suggested_scale: f64,
    },
}

impl std::fmt::Display for SyncWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ImplausibleScale { scale } => write!(
                f,
                "the time scale {scale:.4} is unlikely (expected close to 1.00): check that the video and the .osd are from the same flight"
            ),
            Self::TelemetryEndsEarly {
                seconds,
                mapped_end,
                video_duration,
                suggested_scale,
            } => write!(
                f,
                "the mapped telemetry ends {seconds:.1} s before the end of the video (it ends at {mapped_end:.1} s, the video is {video_duration:.1} s long). If it covers the whole video, the scale would be {suggested_scale:.6}"
            ),
            Self::TelemetryRunsPast {
                seconds,
                mapped_end,
                video_duration,
                suggested_scale,
            } => write!(
                f,
                "the mapped telemetry runs {seconds:.1} s past the end of the video (it ends at {mapped_end:.1} s, the video is {video_duration:.1} s long). If it covers the whole video, the scale would be {suggested_scale:.6}"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_scale_is_the_ratio_of_the_durations() {
        // Values of the raw recording AvatarG0153: video 846.1 s, OSD 845.9 s.
        let timeline = Timeline::new(SyncOptions::default(), 846.1, 845.893);
        assert!((timeline.scale() - 846.1 / 845.893).abs() < 1e-12);
        assert!((timeline.telemetry_time(846.1) - 845.893).abs() < 1e-9);
    }

    #[test]
    fn offset_delays_the_telemetry() {
        let options = SyncOptions {
            scale: ScaleMode::Off,
            offset_s: 2.5,
        };
        let timeline = Timeline::new(options, 100.0, 90.0);
        assert!((timeline.telemetry_time(2.5)).abs() < 1e-12);
        assert!((timeline.telemetry_time(12.5) - 10.0).abs() < 1e-12);
        assert!((timeline.video_time(10.0) - 12.5).abs() < 1e-12);
    }

    #[test]
    fn the_osd_screen_is_the_one_current_at_the_video_time() {
        use crate::osd::Firmware;
        let screen = |time_ms| Frame {
            time_ms,
            columns: 1,
            cells: vec![0],
        };
        let osd = OsdFile {
            firmware: Firmware::Inav,
            columns: 1,
            rows: 1,
            frames: vec![screen(0), screen(100), screen(200)],
            trailing_bytes: 0,
        };
        let options = SyncOptions {
            scale: ScaleMode::Off,
            offset_s: 0.5,
        };
        let timeline = Timeline::new(options, 10.0, 0.3);
        assert_eq!(
            timeline.osd_frame(&osd, 0.0).time_ms,
            0,
            "before the telemetry starts"
        );
        assert_eq!(timeline.osd_frame(&osd, 0.65).time_ms, 100);
        assert_eq!(
            timeline.osd_frame(&osd, 99.0).time_ms,
            200,
            "after the last screen"
        );
    }

    #[test]
    fn manual_and_invalid_scales() {
        let manual = SyncOptions {
            scale: ScaleMode::Manual { scale: 1.25 },
            offset_s: 0.0,
        };
        assert!((Timeline::new(manual, 10.0, 10.0).scale() - 1.25).abs() < 1e-12);
        let broken = SyncOptions {
            scale: ScaleMode::Manual { scale: 0.0 },
            offset_s: 0.0,
        };
        assert!((Timeline::new(broken, 10.0, 10.0).scale() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn warns_when_the_telemetry_does_not_span_the_video() {
        // Go's frame-count model on a raw recording: scale 1.051186 puts the telemetry 43 s past the video.
        let wrong = Timeline::new(
            SyncOptions {
                scale: ScaleMode::Manual { scale: 1.051_186 },
                offset_s: 0.0,
            },
            846.1,
            845.893,
        );
        let warnings = wrong.check(846.1, 845.893, SyncTolerance::default());
        assert!(matches!(
            warnings.as_slice(),
            [SyncWarning::TelemetryRunsPast { seconds, .. }] if (*seconds - 43.1).abs() < 0.1
        ));
        // The OSD-duration scale leaves only about one OSD frame (0.157 s) uncovered, so there is no warning.
        let right = Timeline::new(SyncOptions::default(), 846.1, 846.05);
        assert_eq!(
            right.check(846.1, 845.893, SyncTolerance::default()),
            vec![]
        );
    }

    #[test]
    fn warns_about_an_implausible_scale() {
        let odd = Timeline::new(
            SyncOptions {
                scale: ScaleMode::Manual { scale: 2.0 },
                offset_s: 0.0,
            },
            100.0,
            50.0,
        );
        assert!(
            odd.check(100.0, 50.0, SyncTolerance::default())
                .iter()
                .any(|w| matches!(w, SyncWarning::ImplausibleScale { .. }))
        );
    }

    #[test]
    fn unknown_durations_fall_back_to_no_correction() {
        let timeline = Timeline::new(SyncOptions::default(), 0.0, 90.0);
        assert!((timeline.scale() - 1.0).abs() < 1e-12);
    }
}
