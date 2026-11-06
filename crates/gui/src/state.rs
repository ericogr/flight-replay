// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! What the application holds in memory: the files that are open and what was worked out from them.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use flight_replay_core::ffmpeg::VideoInfo;
use flight_replay_core::font::{BundledFont, OsdFont};
use flight_replay_core::osd::{JoinPart, OsdFile};
use flight_replay_core::paths::file_name;
use flight_replay_core::render::{CancelToken, Outcome, RenderPlan, Stage, Window};
use flight_replay_core::srt::SrtFile;
use flight_replay_core::synth::SignalSet;
use flight_replay_core::telemetry::{Profile, Recording};
use flight_replay_core::timeline::{ScaleMode, SyncOptions};
use iced::widget::image::Handle;

use crate::wizard::PreviewLayers;

/// A video that is open.
#[derive(Debug)]
pub struct LoadedVideo {
    pub path: PathBuf,
    pub info: VideoInfo,
}

impl LoadedVideo {
    /// The time of the last frame, in seconds. A picture cannot be made at the very end of the video, where no frame
    /// starts, so this is the last time the preview can show.
    pub fn last_frame_s(&self) -> f64 {
        (self.info.duration_s - 1.0 / self.info.nominal_frame_rate).max(0.0)
    }
}

/// An `.osd` file that is open.
#[derive(Debug)]
pub struct LoadedOsd {
    /// Every file, in the order they play. More than one when the video is a join of several recordings. There is
    /// always at least one.
    pub paths: Vec<PathBuf>,
    /// The screens of all the files, on one timeline.
    pub file: Arc<OsdFile>,
    /// Where each file sits on that timeline.
    pub parts: Vec<JoinPart>,
    pub profile: Profile,
}

impl LoadedOsd {
    /// The first file, which names the recording.
    pub fn path(&self) -> &Path {
        &self.paths[0]
    }
}

/// An `.srt` file that is open.
#[derive(Debug)]
pub struct LoadedSrt {
    pub path: PathBuf,
    pub file: Arc<SrtFile>,
}

/// Where an OSD font comes from.
#[derive(Debug, Clone)]
pub enum FontSource {
    /// A file the user chose.
    File(PathBuf),
    /// A font that comes with the program.
    Bundled(&'static BundledFont),
}

/// An OSD font that is open.
#[derive(Debug)]
pub struct LoadedFont {
    pub source: FontSource,
    /// The sheet of a file, or the sheets of a font that comes with the program.
    pub font: OsdFont,
}

/// The kinds of file the user picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Video,
    Osd,
    Srt,
    Font,
}

impl FileKind {
    /// What the file is called to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Osd => ".osd file",
            Self::Srt => ".srt file",
            Self::Font => "OSD font file",
        }
    }

    /// The extensions of the file dialog.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Video => &["mp4", "MP4", "mov", "MOV", "mkv"],
            Self::Osd => &["osd", "OSD"],
            Self::Srt => &["srt", "SRT"],
            Self::Font => &["png", "PNG"],
        }
    }
}

/// The preview picture of steps 3, 4 and 5.
#[derive(Default)]
pub struct Preview {
    /// The video time that is shown, in seconds.
    pub time_s: f64,
    /// The picture shown.
    pub handle: Option<Handle>,
    /// The size of the picture shown, in pixels.
    pub shown: (u32, u32),
    /// The decoded frame without anything drawn on it, and the time it is from.
    pub base: Option<(f64, Arc<image::RgbaImage>)>,
    /// The number of the last request. Older answers are ignored.
    pub sequence: u64,
    /// A picture is being made.
    pub busy: bool,
    /// Why the preview could not be made.
    pub error: Option<String>,
    /// Clicking the picture hides or shows the OSD cell under the pointer.
    pub mask_mode: bool,
    /// The pointer, relative to the picture shown.
    pub hover: Option<iced::Point>,
    /// What was drawn on the last picture that was asked for.
    pub layers: PreviewLayers,
}

