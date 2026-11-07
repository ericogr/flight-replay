// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The application: its state, and how it reacts to each message.
//!
//! [`FlightReplay::update`] is one flat table from message to handler, so the compiler checks that every message is
//! handled. The handlers are in the modules of this folder, one for each part of the work:
//!
//! - `files`: opening, replacing and clearing the video and the files that go with it
//! - `options`: what to draw, how to encode, and the other choices of the steps
//! - `picture`: the preview picture
//! - `persist`: writing the settings file whenever a choice changes
//! - `play`: the preview in motion, with the sound or without it
//! - `rendering`: the plan of the render, and running it
//! - `queue`: several videos, one after the other

mod files;
mod options;
mod persist;
mod picture;
mod play;
mod queue;
mod rendering;

use std::path::PathBuf;
use std::sync::Arc;

use flight_replay_core::config::{self, Settings};
use flight_replay_core::ffmpeg::{EncoderInfo, Tools};
use flight_replay_core::font::{BundledFont, ScaledFont};
use flight_replay_core::render::Availability;
use flight_replay_core::synth::SignalSet;
use flight_replay_core::timeline::Timeline;
use iced::{Subscription, Task, Theme};

use crate::message::Message;
use crate::preferences::{self, Preferences, ThemeChoice};
use crate::state::{
    FileKind, LoadedFont, LoadedOsd, LoadedSrt, LoadedVideo, Preview, Queue, RenderState, Telemetry,
};
use crate::tasks;
use crate::wizard::{self, Context, Step};
use persist::SettingsFile;
use play::Playing;

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

/// The application.
pub struct FlightReplay {
    /// What a render needs.
    pub settings: Settings,
    /// What the window remembers.
    pub preferences: Preferences,
    system_mode: iced::theme::Mode,
    file: SettingsFile,
    pub notices: Vec<String>,
    /// The user asked to reset every setting and has not answered the question yet. It is never remembered.
    pub confirming_reset: bool,

    pub tools: ToolsState,
    pub encoders: Option<Arc<Vec<EncoderInfo>>>,

    pub step: Step,
    pub video: Option<Arc<LoadedVideo>>,
    pub osd: Option<Arc<LoadedOsd>>,
    pub srt: Option<Arc<LoadedSrt>>,
    pub font: Option<Arc<LoadedFont>>,
    /// The font at the size of the video, made when the font and the video are both open.
    scaled_font: Option<Arc<ScaledFont>>,
    pub telemetry: Option<Telemetry>,
    pub busy: Busy,
    pub file_errors: Vec<(FileKind, String)>,

    pub preview: Preview,

    /// The name typed to save the sound under.
    pub preset_name: String,
    /// The preview that is playing: the clock, the picture, and the sound if the step has one.
    playing: Option<Playing>,
    /// Why the preview could not play.
    pub play_error: Option<String>,

    pub output_path: String,
    /// Render only a stretch of the video, a quick test of the whole render. It is not remembered: it is off whenever
    /// the program or a video is opened, so that nothing is rendered short by mistake. The start and the length of the
    /// stretch are remembered, in the preferences.
    pub stretch_on: bool,

    pub render: RenderState,
    pub queue: Queue,
    pub command_lines: Vec<String>,
}

impl FlightReplay {
    /// Starts the application: reads the settings and looks for FFmpeg.
    pub fn boot() -> (Self, Task<Message>) {
        let app = Self::new(config::default_path());
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
        let (settings, preferences, notice) = settings_path.as_deref().map_or_else(
            || (Settings::default(), Preferences::default(), None),
            preferences::load,
        );
        let file = SettingsFile::new(settings_path, &settings, &preferences);
        Self {
            preset_name: preferences.selected_preset.clone().unwrap_or_default(),
            settings,
            preferences,
            system_mode: iced::theme::Mode::None,
            file,
            notices: notice.into_iter().collect(),
            confirming_reset: false,
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
            preview: Preview::default(),
            playing: None,
            play_error: None,
            output_path: String::new(),
            stretch_on: false,
            render: RenderState::Idle,
            queue: Queue::default(),
            command_lines: Vec::new(),
        }
    }

    /// A file given on the command line, such as the video that the system opens with this application.
    fn path_from_arguments() -> Option<PathBuf> {
        std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            // The settings and the output name keep this path, so it must not depend on the folder we run from.
            .map(|p| std::path::absolute(&p).unwrap_or(p))
    }

