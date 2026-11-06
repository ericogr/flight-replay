// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The work done off the thread of the interface: opening files, probing, decoding, rendering.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use flight_replay_core::config::Settings;
use flight_replay_core::discover;
use flight_replay_core::ffmpeg::{self, EncoderInfo, Tools};
use flight_replay_core::font::FontSheet;
use flight_replay_core::osd::{OsdFile, join};
use flight_replay_core::render::{
    CancelToken, OutputPlan, PlanInputs, RenderPlan, build_plan, render,
};
use flight_replay_core::srt::SrtFile;
use flight_replay_core::synth::{SignalSet, SoundSettings, render_preview};
use flight_replay_core::telemetry::{LoadOptions, Profile, Recording};
use flight_replay_core::timeline::{SyncOptions, Timeline};
use iced::Task;

use crate::message::{Message, RenderMessage};
use crate::preview::{self, PreviewRequest};
use crate::state::{
    FileKind, FolderPlans, LoadedFont, LoadedOsd, LoadedSrt, LoadedVideo, QueueItem,
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

/// Opens an `.osd` file.
/// Opens the `.osd` files of one video, and puts them on one timeline when there are several.
pub fn load_osd(paths: Vec<PathBuf>) -> Task<Message> {
    Task::perform(
        job(move || {
            let mut files = Vec::with_capacity(paths.len());
            for path in &paths {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                let file = OsdFile::open(path).map_err(|e| format!("{name}: {e}"))?;
                files.push((name, file));
            }
            let joined = join(files, 0.0).map_err(|e| e.to_string())?;
            let profile = Profile::from_firmware(&joined.file.firmware);
            Ok(Arc::new(LoadedOsd {
                path: paths[0].clone(),
                paths,
                file: Arc::new(joined.file),
                parts: joined.parts,
                profile,
            }))
        }),
        Message::OsdLoaded,
    )
}

/// Asks the user for another `.osd` file of the same video.
pub fn pick_more_osd(start_dir: Option<PathBuf>) -> Task<Message> {
    Task::perform(
        async move {
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_title("Choose the next .osd file")
                .add_filter(".osd file", FileKind::Osd.extensions());
            if let Some(dir) = start_dir {
                dialog = dialog.set_directory(dir);
            }
            dialog
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        Message::OsdAdded,
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
                path,
                sheet: Arc::new(sheet),
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

/// Asks the user for a file.
pub fn pick_file(kind: FileKind, start_dir: Option<PathBuf>) -> Task<Message> {
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
        move |path| Message::FileChosen(kind, path),
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
            if let Some(dir) = suggested.parent().filter(|p| !p.as_os_str().is_empty()) {
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
        let result = render(&tools, &plan, &cancel, &mut |event| {
            let _ = events.unbounded_send(RenderMessage::Event(event));
        });
        let _ = sender.unbounded_send(RenderMessage::Done(
            result.map(Arc::new).map_err(|e| e.to_string()),
        ));
    });
    Task::stream(receiver).map(Message::Render)
}

/// What the videos of a folder are rendered with: what the wizard has now, apart from the files.
pub struct Template {
    pub settings: Settings,
    pub encoders: Arc<Vec<EncoderInfo>>,
    pub font: Option<Arc<FontSheet>>,
    pub sync: SyncOptions,
    pub overwrite: bool,
    pub replace_audio: bool,
    /// Output files that the queue already has.
    pub queued: Vec<PathBuf>,
}

/// Makes a plan for every video of a folder, with the files found next to each.
pub fn plan_folder(tools: Arc<Tools>, folder: PathBuf, template: Template) -> Task<Message> {
    Task::perform(
        job(move || {
            let flights = discover::flights_in(&folder)
                .map_err(|e| format!("could not read {}: {e}", folder.display()))?;
            let mut plans = FolderPlans {
                items: Vec::new(),
                skipped: Vec::new(),
            };
            for flight in flights {
                let name = flight.video.file_name().map_or_else(
                    || flight.video.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                match plan_flight(&tools, &flight, &template) {
                    Ok(item) => plans.items.push(item),
                    Err(reason) => plans.skipped.push(format!("{name}: {reason}")),
                }
            }
            Ok(Arc::new(plans))
        }),
        Message::QueueFolderPlanned,
    )
}

fn plan_flight(
    tools: &Tools,
    flight: &discover::Flight,
    template: &Template,
) -> Result<QueueItem, String> {
    let settings = &template.settings;
    let output = OutputPlan::default_path(&flight.video, false);
    if template.queued.contains(&output) {
        return Err("it is in the queue already".to_owned());
    }
    if !template.overwrite && output.exists() {
        return Err("its output file exists already".to_owned());
    }
    let info = ffmpeg::probe(tools, &flight.video).map_err(|e| e.to_string())?;
    let osd = if settings.draw_osd || settings.make_sound {
        let path = flight
            .companions
            .osd
            .as_ref()
            .ok_or("there is no .osd file next to it")?;
        Some(OsdFile::open(path).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let srt = if settings.draw_srt {
        let path = flight
            .companions
            .srt
            .as_ref()
            .ok_or("there is no .srt file next to it")?;
        Some(SrtFile::open(path).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let plan = build_plan(&PlanInputs {
        video: flight.video.clone(),
        info,
        osd: osd.as_ref(),
        osd_path: flight.companions.osd.clone(),
        srt: srt.as_ref(),
        font: template.font.as_deref(),
        settings,
        encoders: &template.encoders,
        sync: template.sync,
        window: None,
        output,
        overwrite: template.overwrite,
        replace_audio: template.replace_audio,
    })
    .map_err(|problems| problems.join(" "))?;
    Ok(QueueItem::new(plan))
}

/// Opens the folder of a file in the file manager of the system.
pub fn open_folder(path: &Path) {
    let folder = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if let Err(error) = opener::open(folder) {
        tracing::warn!(%error, "could not open the folder");
    }
}

/// Sends a tick after a while. It checks whether a sound has finished playing.
pub fn tick_after(milliseconds: u64) -> Task<Message> {
    Task::perform(
        blocking(move || std::thread::sleep(std::time::Duration::from_millis(milliseconds))),
        |_| Message::AudioTick,
    )
}
