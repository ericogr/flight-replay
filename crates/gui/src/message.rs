// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The messages of the application.

use std::path::PathBuf;
use std::sync::Arc;

use flight_replay_core::ffmpeg::install::Progress;
use flight_replay_core::ffmpeg::{EncoderInfo, Tools};
use flight_replay_core::font::BundledFont;
use flight_replay_core::render::{Event, Outcome};
use flight_replay_core::synth::AircraftKind;
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
    /// Choose the kind of aircraft: it sets what belongs to the aircraft and the first sound of the kind.
    Aircraft(AircraftKind),
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

/// What an install of FFmpeg reports.
#[derive(Debug, Clone)]
pub enum InstallMessage {
    Progress(Progress),
    /// It ended: the error is the reason, in words for the user.
    Done(Result<(), String>),
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
    // the settings screen
    /// What a dialog of the system answered: it is closed now. It holds the message that says what was chosen, which is
    /// handled as if it had arrived by itself. Every dialog is asked for through `tasks`, which wraps the answer.
    Dialog(Box<Message>),
    OpenSettings,
    CloseSettings,
    /// Download and install FFmpeg, because the user asked for it.
    InstallFfmpeg,
    CancelInstall,
    /// What the install with this number reports. What an install that was cancelled says is ignored.
    Install(u64, InstallMessage),
    /// Delete the copy of FFmpeg that the application installed.
    RemoveInstalledFfmpeg,
    /// Look for FFmpeg again.
    RecheckFfmpeg,
    OpenSettingsFolder,
    OpenProjectPage,
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
    /// Show the layers of the other steps in the preview, or only the layer of the step.
    ShowOtherLayers(bool),
    PreviewTime(f64),
    PreviewTimeReleased,
    PreviewReady(u64, Result<Arc<crate::preview::PreviewResult>, String>),
    PreviewHover(Point),
    PreviewExited,
    PreviewPressed,
    // the preview in motion
    /// Make the motor sound from the time of the preview, and play it with the picture.
    PlayAudio,
    AudioRendered(Result<Arc<Vec<i16>>, String>),
    /// Play the picture alone from the time of the preview.
    PlayPicture,
    StopPlay,
    /// Time to look at whether the play with this number is over.
    PlayTick(u64),
    /// The picture of the play with this number starts again, unless there was another edit after the one with this
    /// number.
    RestartPlayback {
        play: u64,
        edit: u64,
    },
    /// A picture of the video playing along with the sound, from the stream with this number.
    PlayFrame(u64, Result<Arc<crate::playback::PlayFrame>, String>),
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
    /// The user asked to bring every setting back to the default: the program waits for the confirmation.
    AskResetAll,
    /// The user changed their mind about the reset.
    CancelResetAll,
    /// The user confirmed the reset.
    ResetAll,
    /// The time has come to write the settings that changed in a burst of edits.
    SaveSettings,
}