    /// The colour scheme.
    pub fn theme(&self) -> Theme {
        match self.preferences.theme {
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

    // ----- what the views and the wizard need to know

    /// The tools, if they were found.
    pub fn tools(&self) -> Option<Arc<Tools>> {
        match &self.tools {
            ToolsState::Found(tools) => Some(Arc::clone(tools)),
            _ => None,
        }
    }

    /// The facts the wizard rules work from.
    pub fn context(&self) -> Context {
        Context {
            tools_found: self.tools().is_some(),
            video: self.video.is_some(),
            draw_osd: self.draw_osd_on(),
            draw_srt: self.draw_srt_on(),
            make_sound: self.sound_on(),
            output_path: !self.output_path.trim().is_empty(),
            encoder: !self.settings.output.encoder.is_empty(),
        }
    }

    /// What the files at hand allow, apart from what the telemetry says: why the OSD, the SRT data and the sound
    /// cannot be made, if they cannot.
    pub fn files_allow(&self) -> Availability {
        Availability::new(
            self.osd.as_ref().map(|o| &*o.file),
            self.font.is_some(),
            self.srt.is_some(),
        )
    }

    /// What the files and the telemetry allow. What is not allowed is left out of the render, and the steps say why.
    pub fn availability(&self) -> Availability {
        self.files_allow().with_telemetry(self.telemetry_problem())
    }

    /// Why the telemetry cannot drive the sound yet, if it cannot: it is being read, it could not be read, or it
    /// lacks what the sound needs.
    fn telemetry_problem(&self) -> Option<String> {
        self.osd.as_ref()?;
        match &self.telemetry {
            Some(Telemetry {
                signals: Err(error),
                ..
            }) if !error.is_empty() => Some(error.clone()),
            Some(Telemetry { signals: Ok(_), .. }) => None,
            _ if self.busy.telemetry => Some("The telemetry is being read.".to_owned()),
            _ => Some("The telemetry of the .osd file could not be read.".to_owned()),
        }
    }

    /// Whether the OSD is drawn: it is wanted, and there are an `.osd` file and a font.
    pub fn draw_osd_on(&self) -> bool {
        self.settings.draw_osd && self.availability().osd.is_none()
    }

    /// Whether the SRT data are drawn: they are wanted, and there is an `.srt` file.
    pub fn draw_srt_on(&self) -> bool {
        self.settings.draw_srt && self.availability().srt.is_none()
    }

    /// Whether the sound is made: it is wanted, and the `.osd` file gives what it needs.
    pub fn sound_on(&self) -> bool {
        self.settings.make_sound && self.availability().sound.is_none()
    }

    /// The mapping between video time and telemetry time.
    pub fn timeline(&self) -> Option<Timeline> {
        let (video, osd) = (self.video.as_ref()?, self.osd.as_ref()?);
        Some(Timeline::new(
            self.preferences.sync.options(),
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

    /// What went wrong when the file of this kind was opened.
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

    /// Whether a render is running.
    pub fn is_rendering(&self) -> bool {
        matches!(self.render, RenderState::Running(_))
    }

    // ----- what is remembered and what is derived

    fn rebuild_scaled_font(&mut self) {
        self.scaled_font = match (&self.font, &self.video) {
            (Some(font), Some(video)) => Some(Arc::new(font.font.scaled_for(video.info.height))),
            _ => None,
        };
    }

    /// Works out again what the sound can follow, after the telemetry or the sound settings changed.
    fn recompute_signals(&mut self) {
        let (Some(osd), Some(telemetry)) = (&self.osd, &mut self.telemetry) else {
            return;
        };
        let name = flight_replay_core::paths::file_name(osd.path());
        telemetry.signals = SignalSet::collect(
            &telemetry.recording.table,
            &self.settings.sound,
            &name,
            telemetry.recording.frames,
        )
        .map(Arc::new)
        .map_err(|e| e.to_string());
    }

    // ----- the messages

    /// Reacts to a message, and writes the settings if the message changed them.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.react(message);
        match self.save_when_changed() {
            Some(save) => Task::batch([task, save]),
            None => task,
        }
    }

    /// What each message does.
    #[allow(
        clippy::too_many_lines,
        reason = "one short arm for each message: a flat table is what lets the compiler check that all are handled"
    )]
    fn react(&mut self, message: Message) -> Task<Message> {
        match message {
            // moving between the steps
            Message::Next => self.go_to(self.step.next()),
            Message::Back => self.go_to(self.step.previous()),
            Message::GoTo(step) => self.go_to(Some(step)),

            // the environment
            Message::ToolsLocated(result) => self.tools_located(result),
            Message::EncodersDetected(list) => self.encoders_detected(list),
            Message::ChooseFfmpegFolder => tasks::pick_folder(
                "Choose the folder of ffmpeg and ffprobe",
                Message::FfmpegFolderChosen,
            ),
            Message::FfmpegFolderChosen(dir) => {
                dir.map_or_else(Task::none, |dir| self.ffmpeg_folder_chosen(dir))
            }
            Message::Theme(theme) => self.choose_theme(theme),
            Message::SystemTheme(mode) => {
                self.system_mode = mode;
                Task::none()
            }
            Message::DismissNotice(index) => {
                if index < self.notices.len() {
                    self.notices.remove(index);
                }
                Task::none()
            }
            Message::CloseRequested => self.close_requested(),
            Message::AskResetAll => self.ask_reset_all(),
            Message::CancelResetAll => {
                self.confirming_reset = false;
                Task::none()
            }
            Message::ResetAll => self.reset_all(),
            Message::SaveSettings => {
                self.settings_save_due();
                Task::none()
            }

            // the files
            Message::Browse(kind) => self.browse(kind),
            Message::FileChosen(kind, path) => {
                path.map_or_else(Task::none, |path| self.open_file(kind, path))
            }
            Message::FileDropped(path) => self.open_dropped(path),
            Message::Clear(kind) => self.clear_file(kind),
            Message::UseBundledFont(font) => self.use_bundled_font(font),
            Message::VideoLoaded(result) => self.video_loaded(result),
            Message::OsdLoaded(result) => self.osd_loaded(result),
            Message::SrtLoaded(result) => self.srt_loaded(result),
            Message::FontLoaded(result) => self.font_loaded(result),
            Message::TelemetryReady(path, result) => self.telemetry_ready(&path, result),
            Message::AddOsd => self.browse_more_osd(),
            Message::OsdAdded(path) => path.map_or_else(Task::none, |path| self.add_osd(path)),
            Message::RemoveOsdFile(index) => self.remove_osd(index),
            Message::MoveOsdFileUp(index) => self.move_osd_up(index),

            // what to draw and how
            Message::DrawOsd(on) => self.set_draw_osd(on),
            Message::DrawSrt(on) => self.set_draw_srt(on),
            Message::MakeSound(on) => {
                self.settings.make_sound = on;
                Task::none()
            }
            Message::Osd(edit) => self.edit_osd(edit),
            Message::Srt(edit) => self.edit_srt(edit),
            Message::Sync(edit) => self.edit_sync(edit),
            Message::Sound(edit) => self.edit_sound(edit),
            Message::Output(edit) => self.edit_output(edit),
            Message::BrowseOutput => self.browse_output(),
            Message::OutputChosen(path) => {
                if let Some(path) = path {
                    self.output_path = path.display().to_string();
                }
                Task::none()
            }

            // the preview picture
            Message::ShowOtherLayers(on) => self.set_show_other_layers(on),
            Message::PreviewTime(seconds) => self.set_preview_time(seconds),
            Message::PreviewTimeReleased => self.refresh_preview_if_open(),
            Message::PreviewReady(sequence, result) => self.preview_ready(sequence, result),
            Message::PreviewHover(point) => {
                self.preview.hover = Some(point);
                Task::none()
            }
            Message::PreviewExited => {
                self.preview.hover = None;
                Task::none()
            }
            Message::PreviewPressed => self.preview_pressed(),

            // the preview in motion
            Message::PlayAudio => self.play_audio(),
            Message::AudioRendered(result) => self.audio_rendered(result),
            Message::PlayPicture => self.play_picture(),
            Message::StopPlay => self.stop_play(),
            Message::PlayTick(play) => self.play_tick(play),
            Message::PlayFrame(stream, result) => self.play_frame(stream, result),
            Message::RestartPlayback { play, edit } => self.playback_restart_due(play, edit),

            // the queue
            Message::QueueAdd => self.queue_add(),
            Message::QueueAddFolder => tasks::pick_folder(
                "Choose the folder with the videos",
                Message::QueueFolderChosen,
            ),
            Message::QueueFolderChosen(folder) => {
                folder.map_or_else(Task::none, |folder| self.queue_plan_folder(folder))
            }
            Message::QueueFolderPlanned(result) => {
                self.queue_folder_planned(result);
                Task::none()
            }
            Message::QueueRemove(index) => {
                self.queue_remove(index);
                Task::none()
            }
            Message::QueueClearFinished => {
                self.queue_clear_finished();
                Task::none()
            }
            Message::QueueStart => self.queue_start(),

            // the render
            Message::StartRender => self.start_render(),
            Message::CancelRender => {
                self.cancel_render();
                Task::none()
            }
            Message::Render(message) => self.render_message(message),
            Message::OpenOutputFolder => {
                self.open_output_folder();
                Task::none()
            }
            Message::ShowCommand(on) => self.set_show_command(on),
            Message::CopyCommand => iced::clipboard::write(self.command_lines.join("\n")),
        }
    }

    /// Moves to a step, if the wizard allows it.
    fn go_to(&mut self, target: Option<Step>) -> Task<Message> {
        let Some(target) = target else {
            return Task::none();
        };
        if self.is_rendering() || !wizard::can_go_to(self.step, target, &self.context()) {
            return Task::none();
        }
        let leaving = target != self.step;
        self.step = target;
        // What plays belongs to the step it was started in.
        let stopped = if leaving {
            // A question left open on the welcome step is not waiting when the user comes back.
            self.confirming_reset = false;
            self.play_error = None;
            self.stop_play()
        } else {
            Task::none()
        };
        // The picture is made again when there is none, or when this step draws other layers than the last one did.
        if target.has_preview()
            && (self.preview.handle.is_none() || self.preview.layers != self.layers_wanted())
        {
            return Task::batch([stopped, self.refresh_preview_if_open()]);
        }
        if target == Step::Render {
            self.refresh_command_lines();
        }
        stopped
    }

    fn tools_located(&mut self, result: Result<Arc<Tools>, String>) -> Task<Message> {
        match result {
            Ok(tools) => {
                self.tools = ToolsState::Found(Arc::clone(&tools));
                let mut tasks = vec![tasks::detect_encoders(tools)];
                if let Some(path) = Self::path_from_arguments() {
                    tasks.push(self.open_dropped(path));
                }
                // The font used last: one of the program's, or a file that is still there.
                if let Some(font) = self
                    .settings
                    .bundled_font
                    .as_deref()
                    .and_then(BundledFont::find)
                {
                    tasks.push(self.use_bundled_font(font));
                } else if let Some(path) = self.settings.font_path.clone()
                    && path.is_file()
                {
                    tasks.push(self.open_file(FileKind::Font, path));
                }
                Task::batch(tasks)
            }
            Err(error) => {
                self.tools = ToolsState::Missing(error);
                Task::none()
            }
        }
    }

    /// Keeps the encoder that was chosen if it works. Otherwise takes the first one that does.
    fn encoders_detected(&mut self, list: Arc<Vec<EncoderInfo>>) -> Task<Message> {
        let works = |name: &str| list.iter().any(|e| e.name == name && e.works);
        if !works(&self.settings.output.encoder)
            && let Some(first) = list.iter().find(|e| e.works)
        {
            self.settings.output.encoder.clone_from(&first.name);
        }
        self.encoders = Some(list);
        Task::none()
    }

    fn ffmpeg_folder_chosen(&mut self, dir: PathBuf) -> Task<Message> {
        self.settings.ffmpeg_dir = Some(dir.clone());
        self.tools = ToolsState::Searching;
        tasks::locate_tools(Some(dir))
    }

    fn choose_theme(&mut self, theme: ThemeChoice) -> Task<Message> {
        self.preferences.theme = theme;
        Task::none()
    }

    fn set_show_other_layers(&mut self, on: bool) -> Task<Message> {
        self.preferences.show_other_layers = on;
        self.refresh_preview_if_open()
    }

    fn set_show_command(&mut self, on: bool) -> Task<Message> {
        self.preferences.show_command = on;
        Task::none()
    }

    /// Writes what is remembered, stops a render that is running, and closes the window.
    fn close_requested(&mut self) -> Task<Message> {
        self.flush_settings();
        if let RenderState::Running(running) = &self.render {
            running.cancel.cancel();
        }
        iced::exit()
    }
}
