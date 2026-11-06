// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The messages of the application.

use std::path::PathBuf;
use std::sync::Arc;

use flight_replay_core::ffmpeg::{EncoderInfo, Tools};
use flight_replay_core::font::BundledFont;
use flight_replay_core::render::{Event, Outcome};
use flight_replay_core::telemetry::Recording;
use iced::Point;

use crate::preferences::ThemeChoice;
use crate::sound_params::Param;
use crate::state::{
    FileKind, FolderQueue, LoadedFont, LoadedOsd, LoadedSrt, LoadedVideo, ScaleChoice,
};
use crate::wizard::Step;

/// An edit of the OSD placement.
#[derive(Debug, Clone, Copy)]
pub enum OsdEdit {
    X(i32),
    Y(i32),
    Center,
    Reset,
    MaskMode(bool),
    ClearMasks,
}

/// An edit of the SRT fields.
#[derive(Debug, Clone, Copy)]
pub enum SrtEdit {
    Time(bool),
    SkyBattery(bool),
    GroundBattery(bool),
    Signal(bool),
    Latency(bool),
    Bitrate(bool),
    Distance(bool),
    PositionX(f32),
    PositionY(f32),
    Scale(f32),
}

/// An edit of the synchronisation.
#[derive(Debug, Clone)]
pub enum SyncEdit {
    Scale(ScaleChoice),
    ManualScale(String),
    Offset(String),
}

/// An edit of the sound.
#[derive(Debug, Clone)]
pub enum SoundEdit {
    Param(Param, f64),
    SampleRate(u32),
    Stereo(bool),
    /// Take the sound of a kind of motor and propeller: the place in `MOTOR_PRESETS`.
    MotorPreset(usize),
    Preset(String),
    PresetName(String),
    SavePreset,
    DeletePreset,
    ResetToDefaults,
}

/// An edit of the output.
#[derive(Debug, Clone)]
pub enum OutputEdit {
    Encoder(String),
    Bitrate(u32),
    Upscale(bool),
    ChromaKey(bool),
    AudioBitrate(u32),
    Faststart(bool),
    ShowUndetected(bool),
    Path(String),
    Overwrite(bool),
    ReplaceAudio(bool),
    Stretch(bool),
    StretchStart(String),
    StretchDuration(String),
}

/// What a render reports.
#[derive(Debug, Clone)]
pub enum RenderMessage {
    Event(Event),
    Done(Result<Arc<Outcome>, String>),
}

/// A message of the application.
#[derive(Debug, Clone)]
pub enum Message {
    // navigation
    Next,
    Back,
    GoTo(Step),
    // environment
    ToolsLocated(Result<Arc<Tools>, String>),
    EncodersDetected(Arc<Vec<EncoderInfo>>),
    ChooseFfmpegFolder,
    FfmpegFolderChosen(Option<PathBuf>),
    Theme(ThemeChoice),
    // files
    Browse(FileKind),
    FileChosen(FileKind, Option<PathBuf>),
    FileDropped(PathBuf),
    Clear(FileKind),
    /// A font that comes with the program was chosen from the list.
    UseBundledFont(&'static BundledFont),
    VideoLoaded(Result<Arc<LoadedVideo>, String>),
    OsdLoaded(Result<Arc<LoadedOsd>, String>),
    /// Choose another `.osd` file for a video made of several recordings.
    AddOsd,
    OsdAdded(Option<PathBuf>),
    /// Take the `.osd` file at this place out of the list.
    RemoveOsdFile(usize),
    /// Play the `.osd` file at this place before the one in front of it.
    MoveOsdFileUp(usize),
    SrtLoaded(Result<Arc<LoadedSrt>, String>),
    FontLoaded(Result<Arc<LoadedFont>, String>),
    TelemetryReady(PathBuf, Result<Arc<Recording>, String>),
    // layers
    DrawOsd(bool),
    DrawSrt(bool),
    MakeSound(bool),
    Osd(OsdEdit),
    Srt(SrtEdit),
    Sync(SyncEdit),
    Sound(SoundEdit),
    Output(OutputEdit),
    // preview
    PreviewTime(f64),
    PreviewTimeReleased,
    PreviewReady(u64, Result<Arc<crate::preview::PreviewResult>, String>),
    PreviewHover(Point),
    PreviewExited,
    PreviewPressed,
    // audio preview
    PlayAudio,
    AudioRendered(Result<Arc<Vec<i16>>, String>),
    StopAudio,
    AudioTick,
    // output
    BrowseOutput,
    OutputChosen(Option<PathBuf>),
    // queue
    QueueAdd,
    QueueAddFolder,
    QueueFolderChosen(Option<PathBuf>),
    QueueFolderPlanned(Result<Arc<FolderQueue>, String>),
    QueueRemove(usize),
    QueueClearFinished,
    QueueStart,
    // render
    StartRender,
    CancelRender,
    Render(RenderMessage),
    OpenOutputFolder,
    /// Show or hide the FFmpeg command before the render.
    ShowCommand(bool),
    CopyCommand,
    DismissNotice(usize),
    // application
    SystemTheme(iced::theme::Mode),
    CloseRequested,
}
