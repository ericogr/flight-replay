// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! What the application holds in memory: the files that are open and what was worked out from them.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use flight_replay_core::ffmpeg::VideoInfo;
use flight_replay_core::font::FontSheet;
use flight_replay_core::osd::OsdFile;
use flight_replay_core::render::{CancelToken, Outcome, Stage};
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
    pub path: PathBuf,
    pub file: Arc<OsdFile>,
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
