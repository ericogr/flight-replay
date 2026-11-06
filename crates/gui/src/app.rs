// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The application: its state, how it reacts to messages, and the subscriptions.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use flight_replay_core::config::{Settings, ThemeChoice};
use flight_replay_core::discover;
use flight_replay_core::ffmpeg::{EncoderInfo, Tools};
use flight_replay_core::font::{CharacterSize, ScaledFont};
use flight_replay_core::render::{
    CancelToken, EncoderChoice, Event, OsdLayer, OutputPlan, Overwrite, RenderPlan, SrtLayer,
    Stage, VideoOutput, Window, command_preview,
};
use flight_replay_core::synth::SignalSet;
use flight_replay_core::telemetry::TelemetrySupport;
use flight_replay_core::timeline::{ScaleMode, SyncOptions, Timeline};
use iced::{Subscription, Task, Theme};

use crate::audio::AudioPlayer;
use crate::message::{
    Message, OsdEdit, OutputEdit, RenderMessage, ScaleChoice, SoundEdit, SrtEdit, SyncEdit,
};
use crate::preview::{OsdPreview, PreviewRequest, SrtPreview};
use crate::sound_params;
use crate::state::{
    FileKind, LoadedFont, LoadedOsd, LoadedSrt, LoadedVideo, Preview, RenderState, Running,
    Telemetry,
};
use crate::tasks;
use crate::wizard::{self, Context, Step};

/// The state of the search for FFmpeg.
pub enum ToolsState {
    Searching,
    Found(Arc<Tools>),
    Missing(String),
}

/// What is being loaded right now.
#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one flag per file that can be loading at the same time"
)]
pub struct Busy {
    pub video: bool,
    pub osd: bool,
    pub srt: bool,
    pub font: bool,
    pub telemetry: bool,
    pub audio: bool,
}

/// The fields of the output step that are text while they are being typed.
pub struct StretchInput {
    pub enabled: bool,
    pub start: String,
    pub duration: String,
}

/// The application.
pub struct FlightReplay {
    pub settings: Settings,
    system_mode: iced::theme::Mode,
    settings_path: Option<PathBuf>,
    pub notices: Vec<String>,

    pub tools: ToolsState,
    pub encoders: Option<Arc<Vec<EncoderInfo>>>,

    pub step: Step,
    pub video: Option<Arc<LoadedVideo>>,
    pub osd: Option<Arc<LoadedOsd>>,
    pub srt: Option<Arc<LoadedSrt>>,
    pub font: Option<Arc<LoadedFont>>,
    scaled_font: Option<Arc<ScaledFont>>,
    pub telemetry: Option<Telemetry>,
    pub busy: Busy,
    pub file_errors: Vec<(FileKind, String)>,

    pub sync: SyncOptions,
    pub scale_choice: ScaleChoice,
    pub manual_scale: String,
    pub offset: String,

    pub preview: Preview,

    pub preset_name: String,
    pub selected_preset: Option<String>,
    audio: Option<AudioPlayer>,
    pub audio_playing: bool,
    pub audio_error: Option<String>,

    pub output_path: String,
    pub overwrite: bool,
    pub replace_audio: bool,
    pub stretch: StretchInput,

    pub render: RenderState,
    pub command_lines: Vec<String>,
}

impl FlightReplay {
    /// Starts the application: reads the settings and looks for FFmpeg.
    pub fn boot() -> (Self, Task<Message>) {
        let app = Self::new(Settings::default_path());
        let preferred = app.settings.ffmpeg_dir.clone();
        (
            app,
            Task::batch([
                tasks::locate_tools(preferred),
                iced::system::theme().map(Message::SystemTheme),
            ]),
        )
    }

    /// The application at the first step, with the settings read from `settings_path`.
    pub fn new(settings_path: Option<PathBuf>) -> Self {
        let (settings, notice) = settings_path
            .as_deref()
            .map_or_else(|| (Settings::default(), None), Settings::load_or_default);
        Self {
            settings,
            system_mode: iced::theme::Mode::None,
            settings_path,
            notices: notice.into_iter().collect(),
            tools: ToolsState::Searching,
            encoders: None,
            step: Step::Welcome,
            video: None,
            osd: None,
            srt: None,
            font: None,
            scaled_font: None,
            telemetry: None,
            busy: Busy::default(),
            file_errors: Vec::new(),
            sync: SyncOptions::default(),
            scale_choice: ScaleChoice::Auto,
            manual_scale: "1.000".to_owned(),
            offset: "0".to_owned(),
            preview: Preview::default(),
            preset_name: String::new(),
            selected_preset: None,
            audio: None,
            audio_playing: false,
            audio_error: None,
            output_path: String::new(),
            overwrite: false,
            replace_audio: false,
            stretch: StretchInput {
                enabled: false,
                start: "0".to_owned(),
                duration: "30".to_owned(),
            },
            render: RenderState::Idle,
            command_lines: Vec::new(),
        }
    }