impl Preview {
    /// Shows a picture: `pixels` are its RGBA bytes.
    pub fn set_picture(&mut self, width: u32, height: u32, pixels: Vec<u8>) {
        self.shown = (width, height);
        self.handle = Some(Handle::from_rgba(width, height, pixels));
    }
}

/// The state of a render.
pub enum RenderState {
    /// Nothing has been started.
    Idle,
    /// A render is running.
    Running(Running),
    /// The render ended.
    Finished(Result<Box<Outcome>, String>),
}

/// A render that is running.
pub struct Running {
    pub cancel: CancelToken,
    pub stage: Stage,
    pub audio: (u64, u64),
    pub video: (u64, u64),
    pub started: Instant,
    pub warnings: Vec<String>,
    pub cancelling: bool,
}

impl Running {
    pub fn new(cancel: CancelToken) -> Self {
        Self {
            cancel,
            stage: Stage::Preparing,
            audio: (0, 0),
            video: (0, 0),
            started: Instant::now(),
            warnings: Vec::new(),
            cancelling: false,
        }
    }
}

/// What the telemetry gives the sound.
pub struct Telemetry {
    pub recording: Arc<Recording>,
    /// The signals the sound can use, or why it cannot be made.
    pub signals: Result<Arc<SignalSet>, String>,
}

/// What happened to one video of the queue.
#[derive(Debug, Clone, PartialEq)]
pub enum QueueStatus {
    /// Not started.
    Waiting,
    /// Being rendered.
    Running,
    /// Rendered; what to tell the user.
    Done(String),
    /// Did not render; why.
    Failed(String),
    /// Stopped by the user.
    Cancelled,
}

/// One video in the queue, with everything its render needs.
#[derive(Clone)]
pub struct QueueItem {
    /// What the video is called.
    pub title: String,
    /// The plan, made with the settings of the wizard when the video was added.
    pub plan: Arc<RenderPlan>,
    /// Where it stands.
    pub status: QueueStatus,
}

impl std::fmt::Debug for QueueItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueueItem")
            .field("title", &self.title)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// The videos of a folder that were made ready for the queue, and why the others were left out.
#[derive(Debug)]
pub struct FolderQueue {
    pub items: Vec<QueueItem>,
    /// One line for each video that is in the queue without some of what the settings ask for, and why.
    pub left_out: Vec<String>,
    /// One line for each video that was left out.
    pub skipped: Vec<String>,
}

impl QueueItem {
    /// An item for a plan that was just made.
    pub fn new(plan: RenderPlan) -> Self {
        let title = file_name(&plan.video);
        Self {
            title,
            plan: Arc::new(plan),
            status: QueueStatus::Waiting,
        }
    }
}

/// The videos waiting to be rendered one after the other with the same settings.
#[derive(Default)]
pub struct Queue {
    pub items: Vec<QueueItem>,
    /// The queue is being worked through.
    pub running: bool,
    /// The item that is being rendered.
    pub current: Option<usize>,
    /// The videos of a folder are being prepared.
    pub preparing: bool,
}

impl Queue {
    /// How many items have not started.
    pub fn waiting(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.status == QueueStatus::Waiting)
            .count()
    }
}

/// A number the user typed: spaces around it are ignored, and a comma is taken as the decimal point.
pub fn parse_number(text: &str) -> Option<f64> {
    text.trim().replace(',', ".").parse().ok()
}

/// How the scale between the video and the OSD is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleChoice {
    Auto,
    Off,
    Manual,
}

impl ScaleChoice {
    pub const ALL: [ScaleChoice; 3] = [Self::Auto, Self::Off, Self::Manual];
}

impl std::fmt::Display for ScaleChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Auto => "Automatic (from the durations)",
            Self::Off => "None (same speed)",
            Self::Manual => "Manual",
        })
    }
}

/// The timing of the OSD as the user typed it. What it means is worked out when it is needed, so there is one copy
/// of the choice and not a text and a number that could disagree.
pub struct SyncInput {
    pub choice: ScaleChoice,
    pub manual_scale: String,
    /// Seconds of video before the telemetry starts.
    pub offset: String,
}

