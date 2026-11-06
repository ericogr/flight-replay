// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! What the application holds in memory: the files that are open and what was worked out from them.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use flight_replay_core::ffmpeg::VideoInfo;
use flight_replay_core::font::FontSheet;
use flight_replay_core::osd::{JoinPart, OsdFile};
use flight_replay_core::render::{CancelToken, Outcome, RenderPlan, Stage};
use flight_replay_core::srt::SrtFile;
use flight_replay_core::synth::SignalSet;
use flight_replay_core::telemetry::{Profile, Recording};
use iced::widget::image::Handle;

/// A video that is open.
#[derive(Debug)]
pub struct LoadedVideo {
    pub path: PathBuf,
    pub info: VideoInfo,
}

/// An `.osd` file that is open.
#[derive(Debug)]
pub struct LoadedOsd {
    /// The first file.
    pub path: PathBuf,
    /// Every file, in the order they play. More than one when the video is a join of several recordings.
    pub paths: Vec<PathBuf>,
    /// The screens of all the files, on one timeline.
    pub file: Arc<OsdFile>,
    /// Where each file sits on that timeline.
    pub parts: Vec<JoinPart>,
    pub profile: Profile,
}

/// An `.srt` file that is open.
#[derive(Debug)]
pub struct LoadedSrt {
    pub path: PathBuf,
    pub file: Arc<SrtFile>,
}

/// An OSD font file that is open.
#[derive(Debug)]
pub struct LoadedFont {
    pub path: PathBuf,
    pub sheet: Arc<FontSheet>,
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
pub struct FolderPlans {
    pub items: Vec<QueueItem>,
    pub skipped: Vec<String>,
}

impl QueueItem {
    /// An item for a plan that was just made.
    pub fn new(plan: RenderPlan) -> Self {
        let title = plan.video.file_name().map_or_else(
            || plan.video.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
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
