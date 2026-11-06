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

    /// Video time at which the telemetry shows `telemetry_time_s`. The inverse of [`Timeline::telemetry_time`].
    #[must_use]
    pub fn video_time(&self, telemetry_time_s: f64) -> f64 {
        telemetry_time_s * self.scale + self.offset_s
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
    fn unknown_durations_fall_back_to_no_correction() {
        let timeline = Timeline::new(SyncOptions::default(), 0.0, 90.0);
        assert!((timeline.scale() - 1.0).abs() < 1e-12);
    }
}
