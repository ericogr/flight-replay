// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Opening, replacing and clearing the video and the files that go with it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use flight_replay_core::discover;
use flight_replay_core::font::BundledFont;
use flight_replay_core::paths::parent_folder;
use flight_replay_core::render::OutputPlan;
use flight_replay_core::telemetry::Recording;
use iced::Task;

use super::FlightReplay;
use crate::message::Message;
use crate::state::{
    FileKind, FontSource, LoadedFont, LoadedOsd, LoadedSrt, LoadedVideo, Preview, RenderState,
    Telemetry,
};
use crate::tasks;

impl FlightReplay {
    /// The folder of the video that is open.
    fn video_folder(&self) -> Option<PathBuf> {
        let video = self.video.as_ref()?;
        parent_folder(&video.path).map(Path::to_path_buf)
    }

    /// Asks the user for a file of this kind, starting where the video is.
    pub(super) fn browse(&mut self, kind: FileKind) -> Task<Message> {
        let start = match kind {
            FileKind::Video => self.preferences.last_video_dir.clone(),
            _ => self.video_folder(),
        };
        tasks::pick_file(kind, start, move |path| Message::FileChosen(kind, path))
    }

    /// Starts opening a file. What it gives arrives as a message when it has been read.
    pub(super) fn open_file(&mut self, kind: FileKind, path: PathBuf) -> Task<Message> {
        self.set_file_error(kind, None);
        match kind {
            FileKind::Video => {
                let Some(tools) = self.tools() else {
                    return Task::none();
                };
                self.busy.video = true;
                tasks::load_video(tools, path)
            }
            FileKind::Osd => self.reload_osd(vec![path]),
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
    pub(super) fn open_dropped(&mut self, path: PathBuf) -> Task<Message> {
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
        if matches!(kind, FileKind::Osd | FileKind::Srt) && self.video.is_none() {
            self.notices
                .push("Open the video first, then the files that go with it".to_owned());
            return Task::none();
        }
        self.open_file(kind, path)
    }

    /// Takes a file out, and what only made sense with it.
    pub(super) fn clear_file(&mut self, kind: FileKind) -> Task<Message> {
        self.set_file_error(kind, None);
        match kind {
            FileKind::Video => {
                self.video = None;
                self.osd = None;
                self.srt = None;
                self.telemetry = None;
                self.preview = Preview::default();
            }
            // What the user asked for stays asked for: what the files cannot give is left out of the render for now
            // (see `FlightReplay::availability`), and comes back with the file.
            FileKind::Osd => {
                self.osd = None;
                self.telemetry = None;
            }
            FileKind::Srt => self.srt = None,
            FileKind::Font => {
                self.font = None;
                self.settings.font_path = None;
                self.settings.bundled_font = None;
                self.scaled_font = None;
            }
        }
        self.refresh_preview_if_open()
    }

    // ----- the .osd files of a video made of several recordings

    /// The `.osd` files that are open, in the order they play.
    fn osd_paths(&self) -> Vec<PathBuf> {
        self.osd
            .as_ref()
            .map(|o| o.paths.clone())
            .unwrap_or_default()
    }

    /// Opens these `.osd` files, as one recording.
    fn reload_osd(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        self.set_file_error(FileKind::Osd, None);
        self.busy.osd = true;
        tasks::load_osd(paths)
    }

    /// Asks the user for the `.osd` file of the next recording.
    pub(super) fn browse_more_osd(&mut self) -> Task<Message> {
        tasks::pick_file(FileKind::Osd, self.video_folder(), Message::OsdAdded)
    }

    pub(super) fn add_osd(&mut self, path: PathBuf) -> Task<Message> {
        let mut paths = self.osd_paths();
        if paths.contains(&path) {
            self.notices
                .push("That .osd file is already in the list.".to_owned());
            return Task::none();
        }
        paths.push(path);
        self.reload_osd(paths)
    }

    pub(super) fn remove_osd(&mut self, index: usize) -> Task<Message> {
        let mut paths = self.osd_paths();
        if index < paths.len() {
            paths.remove(index);
        }
        if paths.is_empty() {
            self.clear_file(FileKind::Osd)
        } else {
            self.reload_osd(paths)
        }
    }

    /// Plays the file at `index` before the one in front of it.
    pub(super) fn move_osd_up(&mut self, index: usize) -> Task<Message> {
        let mut paths = self.osd_paths();
        if index == 0 || index >= paths.len() {
            return Task::none();
        }
        paths.swap(index - 1, index);
        self.reload_osd(paths)
    }

    // ----- what the loading tasks answer

    /// A new video starts over: what belonged to the old one is dropped, and the files next to the new one are
    /// opened.
    pub(super) fn video_loaded(
        &mut self,
        result: Result<Arc<LoadedVideo>, String>,
    ) -> Task<Message> {
        self.busy.video = false;
        let video = match result {
            Ok(video) => video,
            Err(error) => {
                self.set_file_error(FileKind::Video, Some(error));
                return Task::none();
            }
        };
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
        self.preferences.last_video_dir = parent_folder(&video.path).map(Path::to_path_buf);
        self.output_path = OutputPlan::default_path(&video.path, false)
            .display()
            .to_string();
        self.stretch.enabled = false;
        self.render = RenderState::Idle;

        let companions = discover::companions(&video.path);
        self.video = Some(video);
        self.rebuild_scaled_font();

        let mut tasks = Vec::new();
        if let Some(path) = companions.osd {
            tasks.push(self.open_file(FileKind::Osd, path));
        }
        if let Some(path) = companions.srt {
            tasks.push(self.open_file(FileKind::Srt, path));
        }
        tasks.push(self.refresh_preview());
        Task::batch(tasks)
    }

    pub(super) fn osd_loaded(&mut self, result: Result<Arc<LoadedOsd>, String>) -> Task<Message> {
        self.busy.osd = false;
        match result {
            Ok(osd) => {
                self.telemetry = None;
                self.busy.telemetry = true;
                let decoding =
                    tasks::decode_telemetry(osd.path().to_path_buf(), Arc::clone(&osd.file));
                self.osd = Some(osd);
                self.set_file_error(FileKind::Osd, None);
                Task::batch([decoding, self.refresh_preview_if_open()])
            }
            Err(error) => {
                self.set_file_error(FileKind::Osd, Some(error));
                Task::none()
            }
        }
    }

    pub(super) fn srt_loaded(&mut self, result: Result<Arc<LoadedSrt>, String>) -> Task<Message> {
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

    pub(super) fn font_loaded(&mut self, result: Result<Arc<LoadedFont>, String>) -> Task<Message> {
        self.busy.font = false;
        match result {
            Ok(font) => {
                // What is remembered is the file or the font of the program, never both.
                match &font.source {
                    FontSource::File(path) => {
                        self.settings.font_path = Some(path.clone());
                        self.settings.bundled_font = None;
                    }
                    FontSource::Bundled(bundled) => {
                        self.settings.bundled_font = Some(bundled.id.to_owned());
                        self.settings.font_path = None;
                    }
                }
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

    /// Starts reading a font that comes with the program.
    pub(super) fn use_bundled_font(&mut self, font: &'static BundledFont) -> Task<Message> {
        self.set_file_error(FileKind::Font, None);
        self.busy.font = true;
        tasks::load_bundled_font(font)
    }

    /// The telemetry of the `.osd` at `path`. It is dropped if that file was replaced while it was read.
    pub(super) fn telemetry_ready(
        &mut self,
        path: &Path,
        result: Result<Arc<Recording>, String>,
    ) -> Task<Message> {
        if self.osd.as_ref().is_none_or(|o| o.path() != path) {
            return Task::none();
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
}
