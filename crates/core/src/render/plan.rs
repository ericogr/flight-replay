// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/ffmpeg/render_settings.rs (GPL-3.0-only), Copyright (C) Alexander
// van Saase, and ported from fpv-motorsound, internal/pipeline/render.go (Apache-2.0, Copyright 2026 EricoGR).

use std::path::{Path, PathBuf};

use crate::ffmpeg::VideoInfo;
use crate::font::FontSheet;
use crate::osd::{OsdFile, OsdOptions};
use crate::srt::{SrtFile, SrtOptions};
use crate::synth::SoundSettings;
use crate::telemetry::{Profile, TelemetrySupport};
use crate::timeline::SyncOptions;

/// The OSD, drawn with a font.
pub struct OsdLayer {
    /// The font sheet of the goggles.
    pub font: FontSheet,
    /// Position and masks.
    pub options: OsdOptions,
}

/// The SRT data, drawn as text.
pub struct SrtLayer {
    /// The subtitles.
    pub file: SrtFile,
    /// Which fields are drawn, and where.
    pub options: SrtOptions,
}

/// A stretch of the video, in video time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    /// Where it starts, in seconds.
    pub start_s: f64,
    /// How long it is, in seconds. `None` runs to the end of the video.
    pub duration_s: Option<f64>,
}

/// What to do when the output file exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overwrite {
    /// Stop with an error.
    #[default]
    Refuse,
    /// Replace the file.
    Replace,
}

/// How the picture is encoded.
#[derive(Debug, Clone, PartialEq)]
pub struct EncoderChoice {
    /// FFmpeg encoder name, such as `libx264`.
    pub name: String,
    /// Options FFmpeg needs for the encoder, besides the bitrate.
    pub extra_args: Vec<String>,
    /// Target bitrate, in megabits per second.
    pub bitrate_mbps: u32,
    /// Scale the picture to 1440p.
    pub upscale_1440p: bool,
}

/// What happens to the video stream.
#[derive(Debug, Clone, PartialEq)]
pub enum VideoOutput {
    /// The stream is copied, bit for bit. Only possible when nothing is drawn on the picture.
    Copy,
    /// The picture is drawn and encoded again.
    Encode(EncoderChoice),
}

/// The output file and how it is made.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputPlan {
    /// Where the video goes.
    pub path: PathBuf,
    /// What happens to the video stream.
    pub video: VideoOutput,
    /// AAC bitrate, in kilobits per second.
    pub audio_bitrate_kbps: u32,
    /// Moves the MP4 index to the start of the file.
    pub faststart: bool,
    /// What to do when the file exists.
    pub overwrite: Overwrite,
    /// Allows the sound of a video that already has some to be replaced.
    pub replace_audio: bool,
}

impl OutputPlan {
    /// The default output: next to the video, with `_flight-replay` added to its name (and `_preview` for a stretch).
    #[must_use]
    pub fn default_path(video: &Path, preview: bool) -> PathBuf {
        let stem = video
            .file_stem()
            .map_or_else(|| "video".into(), |s| s.to_string_lossy().into_owned());
        let name = if preview {
            format!("{stem}_flight-replay_preview.mp4")
        } else {
            format!("{stem}_flight-replay.mp4")
        };
        video.parent().unwrap_or_else(|| Path::new("")).join(name)
    }
}

/// Everything a render needs.
pub struct RenderPlan {
    /// The source video.
    pub video: PathBuf,
    /// What is known about it.
    pub info: VideoInfo,
    /// The OSD recording. Needed to draw the OSD and to make the sound.
    pub osd: Option<OsdFile>,
    /// The path of the `.osd` file, for messages.
    pub osd_path: Option<PathBuf>,
    /// Draw the OSD.
    pub osd_layer: Option<OsdLayer>,
    /// Draw the SRT data.
    pub srt_layer: Option<SrtLayer>,
    /// Draw on a background of this colour instead of the video (to composite in an editor).
    pub chroma_key: Option<[u8; 3]>,
    /// Make the sound.
    pub sound: Option<SoundSettings>,
    /// Synchronisation of the video and the OSD.
    pub sync: SyncOptions,
    /// Render only a stretch of the video.
    pub window: Option<Window>,
    /// The output.
    pub output: OutputPlan,
}

/// How a plan is carried out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The video stream is copied and the sound is added.
    SoundOnly,
    /// The picture is drawn and encoded again, with or without sound.
    Overlay,
}

/// The stretch of the video that is rendered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedWindow {
    /// Where it starts, in seconds.
    pub start_s: f64,
    /// How long it is, in seconds.
    pub duration_s: f64,
    /// Whether it is less than the whole video.
    pub partial: bool,
}

impl RenderPlan {
    /// Whether anything is drawn on the picture.
    #[must_use]
    pub fn draws(&self) -> bool {
        self.osd_layer.is_some() || self.srt_layer.is_some()
    }

    /// How the plan is carried out.
    #[must_use]
    pub fn mode(&self) -> Mode {
        if self.draws() {
            Mode::Overlay
        } else {
            Mode::SoundOnly
        }
    }

