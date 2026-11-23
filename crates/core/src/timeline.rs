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
//!
//! A flight can be made of several videos that follow one another, each with the recording of its own. Each video
//! and its recording are a [`Leg`], and every leg has the scale of its own, because the clock of each recording
//! drifts on its own. The legs are put one after the other on the joined timeline of the video and on the joined
//! timeline of the recordings (see `crate::osd::join`), and a time is mapped inside the leg that holds it.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::osd::{Frame, OsdFile};

/// How close to the end of a leg a time is held, in seconds. The first screen of the next recording starts where
/// this one ends, so a time at the very end would show it.
const LEG_GUARD_S: f64 = 0.001;

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

/// One video and the OSD recording that goes with it, on the joined timelines of the video and of the recordings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leg {
    /// Where the video starts on the timeline of the whole video, in seconds.
    pub video_start_s: f64,
    /// How long the video is, in seconds.
    pub video_duration_s: f64,
    /// Where the recording starts on the timeline of the joined recordings, in seconds.
    pub osd_start_s: f64,
    /// How long the recording is, in seconds.
    pub osd_duration_s: f64,
}

/// A leg with its scale worked out.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stretch {
    video_start_s: f64,
    osd_start_s: f64,
    osd_end_s: f64,
    scale: f64,
}

/// The resolved mapping between video time and telemetry time.
#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    /// At least one, in the order they play.
    stretches: Arc<[Stretch]>,
    offset_s: f64,
}

impl Timeline {
    /// Resolves the options for a video of `video_duration_s` seconds and an OSD recording of `osd_duration_s`.
    ///
    /// An invalid scale (zero, negative or not finite) is replaced by 1.0.
    #[must_use]
    pub fn new(options: SyncOptions, video_duration_s: f64, osd_duration_s: f64) -> Self {
        Self::joined(
            options,
            &[Leg {
                video_start_s: 0.0,
                video_duration_s,
                osd_start_s: 0.0,
                osd_duration_s,
            }],
        )
    }

    /// Resolves the options for videos that follow one another, each with its own recording. The scale of each leg
    /// is worked out from the durations of that leg, and the delay applies inside each leg. `legs` must not be
    /// empty.
    #[must_use]
    pub fn joined(options: SyncOptions, legs: &[Leg]) -> Self {
        let stretches = legs
            .iter()
            .map(|leg| Stretch {
                video_start_s: leg.video_start_s,
                osd_start_s: leg.osd_start_s,
                osd_end_s: leg.osd_start_s + leg.osd_duration_s,
                scale: resolve_scale(options.scale, leg.video_duration_s, leg.osd_duration_s),
            })
            .collect();
        Self {
            stretches,
            offset_s: options.offset_s,
        }
    }

    /// The scale that is used: telemetry seconds per video second is `1 / scale`. With several legs, the scale of
    /// the first one: see [`Timeline::scales`].
    #[must_use]
    pub fn scale(&self) -> f64 {
        self.stretches[0].scale
    }