    /// A file given on the command line, such as the video that the system opens with this application.
    fn path_from_arguments() -> Option<PathBuf> {
        std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .filter(|p| p.is_file())
    }

    /// The colour scheme.
    pub fn theme(&self) -> Theme {
        match self.settings.theme {
            ThemeChoice::System => <Theme as iced::theme::Base>::default(self.system_mode),
            ThemeChoice::Light => Theme::Light,
            ThemeChoice::Dark => Theme::Dark,
        }
    }

    /// Drag and drop, closing the window, and changes of the system colour scheme.
    pub fn subscription() -> Subscription<Message> {
        let events = iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                Some(Message::FileDropped(path))
            }
            iced::Event::Window(iced::window::Event::CloseRequested) => {
                Some(Message::CloseRequested)
            }
            _ => None,
        });
        Subscription::batch([
            events,
            iced::system::theme_changes().map(Message::SystemTheme),
        ])
    }

    // ----- what the wizard needs to know

    /// The tools, if they were found.
    pub fn tools(&self) -> Option<Arc<Tools>> {
        match &self.tools {
            ToolsState::Found(tools) => Some(Arc::clone(tools)),
            _ => None,
        }
    }

    /// The facts the wizard rules work from.
    pub fn context(&self) -> Context {
        let signals_error = match &self.telemetry {
            Some(Telemetry {
                signals: Err(error),
                ..
            }) => Some(error.clone()),
            _ => None,
        };
        Context {
            tools_found: self.tools().is_some(),
            video: self.video.is_some(),
            osd: self.osd.is_some(),
            font: self.font.is_some(),
            srt: self.srt.is_some(),
            draw_osd: self.settings.draw_osd,
            draw_srt: self.settings.draw_srt,
            make_sound: self.settings.make_sound,
            sound_unavailable: self.sound_unavailable(),
            signals_error,
            output_path: !self.output_path.trim().is_empty(),
            encoder: !self.settings.output.encoder.is_empty(),
        }
    }

    /// Why the `.osd` cannot drive the sound, if it cannot.
    pub fn sound_unavailable(&self) -> Option<String> {
        let osd = self.osd.as_ref()?;
        (osd.profile.telemetry_support() != TelemetrySupport::Available)
            .then(|| osd.profile.explanation().to_owned())
    }

    /// The mapping between video time and telemetry time.
    pub fn timeline(&self) -> Option<Timeline> {
        let (video, osd) = (self.video.as_ref()?, self.osd.as_ref()?);
        Some(Timeline::new(
            self.sync,
            video.info.duration_s,
            osd.file.duration_s(),
        ))
    }

    /// The signals of the sound, if the telemetry has been read and has what the sound needs.
    pub fn signals(&self) -> Option<Arc<SignalSet>> {
        match &self.telemetry {
            Some(Telemetry {
                signals: Ok(signals),
                ..
            }) => Some(Arc::clone(signals)),
            _ => None,
        }
    }

    pub fn file_error(&self, kind: FileKind) -> Option<&str> {
        self.file_errors
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, e)| e.as_str())
    }

    fn set_file_error(&mut self, kind: FileKind, error: Option<String>) {
        self.file_errors.retain(|(k, _)| *k != kind);
        if let Some(error) = error {
            self.file_errors.push((kind, error));
        }
    }

    pub fn is_rendering(&self) -> bool {
        matches!(self.render, RenderState::Running(_))
    }

    // ----- settings

    fn save_settings(&self) {
        if let Some(path) = &self.settings_path
            && let Err(error) = self.settings.save(path)
        {
            tracing::warn!(%error, "could not save the settings");
        }
    }

    // ----- derived data

    fn rebuild_scaled_font(&mut self) {
        self.scaled_font = match (&self.font, &self.video) {
            (Some(font), Some(video)) => Some(Arc::new(
                font.sheet
                    .scaled(CharacterSize::for_video_height(video.info.height)),
            )),
            _ => None,
        };
    }

    fn recompute_signals(&mut self) {
        let (Some(osd), Some(telemetry)) = (&self.osd, &mut self.telemetry) else {
            return;
        };
        let name = osd.path.file_name().map_or_else(
            || "the .osd".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        telemetry.signals = SignalSet::collect(
            &telemetry.recording.table,
            &self.settings.sound,
            &name,
            telemetry.recording.frames,
        )
        .map(Arc::new)
        .map_err(|e| e.to_string());
    }

    fn apply_sync_text(&mut self) {
        let scale = match self.scale_choice {
            ScaleChoice::Auto => ScaleMode::Auto,
            ScaleChoice::Off => ScaleMode::Off,
            ScaleChoice::Manual => ScaleMode::Manual {
                scale: self
                    .manual_scale
                    .trim()
                    .replace(',', ".")
                    .parse()
                    .unwrap_or(1.0),
            },
        };
        let offset_s = self.offset.trim().replace(',', ".").parse().unwrap_or(0.0);
        self.sync = SyncOptions { scale, offset_s };
    }

    // ----- the preview

    fn refresh_preview(&mut self) -> Task<Message> {
        let (Some(tools), Some(video)) = (self.tools(), self.video.clone()) else {
            return Task::none();
        };
        let osd = match (&self.osd, &self.scaled_font, self.settings.draw_osd) {
            (Some(osd), Some(font), true) => Some(OsdPreview {
                file: Arc::clone(&osd.file),
                font: Arc::clone(font),
                options: self.settings.osd.clone(),
            }),
            _ => None,
        };
        let srt = match (&self.srt, self.settings.draw_srt) {
            (Some(srt), true) => Some(SrtPreview {
                file: Arc::clone(&srt.file),
                options: self.settings.srt.clone(),
            }),
            _ => None,
        };
        self.preview.sequence += 1;
        self.preview.busy = true;
        self.preview.error = None;
        let show_grid = self.preview.mask_mode && osd.is_some();
        tasks::make_preview(PreviewRequest {
            sequence: self.preview.sequence,
            tools,
            video: video.path.clone(),
            info: video.info.clone(),
            time_s: self.preview.time_s,
            base: self.preview.base.clone(),
            osd,
            srt,
            timeline: self.timeline(),
            show_grid,
        })
    }

    /// The cell of the OSD under the pointer, if any.
    fn cell_under_pointer(&self) -> Option<(u32, u32)> {
        let (video, osd, font) = (
            self.video.as_ref()?,
            self.osd.as_ref()?,
            self.scaled_font.as_ref()?,
        );
        let point = self.preview.hover?;
        let (shown_w, shown_h) = self.preview.shown;
        if shown_w == 0 || shown_h == 0 {
            return None;
        }
        let x = f64::from(point.x) / f64::from(shown_w) * f64::from(video.info.width);
        let y = f64::from(point.y) / f64::from(shown_h) * f64::from(video.info.height);
        let column =
            ((x - f64::from(self.settings.osd.x_offset)) / f64::from(font.size().width())).floor();
        let row =
            ((y - f64::from(self.settings.osd.y_offset)) / f64::from(font.size().height())).floor();
        let in_grid = column >= 0.0
            && row >= 0.0
            && column < f64::from(osd.file.columns)
            && row < f64::from(osd.file.rows);
        in_grid.then_some((column as u32, row as u32))
    }

    // ----- loading files

    fn open_file(&mut self, kind: FileKind, path: PathBuf) -> Task<Message> {
        self.set_file_error(kind, None);
        match kind {
            FileKind::Video => {
                let Some(tools) = self.tools() else {
                    return Task::none();
                };
                self.busy.video = true;
                tasks::load_video(tools, path)
            }
            FileKind::Osd => {
                self.busy.osd = true;
                tasks::load_osd(path)
            }
            FileKind::Srt => {
                self.busy.srt = true;
                tasks::load_srt(path)
            }
            FileKind::Font => {
                self.busy.font = true;
                tasks::load_font(path)
            }
        }
    }

    /// Opens a file that was dropped on the window, by what it looks like.
    fn open_dropped(&mut self, path: PathBuf) -> Task<Message> {
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let kind = match extension.as_str() {
            "mp4" | "mov" | "mkv" => FileKind::Video,
            "osd" => FileKind::Osd,
            "srt" => FileKind::Srt,
            "png" => FileKind::Font,
            _ => {
                self.notices.push(format!(
                    "{} is not a video, .osd, .srt or font file",
                    path.display()
                ));
                return Task::none();
            }
        };
        if kind != FileKind::Video && self.video.is_none() && kind != FileKind::Font {
            self.notices
                .push("Open the video first, then the files that go with it".to_owned());
            return Task::none();
        }
        self.open_file(kind, path)
    }

    fn clear_file(&mut self, kind: FileKind) -> Task<Message> {
        self.set_file_error(kind, None);
        match kind {
            FileKind::Video => {
                self.video = None;
                self.osd = None;
                self.srt = None;
                self.telemetry = None;
                self.preview = Preview::default();
            }
            FileKind::Osd => {
                self.osd = None;
                self.telemetry = None;
                self.settings.draw_osd = false;
                self.settings.make_sound = false;
            }
            FileKind::Srt => {
                self.srt = None;
                self.settings.draw_srt = false;
            }
            FileKind::Font => {
                self.font = None;
                self.settings.font_path = None;
                self.scaled_font = None;
                self.settings.draw_osd = false;
            }
        }
        self.refresh_preview_if_open()
    }

    fn refresh_preview_if_open(&mut self) -> Task<Message> {
        if self.video.is_some() {
            self.refresh_preview()
        } else {
            Task::none()
        }
    }

    // ----- the plan of the render

    /// Builds the plan from what the user chose.
    pub fn build_plan(&self) -> Result<RenderPlan, Vec<String>> {
        let mut problems = Vec::new();
        let Some(video) = &self.video else {
            return Err(vec!["Open a video.".into()]);
        };
        let output = &self.settings.output;
        let draws = self.settings.draw_osd || self.settings.draw_srt;
        let encoder = self.encoder_choice();
        let video_output = if draws {
            if let Some(choice) = encoder {
                VideoOutput::Encode(choice)
            } else {
                problems.push("Choose a video encoder.".to_owned());
                VideoOutput::Copy
            }
        } else {
            VideoOutput::Copy
        };

        let osd_layer = match (&self.font, self.settings.draw_osd) {
            (Some(font), true) => Some(OsdLayer {
                font: (*font.sheet).clone(),
                options: self.settings.osd.clone(),
            }),
            (None, true) => {
                problems.push("Choose the OSD font file.".to_owned());
                None
            }
            _ => None,
        };
        let srt_layer = match (&self.srt, self.settings.draw_srt) {
            (Some(srt), true) => Some(SrtLayer {
                file: (*srt.file).clone(),
                options: self.settings.srt.clone(),
            }),
            _ => None,
        };
        let window = self.stretch.enabled.then(|| Window {
            start_s: self
                .stretch
                .start
                .trim()
                .replace(',', ".")
                .parse()
                .unwrap_or(0.0),
            duration_s: self.stretch.duration.trim().replace(',', ".").parse().ok(),
        });
        let needs_osd = self.settings.draw_osd || self.settings.make_sound;
        let plan = RenderPlan {
            video: video.path.clone(),
            info: video.info.clone(),
            osd: needs_osd
                .then(|| self.osd.as_ref().map(|o| (*o.file).clone()))
                .flatten(),
            osd_path: self.osd.as_ref().map(|o| o.path.clone()),
            osd_layer,
            srt_layer,
            chroma_key: (output.chroma_key && draws).then_some(output.chroma_key_color),
            sound: self
                .settings
                .make_sound
                .then(|| self.settings.sound.clone()),
            sync: self.sync,
            window,
            output: OutputPlan {
                path: PathBuf::from(self.output_path.trim()),
                video: video_output,
                audio_bitrate_kbps: output.audio_bitrate_kbps,
                faststart: output.faststart,
                overwrite: if self.overwrite {
                    Overwrite::Replace
                } else {
                    Overwrite::Refuse
                },
                replace_audio: self.replace_audio,
            },
        };
        match plan.validate() {
            Ok(()) if problems.is_empty() => Ok(plan),
            Ok(()) => Err(problems),
            Err(more) => {
                problems.extend(more);
                Err(problems)
            }
        }
    }

    fn encoder_choice(&self) -> Option<EncoderChoice> {
        let output = &self.settings.output;
        if output.encoder.is_empty() {
            return None;
        }
        let extra_args = self
            .encoders
            .as_ref()
            .and_then(|list| list.iter().find(|e| e.name == output.encoder))
            .map(|e| e.extra_args.clone())
            .unwrap_or_default();
        Some(EncoderChoice {
            name: output.encoder.clone(),
            extra_args,
            bitrate_mbps: output.bitrate_mbps,
            upscale_1440p: output.upscale_1440p,
        })
    }

    fn refresh_command_lines(&mut self) {
        self.command_lines = match (self.build_plan(), self.tools()) {
            (Ok(plan), Some(tools)) => command_preview(&plan, tools.ffmpeg()),
            _ => Vec::new(),
        };
    }

    // ----- update

    /// Reacts to a message.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Next | Message::Back | Message::GoTo(_) => self.navigate(&message),
            Message::SystemTheme(mode) => {
                self.system_mode = mode;
                Task::none()
            }
            Message::ToolsLocated(_)
            | Message::EncodersDetected(_)
            | Message::ChooseFfmpegFolder
            | Message::FfmpegFolderChosen(_)
            | Message::Theme(_) => self.update_environment(message),
            Message::Browse(_)
            | Message::FileChosen(..)
            | Message::FileDropped(_)
            | Message::Clear(_)
            | Message::VideoLoaded(_)
            | Message::OsdLoaded(_)
            | Message::SrtLoaded(_)
            | Message::FontLoaded(_)
            | Message::TelemetryReady(..) => self.update_files(message),
            Message::DrawOsd(_)
            | Message::DrawSrt(_)
            | Message::MakeSound(_)
            | Message::Osd(_)
            | Message::Srt(_)
            | Message::Sync(_)
            | Message::Sound(_)
            | Message::Output(_)
            | Message::BrowseOutput
            | Message::OutputChosen(_) => self.update_options(message),
            Message::PreviewTime(_)
            | Message::PreviewTimeReleased
            | Message::PreviewReady(..)
            | Message::PreviewHover(_)
            | Message::PreviewExited
            | Message::PreviewPressed => self.update_preview(message),
            Message::PlayAudio
            | Message::AudioRendered(_)
            | Message::StopAudio
            | Message::AudioTick => self.update_audio(message),
            Message::StartRender
            | Message::CancelRender
            | Message::Render(_)
            | Message::OpenOutputFolder
            | Message::CopyCommand => self.update_render(message),
            Message::DismissNotice(index) => {
                if index < self.notices.len() {
                    self.notices.remove(index);
                }
                Task::none()
            }
            Message::CloseRequested => {
                self.save_settings();
                if let RenderState::Running(running) = &self.render {
                    running.cancel.cancel();
                }
                iced::exit()
            }
        }
    }

    fn navigate(&mut self, message: &Message) -> Task<Message> {
        let target = match message {
            Message::Next => self.step.next(),
            Message::Back => self.step.previous(),
            Message::GoTo(step) => Some(*step),
            _ => None,
        };
        let Some(target) = target else {
            return Task::none();
        };
        if self.is_rendering() || !wizard::can_go_to(self.step, target, &self.context()) {
            return Task::none();
        }
        self.step = target;
        self.save_settings();
        if matches!(target, Step::Overlay | Step::Srt | Step::Sound)
            && self.preview.handle.is_none()
        {
            return self.refresh_preview_if_open();
        }
        if target == Step::Render {
            self.refresh_command_lines();
        }
        Task::none()
    }

    fn update_environment(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToolsLocated(Ok(tools)) => {
                self.tools = ToolsState::Found(Arc::clone(&tools));
                let mut tasks = vec![tasks::detect_encoders(tools)];
                if let Some(path) = Self::path_from_arguments() {
                    tasks.push(self.open_dropped(path));
                }
                if let Some(path) = self.settings.font_path.clone()
                    && path.is_file()
                {
                    tasks.push(self.open_file(FileKind::Font, path));
                }
                Task::batch(tasks)
            }
            Message::ToolsLocated(Err(error)) => {
                self.tools = ToolsState::Missing(error);
                Task::none()
            }
            Message::EncodersDetected(list) => {
                // Keep the encoder that was chosen if it works. Otherwise take the first one that does.
                let works = |name: &str| list.iter().any(|e| e.name == name && e.works);
                if !works(&self.settings.output.encoder)
                    && let Some(first) = list.iter().find(|e| e.works)
                {
                    self.settings.output.encoder.clone_from(&first.name);
                }
                self.encoders = Some(list);
                Task::none()
            }
            Message::ChooseFfmpegFolder => {
                tasks::pick_folder("Choose the folder of ffmpeg and ffprobe")
            }
            Message::FfmpegFolderChosen(Some(dir)) => {
                self.settings.ffmpeg_dir = Some(dir.clone());
                self.tools = ToolsState::Searching;
                self.save_settings();
                tasks::locate_tools(Some(dir))
            }
            Message::Theme(theme) => {
                self.settings.theme = theme;
                self.save_settings();
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn update_files(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Browse(kind) => {
                let start = match kind {
                    FileKind::Video => self.settings.last_video_dir.clone(),
                    _ => self
                        .video
                        .as_ref()
                        .and_then(|v| v.path.parent().map(Path::to_path_buf)),
                };
                tasks::pick_file(kind, start)
            }
            Message::FileChosen(kind, Some(path)) => self.open_file(kind, path),
            Message::FileDropped(path) => self.open_dropped(path),
            Message::Clear(kind) => self.clear_file(kind),
            Message::VideoLoaded(result) => self.video_loaded(result),
            Message::OsdLoaded(result) => {
                self.busy.osd = false;
                match result {
                    Ok(osd) => {
                        self.telemetry = None;
                        self.busy.telemetry = true;
                        let task = tasks::decode_telemetry(osd.path.clone(), Arc::clone(&osd.file));
                        self.osd = Some(osd);
                        self.set_file_error(FileKind::Osd, None);
                        Task::batch([task, self.refresh_preview_if_open()])
                    }
                    Err(error) => {
                        self.set_file_error(FileKind::Osd, Some(error));
                        Task::none()
                    }
                }
            }
            Message::SrtLoaded(result) => {
                self.busy.srt = false;
                match result {
                    Ok(srt) => {
                        self.srt = Some(srt);
                        self.set_file_error(FileKind::Srt, None);
                        self.refresh_preview_if_open()
                    }
                    Err(error) => {
                        self.set_file_error(FileKind::Srt, Some(error));
                        Task::none()
                    }
                }
            }
            Message::FontLoaded(result) => {
                self.busy.font = false;
                match result {
                    Ok(font) => {
                        self.settings.font_path = Some(font.path.clone());
                        self.font = Some(font);
                        self.rebuild_scaled_font();
                        self.set_file_error(FileKind::Font, None);
                        self.refresh_preview_if_open()
                    }
                    Err(error) => {
                        self.set_file_error(FileKind::Font, Some(error));
                        Task::none()
                    }
                }
            }
            Message::TelemetryReady(path, result) => {
                if self.osd.as_ref().is_none_or(|o| o.path != path) {
                    return Task::none(); // the file was replaced while it was read
                }
                self.busy.telemetry = false;
                match result {
                    Ok(recording) => {
                        self.telemetry = Some(Telemetry {
                            recording,
                            signals: Err(String::new()),
                        });
                        self.recompute_signals();
                    }
                    Err(error) => self
                        .notices
                        .push(format!("The telemetry could not be read: {error}")),
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn video_loaded(&mut self, result: Result<Arc<LoadedVideo>, String>) -> Task<Message> {
        self.busy.video = false;
        let video = match result {
            Ok(video) => video,
            Err(error) => {
                self.set_file_error(FileKind::Video, Some(error));
                return Task::none();
            }
        };
        // A new video starts over: what belonged to the old one is dropped.
        let replaced = self.video.is_some();
        self.osd = None;
        self.srt = None;
        self.telemetry = None;
        self.preview = Preview::default();
        self.file_errors.clear();
        if replaced {
            self.notices.push(
                "A new video was opened: its .osd and .srt were looked for again.".to_owned(),
            );
        }
        self.settings.last_video_dir = video.path.parent().map(Path::to_path_buf);
        self.output_path = OutputPlan::default_path(&video.path, false)
            .display()
            .to_string();
        self.stretch.enabled = false;
        self.render = RenderState::Idle;

        let companions = discover::companions(&video.path);
        self.video = Some(video);
        self.rebuild_scaled_font();

        let mut tasks = Vec::new();
        match companions.osd {
            Some(path) => tasks.push(self.open_file(FileKind::Osd, path)),
            None => self.settings.draw_osd = false,
        }
        match companions.srt {
            Some(path) => tasks.push(self.open_file(FileKind::Srt, path)),
            None => self.settings.draw_srt = false,
        }
        tasks.push(self.refresh_preview());
        Task::batch(tasks)
    }

    fn update_options(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DrawOsd(on) => {
                self.settings.draw_osd = on;
                self.refresh_preview_if_open()
            }
            Message::DrawSrt(on) => {
                self.settings.draw_srt = on;
                self.refresh_preview_if_open()
            }
            Message::MakeSound(on) => {
                self.settings.make_sound = on;
                Task::none()
            }
            Message::Osd(edit) => self.edit_osd(edit),
            Message::Srt(edit) => {
                let srt = &mut self.settings.srt;
                match edit {
                    SrtEdit::Time(v) => srt.show_time = v,
                    SrtEdit::SkyBattery(v) => srt.show_sky_battery = v,
                    SrtEdit::GroundBattery(v) => srt.show_ground_battery = v,
                    SrtEdit::Signal(v) => srt.show_signal = v,
                    SrtEdit::Latency(v) => srt.show_latency = v,
                    SrtEdit::Bitrate(v) => srt.show_bitrate = v,
                    SrtEdit::Distance(v) => srt.show_distance = v,
                    SrtEdit::PositionX(v) => srt.position_percent.0 = v,
                    SrtEdit::PositionY(v) => srt.position_percent.1 = v,
                    SrtEdit::Scale(v) => srt.scale = v,
                }
                self.refresh_preview_if_open()
            }
            Message::Sync(edit) => {
                match edit {
                    SyncEdit::Scale(choice) => self.scale_choice = choice,
                    SyncEdit::ManualScale(text) => self.manual_scale = text,
                    SyncEdit::Offset(text) => self.offset = text,
                }
                self.apply_sync_text();
                self.refresh_preview_if_open()
            }
            Message::Sound(edit) => self.edit_sound(edit),
            Message::Output(edit) => {
                self.edit_output(edit);
                Task::none()
            }
            Message::BrowseOutput => tasks::pick_output(PathBuf::from(self.output_path.trim())),
            Message::OutputChosen(Some(path)) => {
                self.output_path = path.display().to_string();
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn edit_osd(&mut self, edit: OsdEdit) -> Task<Message> {
        match edit {
            OsdEdit::X(x) => self.settings.osd.x_offset = x,
            OsdEdit::Y(y) => self.settings.osd.y_offset = y,
            OsdEdit::Reset => {
                self.settings.osd.x_offset = 0;
                self.settings.osd.y_offset = 0;
            }
            OsdEdit::Center => {
                if let (Some(video), Some(osd), Some(font), Some(timeline)) =
                    (&self.video, &self.osd, &self.scaled_font, self.timeline())
                {
                    let ms = timeline.telemetry_time(self.preview.time_s) * 1000.0;
                    let frame = &osd.file.frames[osd.file.frame_index_at(ms)];
                    self.settings.osd.center_on(
                        frame,
                        video.info.width,
                        video.info.height,
                        &font.size(),
                    );
                }
            }
            OsdEdit::MaskMode(on) => self.preview.mask_mode = on,
            OsdEdit::ClearMasks => self.settings.osd.masked_cells.clear(),
        }
        self.refresh_preview_if_open()
    }

    fn edit_sound(&mut self, edit: SoundEdit) -> Task<Message> {
        match edit {
            SoundEdit::Param(param, value) => {
                sound_params::set(&mut self.settings.sound, param, value);
            }
            SoundEdit::SampleRate(rate) => self.settings.sound.audio.sample_rate = rate,
            SoundEdit::Stereo(stereo) => {
                self.settings.sound.audio.channels = if stereo { 2 } else { 1 }
            }
            SoundEdit::Preset(name) => {
                if let Some(preset) = self.settings.presets.get(&name) {
                    self.settings.sound = preset.clone();
                }
                self.selected_preset = Some(name.clone());
                self.preset_name = name;
            }
            SoundEdit::PresetName(name) => self.preset_name = name,
            SoundEdit::SavePreset => {
                let name = self.preset_name.trim().to_owned();
                if !name.is_empty() {
                    self.settings
                        .presets
                        .insert(name.clone(), self.settings.sound.clone());
                    self.selected_preset = Some(name);
                }
            }
            SoundEdit::DeletePreset => {
                if let Some(name) = self.selected_preset.take() {
                    self.settings.presets.remove(&name);
                }
            }
            SoundEdit::ResetToDefaults => {
                self.settings.sound = flight_replay_core::synth::SoundSettings::default();
                self.selected_preset = None;
            }
        }
        self.recompute_signals();
        Task::none()
    }

    fn edit_output(&mut self, edit: OutputEdit) {
        let output = &mut self.settings.output;
        match edit {
            OutputEdit::Encoder(name) => output.encoder = name,
            OutputEdit::Bitrate(v) => output.bitrate_mbps = v,
            OutputEdit::Upscale(v) => output.upscale_1440p = v,
            OutputEdit::ChromaKey(v) => output.chroma_key = v,
            OutputEdit::AudioBitrate(v) => output.audio_bitrate_kbps = v,
            OutputEdit::Faststart(v) => output.faststart = v,
            OutputEdit::ShowUndetected(v) => output.show_undetected_encoders = v,
            OutputEdit::Path(text) => self.output_path = text,
            OutputEdit::Overwrite(v) => self.overwrite = v,
            OutputEdit::ReplaceAudio(v) => self.replace_audio = v,
            OutputEdit::Stretch(v) => self.stretch.enabled = v,
            OutputEdit::StretchStart(text) => self.stretch.start = text,
            OutputEdit::StretchDuration(text) => self.stretch.duration = text,
        }
    }

    fn update_preview(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PreviewTime(t) => {
                self.preview.time_s = t;
                Task::none()
            }
            Message::PreviewTimeReleased => self.refresh_preview_if_open(),
            Message::PreviewReady(sequence, result) => {
                if sequence != self.preview.sequence {
                    return Task::none(); // a newer picture was asked for
                }
                self.preview.busy = false;
                match result {
                    Ok(made) => {
                        let shown = &made.shown;
                        self.preview.shown = (shown.width(), shown.height());
                        self.preview.handle = Some(iced::widget::image::Handle::from_rgba(
                            shown.width(),
                            shown.height(),
                            shown.as_raw().clone(),
                        ));
                        self.preview.base = Some((made.base.0, Arc::clone(&made.base.1)));
                        self.preview.error = None;
                    }
                    Err(error) => self.preview.error = Some(error),
                }
                Task::none()
            }
            Message::PreviewHover(point) => {
                self.preview.hover = Some(point);
                Task::none()
            }
            Message::PreviewExited => {
                self.preview.hover = None;
                Task::none()
            }
            Message::PreviewPressed => {
                if self.preview.mask_mode
                    && let Some((x, y)) = self.cell_under_pointer()
                {
                    self.settings.osd.toggle_mask(x, y);
                    return self.refresh_preview();
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn update_audio(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PlayAudio => {
                self.audio = None;
                self.audio_playing = false;
                self.audio_error = None;
                let (Some(signals), Some(timeline)) = (self.signals(), self.timeline()) else {
                    return Task::none();
                };
                self.busy.audio = true;
                tasks::render_audio_preview(
                    self.settings.sound.clone(),
                    signals,
                    timeline,
                    self.preview.time_s,
                    10.0,
                )
            }
            Message::AudioRendered(result) => {
                self.busy.audio = false;
                match result {
                    Ok(pcm) => {
                        let audio = &self.settings.sound.audio;
                        match AudioPlayer::play(&pcm, audio.sample_rate, audio.channels) {
                            Ok(player) => {
                                self.audio = Some(player);
                                self.audio_playing = true;
                                return tasks::tick_after(300);
                            }
                            Err(error) => self.audio_error = Some(error),
                        }
                    }
                    Err(error) => self.audio_error = Some(error),
                }
                Task::none()
            }
            Message::StopAudio => {
                self.audio = None;
                self.audio_playing = false;
                Task::none()
            }
            Message::AudioTick => {
                if self.audio.as_ref().is_some_and(AudioPlayer::is_finished) {
                    self.audio = None;
                    self.audio_playing = false;
                }
                if self.audio.is_some() {
                    tasks::tick_after(300)
                } else {
                    Task::none()
                }
            }
            _ => Task::none(),
        }
    }

    fn update_render(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StartRender => {
                let (Some(tools), plan) = (self.tools(), self.build_plan()) else {
                    return Task::none();
                };
                match plan {
                    Ok(plan) => {
                        self.save_settings();
                        let cancel = CancelToken::new();
                        self.render = RenderState::Running(Running::new(cancel.clone()));
                        tasks::start_render(tools, plan, cancel)
                    }
                    Err(problems) => {
                        self.render = RenderState::Finished(Err(problems.join("\n")));
                        Task::none()
                    }
                }
            }
            Message::CancelRender => {
                if let RenderState::Running(running) = &mut self.render {
                    running.cancel.cancel();
                    running.cancelling = true;
                }
                Task::none()
            }
            Message::Render(message) => {
                self.render_message(message);
                Task::none()
            }
            Message::OpenOutputFolder => {
                if let RenderState::Finished(Ok(outcome)) = &self.render {
                    tasks::open_folder(&outcome.output);
                }
                Task::none()
            }
            Message::CopyCommand => iced::clipboard::write(self.command_lines.join("\n")),
            _ => Task::none(),
        }
    }

    fn render_message(&mut self, message: RenderMessage) {
        match message {
            RenderMessage::Event(event) => {
                let RenderState::Running(running) = &mut self.render else {
                    return;
                };
                match event {
                    Event::Stage(stage) => running.stage = stage,
                    Event::Progress { stage, done, total } => match stage {
                        Stage::Audio => running.audio = (done, total),
                        Stage::Video => running.video = (done, total),
                        _ => {}
                    },
                    Event::Warning(text) => running.warnings.push(text),
                }
            }
            RenderMessage::Done(result) => {
                self.render = RenderState::Finished(
                    result
                        .map(|outcome| Box::new((*outcome).clone()))
                        .map_err(|error| error.clone()),
                );
            }
        }
    }
}
