// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The work done off the thread of the interface: opening files, probing, decoding, rendering.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use flight_replay_core::ffmpeg::{self, Tools};
use flight_replay_core::font::{BundledFont, FontSheet, OsdFont};
use flight_replay_core::osd::{OsdFile, join};
use flight_replay_core::paths::{file_name, parent_folder};
use flight_replay_core::render::{self, CancelToken, FlightDefaults, LeftOut, RenderPlan, Skipped};
use flight_replay_core::srt::SrtFile;
use flight_replay_core::synth::{SignalSet, SoundSettings, render_preview};
use flight_replay_core::telemetry::{LoadOptions, Profile, Recording};
use flight_replay_core::timeline::Timeline;
use iced::Task;

use crate::message::{Message, RenderMessage};
use crate::preview::{self, PreviewRequest};
use crate::state::{
    FileKind, FolderQueue, FontSource, LoadedFont, LoadedOsd, LoadedSrt, LoadedVideo, QueueItem,
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

/// Opens a video and reads its description.
pub fn load_video(tools: Arc<Tools>, path: PathBuf) -> Task<Message> {
    Task::perform(
        job(move || {
            let info = ffmpeg::probe(&tools, &path).map_err(|e| e.to_string())?;
            Ok(Arc::new(LoadedVideo { path, info }))
        }),
        Message::VideoLoaded,
    )
}

/// Opens the `.osd` files of one video, and puts them on one timeline when there are several.
pub fn load_osd(paths: Vec<PathBuf>) -> Task<Message> {
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
        Message::OsdLoaded,
    )
}

/// Opens an `.srt` file.
pub fn load_srt(path: PathBuf) -> Task<Message> {
    Task::perform(
        job(move || {
            let file = SrtFile::open(&path).map_err(|e| e.to_string())?;
            Ok(Arc::new(LoadedSrt {
                path,
                file: Arc::new(file),
            }))
        }),
        Message::SrtLoaded,
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

/// Reads the telemetry of an `.osd` file. It takes about a second for a whole flight.
pub fn decode_telemetry(path: PathBuf, osd: Arc<OsdFile>) -> Task<Message> {
    let answer_for = path;
    Task::perform(
        job(move || {
            Recording::decode(&osd, LoadOptions::default())
                .map(Arc::new)
                .map_err(|e| e.to_string())
        }),
        move |result| Message::TelemetryReady(answer_for.clone(), result),
    )
}

/// Asks the user for a file of `kind`. `then` makes the message of the answer, which is `None` when the dialog was
/// closed without choosing.
pub fn pick_file(
    kind: FileKind,
    start_dir: Option<PathBuf>,
    then: impl Fn(Option<PathBuf>) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(
        async move {
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_title(format!("Choose the {}", kind.label()))
                .add_filter(kind.label(), kind.extensions());
            if let Some(dir) = start_dir {
                dialog = dialog.set_directory(dir);
            }
            dialog
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        then,
    )
}

/// Asks the user for a folder.
pub fn pick_folder(title: &'static str, then: fn(Option<PathBuf>) -> Message) -> Task<Message> {
    Task::perform(
        async move {
            rfd::AsyncFileDialog::new()
                .set_title(title)
                .pick_folder()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        then,
    )
}

/// Asks the user where to save the output.
pub fn pick_output(suggested: PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_title("Save the video as")
                .add_filter("MP4 video", &["mp4"]);
            if let Some(dir) = parent_folder(&suggested) {
                dialog = dialog.set_directory(dir);
            }
            if let Some(name) = suggested.file_name() {
                dialog = dialog.set_file_name(name.to_string_lossy());
            }
            dialog
                .save_file()
                .await
                .map(|handle| handle.path().to_path_buf())
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

/// Runs a render and reports what it does.
pub fn start_render(tools: Arc<Tools>, plan: RenderPlan, cancel: CancelToken) -> Task<Message> {
    let (sender, receiver) = iced::futures::channel::mpsc::unbounded::<RenderMessage>();
    std::thread::spawn(move || {
        let events = sender.clone();
        let result = render::render(&tools, &plan, &cancel, &mut |event| {
            let _ = events.unbounded_send(RenderMessage::Event(event));
        });
        let _ = sender.unbounded_send(RenderMessage::Done(
            result.map(Arc::new).map_err(|e| e.to_string()),
        ));
    });
    Task::stream(receiver).map(Message::Render)
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

/// Sends [`Message::AudioTick`] after a while, so the picture can follow the sound that is playing.
pub fn audio_tick_after(milliseconds: u64) -> Task<Message> {
    Task::perform(
        blocking(move || std::thread::sleep(std::time::Duration::from_millis(milliseconds))),
        |_| Message::AudioTick,
    )
}