    /// The scale of each leg.
    pub fn scales(&self) -> impl Iterator<Item = f64> + '_ {
        self.stretches.iter().map(|stretch| stretch.scale)
    }

    /// The leg that holds `video_time_s`: the last one that has started. Before the first, the first.
    fn stretch_at(&self, video_time_s: f64) -> usize {
        self.stretches
            .partition_point(|stretch| stretch.video_start_s <= video_time_s)
            .saturating_sub(1)
    }

    /// Telemetry time, in seconds from the start of the OSD recording, shown at `video_time_s`.
    ///
    /// Inside a joined recording a leg does not run into its neighbours: a time before its recording starts shows
    /// its first screen, and one after it ends shows its last. Before the first leg and after the last there is
    /// nothing to run into, so the mapping goes on.
    #[must_use]
    pub fn telemetry_time(&self, video_time_s: f64) -> f64 {
        let index = self.stretch_at(video_time_s);
        let stretch = &self.stretches[index];
        let time = stretch.osd_start_s
            + (video_time_s - stretch.video_start_s - self.offset_s) / stretch.scale;
        let lowest = if index > 0 {
            stretch.osd_start_s
        } else {
            f64::NEG_INFINITY
        };
        let highest = if index + 1 < self.stretches.len() {
            (stretch.osd_end_s - LEG_GUARD_S).max(stretch.osd_start_s)
        } else {
            f64::INFINITY
        };
        time.clamp(lowest, highest)
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
        let index = self
            .stretches
            .partition_point(|stretch| stretch.osd_start_s <= telemetry_time_s)
            .saturating_sub(1);
        let stretch = &self.stretches[index];
        stretch.video_start_s
            + (telemetry_time_s - stretch.osd_start_s) * stretch.scale
            + self.offset_s
    }

    /// How much the mapped telemetry falls short of (positive) or overruns (negative) the video: the video duration
    /// minus the video time of the last telemetry frame. For a single video: the legs of a joined one are each
    /// checked on their own.
    #[must_use]
    pub fn coverage_delta(&self, video_duration_s: f64, telemetry_end_s: f64) -> f64 {
        video_duration_s - self.video_time(telemetry_end_s)
    }

    /// Checks that the mapped telemetry spans the video, and that the scale is plausible. Returns what is worth
    /// telling the user. `telemetry_end_s` is the time of the last OSD frame. For a single video: the legs of a
    /// joined one are each checked on their own, with a [`Timeline::new`] of their durations.
    #[must_use]
    pub fn check(
        &self,
        video_duration_s: f64,
        telemetry_end_s: f64,
        tolerance: SyncTolerance,
    ) -> Vec<SyncWarning> {
        let scale = self.scale();
        let mut warnings = Vec::new();
        if !(0.8..=1.25).contains(&scale) {
            warnings.push(SyncWarning::ImplausibleScale { scale });
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

/// The scale of a leg: the options say how it is chosen. An invalid one is replaced by 1.0.
fn resolve_scale(mode: ScaleMode, video_duration_s: f64, osd_duration_s: f64) -> f64 {
    let scale = match mode {
        ScaleMode::Auto if video_duration_s > 0.0 && osd_duration_s > 0.0 => {
            video_duration_s / osd_duration_s
        }
        ScaleMode::Auto | ScaleMode::Off => 1.0,
        ScaleMode::Manual { scale } => scale,
    };
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
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

    /// Two videos of 10 s whose recordings last 9.5 s and 10.5 s, joined one after the other (19.5 s and 20 s).
    fn two_legs(options: SyncOptions) -> Timeline {
        Timeline::joined(
            options,
            &[
                Leg {
                    video_start_s: 0.0,
                    video_duration_s: 10.0,
                    osd_start_s: 0.0,
                    osd_duration_s: 9.5,
                },
                Leg {
                    video_start_s: 10.0,
                    video_duration_s: 10.0,
                    osd_start_s: 9.5,
                    osd_duration_s: 10.5,
                },
            ],
        )
    }

    #[test]
    fn each_leg_has_the_scale_of_its_own_recording() {
        let timeline = two_legs(SyncOptions::default());
        let scales: Vec<f64> = timeline.scales().collect();
        assert!((scales[0] - 10.0 / 9.5).abs() < 1e-12);
        assert!((scales[1] - 10.0 / 10.5).abs() < 1e-12);
        // The end of each video is the end of its recording, whatever the speed of the other one. The end of the
        // first is held a millisecond short, so that it does not show the first screen of the second recording.
        assert!((timeline.telemetry_time(10.0 - 1e-9) - (9.5 - LEG_GUARD_S)).abs() < 1e-6);
        assert!((timeline.telemetry_time(10.0) - 9.5).abs() < 1e-9);
        assert!((timeline.telemetry_time(20.0) - 20.0).abs() < 1e-9);
        assert!((timeline.video_time(9.5) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_leg_never_shows_the_screens_of_its_neighbour() {
        use crate::osd::Firmware;
        let screen = |time_ms| Frame {
            time_ms,
            columns: 1,
            cells: vec![0],
        };
        // The recordings of the two legs, joined: 0 to 9.5 s, and 9.5 to 20 s.
        let osd = OsdFile {
            firmware: Firmware::Inav,
            columns: 1,
            rows: 1,
            frames: vec![screen(0), screen(9000), screen(9500), screen(10_000)],
            trailing_bytes: 0,
        };
        let delayed = |offset_s| {
            two_legs(SyncOptions {
                scale: ScaleMode::Off,
                offset_s,
            })
        };
        // The end of the first video shows its last screen, and not the first one of the second recording.
        assert_eq!(delayed(0.0).osd_frame(&osd, 9.99).time_ms, 9000);
        // A delay of 2 s: the first 2 s of the second video show the first screen of its recording, not the end of
        // the first one's.
        assert_eq!(delayed(2.0).osd_frame(&osd, 10.5).time_ms, 9500);
        // The first and the last leg are not held in: the mapping goes on past them.
        assert_eq!(delayed(2.0).osd_frame(&osd, 0.0).time_ms, 0);
        assert_eq!(delayed(2.0).osd_frame(&osd, 99.0).time_ms, 10_000);
    }

    #[test]
    fn one_leg_is_the_timeline_of_one_video() {
        let options = SyncOptions {
            scale: ScaleMode::Manual { scale: 1.02 },
            offset_s: 1.5,
        };
        let single = Timeline::new(options, 30.0, 29.0);
        let joined = Timeline::joined(
            options,
            &[Leg {
                video_start_s: 0.0,
                video_duration_s: 30.0,
                osd_start_s: 0.0,
                osd_duration_s: 29.0,
            }],
        );
        assert_eq!(single, joined);
        for time in [-5.0, 0.0, 1.5, 12.3, 30.0, 99.0] {
            assert_eq!(single.telemetry_time(time), (time - 1.5) / 1.02);
        }
    }

    #[test]
    fn unknown_durations_fall_back_to_no_correction() {
        let timeline = Timeline::new(SyncOptions::default(), 0.0, 90.0);
        assert!((timeline.scale() - 1.0).abs() < 1e-12);
    }
}
