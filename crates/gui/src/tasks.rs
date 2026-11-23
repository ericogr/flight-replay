// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The work done off the thread of the interface: opening files, probing, decoding, rendering.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use flight_replay_core::cancel::CancelToken;
use flight_replay_core::ffmpeg::install::{Build, install_here};
use flight_replay_core::ffmpeg::{self, Tools};
use flight_replay_core::font::{BundledFont, FontSheet, OsdFont};
use flight_replay_core::footage::VideoPart;
use flight_replay_core::osd::{OsdFile, join};
use flight_replay_core::paths::{file_name, parent_folder};
use flight_replay_core::render::{self, FlightDefaults, LeftOut, RenderPlan, Skipped};
use flight_replay_core::srt::SrtFile;
use flight_replay_core::synth::{SignalSet, SoundSettings, render_preview};
use flight_replay_core::telemetry::{LoadOptions, Profile, Recording};
use flight_replay_core::timeline::Timeline;
use iced::Task;
use iced::futures::channel::mpsc::UnboundedSender;

use crate::message::{InstallMessage, Message, RenderMessage};
use crate::preview::{self, PreviewRequest};
use crate::state::{
    FileKind, FolderQueue, FontSource, LoadedFont, LoadedOsd, LoadedSrt, Placing, QueueItem,
};

/// Runs `work` on its own thread and waits for it without blocking the interface.
pub async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (sender, receiver) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver
        .await
        .map_err(|_| "the background work stopped unexpectedly".to_owned())
}

/// A [`blocking`] job whose own error is a `String`.
async fn job<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    blocking(work).await.and_then(|result| result)
}

/// Looks for FFmpeg and FFprobe.
pub fn locate_tools(preferred: Option<PathBuf>) -> Task<Message> {
    Task::perform(
        job(move || {
            Tools::locate(preferred.as_deref())
                .map(Arc::new)
                .map_err(|e| e.to_string())
        }),
        Message::ToolsLocated,
    )
}

/// Finds out which encoders work.
pub fn detect_encoders(tools: Arc<Tools>) -> Task<Message> {
    Task::perform(blocking(move || ffmpeg::detect(&tools)), |result| {
        Message::EncodersDetected(Arc::new(result.unwrap_or_default()))
    })
}

/// Opens videos and reads their descriptions. `placing` says whether they are the flight or follow the ones that are
/// open: it is given back with the answer.
pub fn load_videos(tools: Arc<Tools>, paths: Vec<PathBuf>, placing: Placing) -> Task<Message> {
    Task::perform(
        job(move || {
            paths
                .into_iter()
                .map(|path| {
                    let info = ffmpeg::probe(&tools, &path)
                        .map_err(|e| format!("{}: {e}", file_name(&path)))?;
                    Ok(VideoPart { path, info })
                })
                .collect()
        }),
        move |result| Message::VideosLoaded(placing, result),
    )
}

/// Opens the `.osd` files of the flight, in the order they play, and puts them on one timeline. `generation` is the
/// number of this load: it is given back with the answer, so that an answer that was overtaken can be told.
pub fn load_osd(generation: u64, paths: Vec<PathBuf>) -> Task<Message> {
    Task::perform(
        job(move || {
            let mut files = Vec::with_capacity(paths.len());
            for path in &paths {
                let name = file_name(path);
                let file = OsdFile::open(path).map_err(|e| format!("{name}: {e}"))?;
                files.push((name, file));
            }
            let joined = join(files, 0.0).map_err(|e| e.to_string())?;
            let profile = Profile::from_firmware(&joined.file.firmware);
            Ok(Arc::new(LoadedOsd {
                paths,
                file: Arc::new(joined.file),
                parts: joined.parts,
                profile,
            }))
        }),
        move |result| Message::OsdLoaded(generation, result),
    )
}

/// Opens the `.srt` files of the flight, each with the time its video starts at on the timeline of the whole video,
/// and puts them on that timeline. `generation` is given back with the answer, as for [`load_osd`].
pub fn load_srt(generation: u64, files: Vec<(PathBuf, f64)>) -> Task<Message> {
    Task::perform(
        job(move || {
            let mut opened = Vec::with_capacity(files.len());
            for (path, start_s) in &files {
                let file = SrtFile::open(path).map_err(|e| format!("{}: {e}", file_name(path)))?;
                opened.push((file, *start_s));
            }
            let parts: Vec<(&SrtFile, f64)> = opened.iter().map(|(f, s)| (f, *s)).collect();
            Ok(Arc::new(LoadedSrt {
                paths: files.into_iter().map(|(path, _)| path).collect(),
                file: Arc::new(SrtFile::join(&parts)),
            }))
        }),
        move |result| Message::SrtLoaded(generation, result),
    )
}