    /// The stretch of the video that is rendered.
    #[must_use]
    pub fn resolved_window(&self) -> ResolvedWindow {
        let total = self.info.duration_s;
        let Some(window) = self.window else {
            return ResolvedWindow {
                start_s: 0.0,
                duration_s: total,
                partial: false,
            };
        };
        let start_s = window.start_s.clamp(0.0, total);
        let left = total - start_s;
        let duration_s = window.duration_s.map_or(left, |d| d.min(left));
        ResolvedWindow {
            start_s,
            duration_s,
            partial: start_s > 0.0 || duration_s < total - 1e-9,
        }
    }

    /// Checks that the plan can run, and returns every problem at once.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut problems = Vec::new();
        if !self.draws() && self.sound.is_none() {
            problems.push(
                "choose at least one thing to add: the OSD, the SRT data or the sound".to_owned(),
            );
        }
        if self.osd_layer.is_some() && self.osd.is_none() {
            problems.push("the OSD cannot be drawn without an .osd file".into());
        }
        if self.chroma_key.is_some() && !self.draws() {
            problems
                .push("a chroma-key background needs the OSD or the SRT data to draw on it".into());
        }
        if let Some(settings) = &self.sound {
            match &self.osd {
                None => problems.push(
                    "the sound needs an .osd file: it follows the telemetry stored in it".into(),
                ),
                Some(osd) => {
                    let profile = Profile::from_firmware(&osd.firmware);
                    if profile.telemetry_support() != TelemetrySupport::Available {
                        problems.push(profile.explanation().to_owned());
                    }
                }
            }
            if let Err(sound_problems) = settings.validate() {
                problems.extend(sound_problems);
            }
        }
        match (&self.output.video, self.draws()) {
            (VideoOutput::Copy, true) => {
                problems.push("drawing on the picture needs the video to be encoded again".into());
            }
            (VideoOutput::Encode(_), false) => {
                problems.push(
                    "without anything drawn on the picture, the video stream is copied".into(),
                );
            }
            (VideoOutput::Encode(choice), true) if choice.name.is_empty() => {
                problems.push("choose a video encoder".into());
            }
            _ => {}
        }
        if self.output.path == self.video {
            problems.push("the output file is the source video".into());
        }
        if let Some(window) = self.window {
            if window.start_s < 0.0 || window.start_s >= self.info.duration_s {
                problems.push(format!(
                    "the start of the stretch ({:.1} s) is outside the video ({:.1} s)",
                    window.start_s, self.info.duration_s
                ));
            }
            if window.duration_s.is_some_and(|d| d <= 0.0) {
                problems.push("the length of the stretch must be more than zero".into());
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> VideoInfo {
        VideoInfo {
            codec: "h264".into(),
            width: 1280,
            height: 720,
            frame_rate: 60.0,
            nominal_frame_rate: 60.0,
            duration_s: 100.0,
            frame_count: 6000,
            audio_streams: 0,
            audio_duration_s: None,
        }
    }

    fn plan() -> RenderPlan {
        RenderPlan {
            video: PathBuf::from("in.mp4"),
            info: info(),
            osd: None,
            osd_path: None,
            osd_layer: None,
            srt_layer: None,
            chroma_key: None,
            sound: None,
            sync: SyncOptions::default(),
            window: None,
            output: OutputPlan {
                path: PathBuf::from("out.mp4"),
                video: VideoOutput::Copy,
                audio_bitrate_kbps: 192,
                faststart: true,
                overwrite: Overwrite::Refuse,
                replace_audio: false,
            },
        }
    }

    #[test]
    fn a_plan_with_nothing_to_add_is_refused() {
        let problems = plan().validate().unwrap_err();
        assert!(problems[0].contains("at least one"), "{problems:?}");
    }

    #[test]
    fn the_sound_needs_an_osd_file() {
        let mut p = plan();
        p.sound = Some(SoundSettings::default());
        let problems = p.validate().unwrap_err();
        assert!(
            problems.iter().any(|m| m.contains("needs an .osd")),
            "{problems:?}"
        );
    }

    #[test]
    fn the_default_output_is_next_to_the_video() {
        assert_eq!(
            OutputPlan::default_path(Path::new("/flights/AvatarG0153.mp4"), false),
            PathBuf::from("/flights/AvatarG0153_flight-replay.mp4")
        );
        assert_eq!(
            OutputPlan::default_path(Path::new("a.mp4"), true),
            PathBuf::from("a_flight-replay_preview.mp4")
        );
    }

    #[test]
    fn the_window_is_clipped_to_the_video() {
        let mut p = plan();
        assert_eq!(p.resolved_window().duration_s, 100.0);
        assert!(!p.resolved_window().partial);
        p.window = Some(Window {
            start_s: 90.0,
            duration_s: Some(30.0),
        });
        let w = p.resolved_window();
        assert_eq!((w.start_s, w.duration_s, w.partial), (90.0, 10.0, true));
        p.window = Some(Window {
            start_s: 120.0,
            duration_s: None,
        });
        assert!(
            p.validate()
                .unwrap_err()
                .iter()
                .any(|m| m.contains("outside the video"))
        );
    }

    #[test]
    fn the_output_cannot_overwrite_the_source() {
        let mut p = plan();
        p.sound = Some(SoundSettings::default());
        p.output.path = p.video.clone();
        assert!(
            p.validate()
                .unwrap_err()
                .iter()
                .any(|m| m.contains("is the source video"))
        );
    }
}