impl Default for SyncInput {
    fn default() -> Self {
        Self {
            choice: ScaleChoice::Auto,
            manual_scale: "1.000".to_owned(),
            offset: "0".to_owned(),
        }
    }
}

impl SyncInput {
    /// The options the text stands for. A number that cannot be read counts as no correction.
    pub fn options(&self) -> SyncOptions {
        let scale = match self.choice {
            ScaleChoice::Auto => ScaleMode::Auto,
            ScaleChoice::Off => ScaleMode::Off,
            ScaleChoice::Manual => ScaleMode::Manual {
                scale: parse_number(&self.manual_scale).unwrap_or(1.0),
            },
        };
        SyncOptions {
            scale,
            offset_s: parse_number(&self.offset).unwrap_or(0.0),
        }
    }
}

/// The stretch of the video to render, as the user typed it.
pub struct StretchInput {
    pub enabled: bool,
    pub start: String,
    pub duration: String,
}

impl Default for StretchInput {
    fn default() -> Self {
        Self {
            enabled: false,
            start: "0".to_owned(),
            duration: "30".to_owned(),
        }
    }
}

impl StretchInput {
    /// The stretch, if one is asked for. A start that cannot be read is the beginning, and a length that cannot be
    /// read is the rest of the video.
    pub fn window(&self) -> Option<Window> {
        self.enabled.then(|| Window {
            start_s: parse_number(&self.start).unwrap_or(0.0),
            duration_s: parse_number(&self.duration),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_time_that_can_be_shown_is_the_last_frame() {
        let video = |duration_s, nominal_frame_rate| LoadedVideo {
            path: PathBuf::from("a.mp4"),
            info: VideoInfo {
                codec: "h264".into(),
                width: 1280,
                height: 720,
                frame_rate: nominal_frame_rate,
                nominal_frame_rate,
                duration_s,
                frame_count: 0,
                audio_streams: 0,
                audio_duration_s: None,
            },
        };
        assert!((video(3.0, 60.0).last_frame_s() - (3.0 - 1.0 / 60.0)).abs() < 1e-12);
        assert_eq!(
            video(0.0, 60.0).last_frame_s(),
            0.0,
            "a video with no length"
        );
    }

    #[test]
    fn a_typed_number_may_have_spaces_and_a_decimal_comma() {
        assert_eq!(parse_number(" 1,25 "), Some(1.25));
        assert_eq!(parse_number("30"), Some(30.0));
        assert_eq!(parse_number("-0.5"), Some(-0.5));
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("abc"), None);
    }

    #[test]
    fn the_timing_follows_what_was_typed() {
        let mut input = SyncInput::default();
        assert_eq!(input.options(), SyncOptions::default());

        input.choice = ScaleChoice::Manual;
        input.manual_scale = "1,02".into();
        input.offset = "2,5".into();
        let options = input.options();
        assert_eq!(options.scale, ScaleMode::Manual { scale: 1.02 });
        assert_eq!(options.offset_s, 2.5);

        input.manual_scale = "nonsense".into();
        input.offset = String::new();
        let options = input.options();
        assert_eq!(
            options.scale,
            ScaleMode::Manual { scale: 1.0 },
            "no correction"
        );
        assert_eq!(options.offset_s, 0.0);

        input.choice = ScaleChoice::Off;
        assert_eq!(input.options().scale, ScaleMode::Off);
    }

    #[test]
    fn a_stretch_is_only_a_window_when_it_is_asked_for() {
        let mut input = StretchInput::default();
        assert_eq!(input.window(), None);

        input.enabled = true;
        input.start = "12,5".into();
        input.duration = "30".into();
        assert_eq!(
            input.window(),
            Some(Window {
                start_s: 12.5,
                duration_s: Some(30.0)
            })
        );

        input.start = "x".into();
        input.duration = String::new();
        assert_eq!(
            input.window(),
            Some(Window {
                start_s: 0.0,
                duration_s: None
            }),
            "an unreadable start is the beginning and an unreadable length is the rest"
        );
    }
}