/// Opens an OSD font file.
pub fn load_font(path: PathBuf) -> Task<Message> {
    Task::perform(
        job(move || {
            let sheet = FontSheet::open(&path).map_err(|e| e.to_string())?;
            Ok(Arc::new(LoadedFont {
                source: FontSource::File(path),
                font: OsdFont::single(sheet),
            }))
        }),
        Message::FontLoaded,
    )
}

/// Reads a font that comes with the program.
pub fn load_bundled_font(font: &'static BundledFont) -> Task<Message> {
    Task::perform(
        job(move || {
            let loaded = font.load().map_err(|e| format!("{}: {e}", font.name))?;
            Ok(Arc::new(LoadedFont {
                source: FontSource::Bundled(font),
                font: loaded,
            }))
        }),
        Message::FontLoaded,
    )
}

/// Reads the telemetry of the `.osd` files of the load with this number. It takes about a second for a whole flight.
pub fn decode_telemetry(generation: u64, osd: Arc<OsdFile>) -> Task<Message> {
    Task::perform(
        job(move || {
            Recording::decode(&osd, LoadOptions::default())
                .map(Arc::new)
                .map_err(|e| e.to_string())
        }),
        move |result| Message::TelemetryReady(generation, result),
    )
}

/// Opens a dialog of the system with `open` and waits for what the user chooses. The answer is the message that `then`
/// makes from it, wrapped in [`Message::Dialog`] so the program knows the dialog is closed. Every dialog goes through
/// here.
///
/// `open` runs when the task is run, not when it is made: on some systems a dialog opens as soon as its future exists.
fn ask<T, F>(
    open: impl FnOnce() -> F + Send + 'static,
    then: impl Fn(T) -> Message + Send + 'static,
) -> Task<Message>
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    Task::perform(async move { open().await }, move |answer| {
        Message::Dialog(Box::new(then(answer)))
    })
}

/// [`ask`] for a dialog that chooses one file or folder: the answer is its path, or `None` when the dialog was closed
/// without choosing.
fn ask_path<F>(
    open: impl FnOnce() -> F + Send + 'static,
    then: impl Fn(Option<PathBuf>) -> Message + Send + 'static,
) -> Task<Message>
where
    F: Future<Output = Option<rfd::FileHandle>> + Send + 'static,
{
    ask(
        move || async move { open().await.map(|handle| handle.path().to_path_buf()) },
        then,
    )
}

/// A dialog that chooses files of `kind`, starting in `start_dir`.
fn file_dialog(kind: FileKind, start_dir: Option<PathBuf>) -> rfd::AsyncFileDialog {
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title(format!("Choose the {}", kind.label()))
        .add_filter(kind.label(), kind.extensions());
    if let Some(dir) = start_dir {
        dialog = dialog.set_directory(dir);
    }
    dialog
}

/// Asks the user for a file of `kind`. `then` makes the message of the answer, which is `None` when the dialog was
/// closed without choosing.
pub fn pick_file(
    kind: FileKind,
    start_dir: Option<PathBuf>,
    then: impl Fn(Option<PathBuf>) -> Message + Send + 'static,
) -> Task<Message> {
    ask_path(move || file_dialog(kind, start_dir).pick_file(), then)
}

/// Asks the user for one or several videos. `then` makes the message of the answer, which has no video when the
/// dialog was closed without choosing.
pub fn pick_videos(
    start_dir: Option<PathBuf>,
    then: impl Fn(Vec<PathBuf>) -> Message + Send + 'static,
) -> Task<Message> {
    ask(
        move || async move {
            file_dialog(FileKind::Video, start_dir)
                .pick_files()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|handle| handle.path().to_path_buf())
                .collect::<Vec<_>>()
        },
        then,
    )
}

/// Asks the user for a folder.
pub fn pick_folder(title: &'static str, then: fn(Option<PathBuf>) -> Message) -> Task<Message> {
    ask_path(
        move || rfd::AsyncFileDialog::new().set_title(title).pick_folder(),
        then,
    )
}

/// Asks the user where to save the output.
pub fn pick_output(suggested: PathBuf) -> Task<Message> {
    ask_path(
        move || {
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_title("Save the video as")
                .add_filter("MP4 video", &["mp4"]);
            if let Some(dir) = parent_folder(&suggested) {
                dialog = dialog.set_directory(dir);
            }
            if let Some(name) = suggested.file_name() {
                dialog = dialog.set_file_name(name.to_string_lossy());
            }
            dialog.save_file()
        },
        Message::OutputChosen,
    )
}

/// Makes the preview picture.
pub fn make_preview(request: PreviewRequest) -> Task<Message> {
    let sequence = request.sequence;
    Task::perform(
        job(move || preview::make(&request).map(Arc::new)),
        move |result| Message::PreviewReady(sequence, result),
    )
}

/// Makes a stretch of the sound for listening.
pub fn render_audio_preview(
    settings: SoundSettings,
    signals: Arc<SignalSet>,
    timeline: Timeline,
    start_s: f64,
    duration_s: f64,
) -> Task<Message> {
    Task::perform(
        job(move || {
            render_preview(&settings, &signals, timeline, start_s, duration_s)
                .map(Arc::new)
                .map_err(|e| e.to_string())
        }),
        Message::AudioRendered,
    )
}

/// Runs `work` on its own thread and turns what it sends into the messages of a task. The task ends when `work` does.
fn stream_from_thread<T: Send + 'static>(
    work: impl FnOnce(&UnboundedSender<T>) + Send + 'static,
) -> Task<T> {
    let (sender, receiver) = iced::futures::channel::mpsc::unbounded();
    std::thread::spawn(move || work(&sender));
    Task::stream(receiver)
}

/// Runs a render and reports what it does.
pub fn start_render(tools: Arc<Tools>, plan: RenderPlan, cancel: CancelToken) -> Task<Message> {
    stream_from_thread(move |sender| {
        let result = render::render(&tools, &plan, &cancel, &mut |event| {
            let _ = sender.unbounded_send(RenderMessage::Event(event));
        });
        let _ = sender.unbounded_send(RenderMessage::Done(
            result.map(Arc::new).map_err(|e| e.to_string()),
        ));
    })
    .map(Message::Render)
}

/// Installs FFmpeg and reports how far it has come. `id` is the number of the install, which every report carries.
pub fn install_ffmpeg(id: u64, build: &'static Build, cancel: CancelToken) -> Task<Message> {
    stream_from_thread(move |sender| {
        let result = install_here(build, &cancel, &mut |report| {
            let _ = sender.unbounded_send(InstallMessage::Progress(report));
        });
        let _ = sender.unbounded_send(InstallMessage::Done(
            result.map(|_| ()).map_err(|error| error.to_string()),
        ));
    })
    .map(move |message| Message::Install(id, message))
}

/// Plans the videos of a folder, for the queue. `planned` are the outputs the queue already writes.
pub fn plan_folder(
    tools: Arc<Tools>,
    folder: PathBuf,
    defaults: FlightDefaults,
    planned: Vec<PathBuf>,
) -> Task<Message> {
    Task::perform(
        job(move || {
            let plans = render::plan_folder(&tools, &folder, &defaults, &planned)
                .map_err(|e| format!("could not read {}: {e}", folder.display()))?;
            Ok(Arc::new(FolderQueue {
                items: plans.plans.into_iter().map(QueueItem::new).collect(),
                left_out: plans.left_out.iter().map(LeftOut::describe).collect(),
                skipped: plans.skipped.iter().map(Skipped::describe).collect(),
            }))
        }),
        Message::QueueFolderPlanned,
    )
}

/// Opens a folder in the file manager of the system.
pub fn open_folder(folder: &Path) {
    if let Err(error) = opener::open(folder) {
        tracing::warn!(%error, "could not open the folder");
    }
}

/// Sends `message` after `milliseconds`.
pub fn after(milliseconds: u64, message: Message) -> Task<Message> {
    Task::perform(
        blocking(move || std::thread::sleep(std::time::Duration::from_millis(milliseconds))),
        move |_| message,
    )
}
