// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Opening, replacing and clearing the videos of the flight and the files that go with each.
//!
//! A flight is one video, or several that follow one another (the goggles split a long recording into files). What
//! the user chose is `FlightReplay::clips`: each video with its `.osd` and `.srt`. Whenever it changes, the files of
//! all the videos are read again and put on one timeline (`FlightReplay::osd`, `FlightReplay::srt`), so the rest of
//! the program sees the flight as one video with one recording.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use flight_replay_core::discover;
use flight_replay_core::font::BundledFont;
use flight_replay_core::footage::{Footage, FootageError, VideoPart};
use flight_replay_core::paths::{file_name, parent_folder};
use flight_replay_core::render::OutputPlan;
use flight_replay_core::telemetry::Recording;
use iced::Task;

use super::FlightReplay;
use crate::message::Message;
use crate::state::{
    Clip, FileKind, FontSource, LoadedFont, LoadedOsd, LoadedSrt, Placing, Preview, RenderState,
    Telemetry,
};
use crate::tasks;

/// How long, in milliseconds, the program waits for the other videos that were dropped together with one. They arrive
/// one after another, within the same moment.
const DROP_SETTLE_MS: u64 = 50;

/// The videos of a flight on one timeline.
fn footage_of(clips: &[Clip]) -> Result<Footage, FootageError> {
    Footage::new(clips.iter().map(|clip| clip.video.clone()).collect())
}

impl FlightReplay {
    /// The folder of the first video.
    fn video_folder(&self) -> Option<PathBuf> {
        let clip = self.clips.first()?;
        parent_folder(&clip.video.path).map(Path::to_path_buf)
    }

    /// Asks the user for a file of this kind, starting where the video is. For the videos, the user may choose several:
    /// they are the flight.
    pub(super) fn browse(&mut self, kind: FileKind) -> Task<Message> {
        match kind {
            FileKind::Video => {
                let start = self.preferences.last_video_dir.clone();
                self.ask(move || tasks::pick_videos(start, Message::VideosChosen))
            }
            FileKind::Osd | FileKind::Srt => self.browse_clip_file(kind, 0),
            FileKind::Font => {
                let start = self.video_folder();
                self.ask(move || {
                    tasks::pick_file(kind, start, move |path| Message::FileChosen(kind, path))
                })
            }
        }
    }

    /// Asks the user for the videos that follow the ones that are open.
    pub(super) fn browse_more_videos(&mut self) -> Task<Message> {
        let start = self
            .video_folder()
            .or_else(|| self.preferences.last_video_dir.clone());
        self.ask(move || tasks::pick_videos(start, Message::VideosAdded))
    }

    /// Asks the user for the `.osd` or the `.srt` of the video at this place.
    pub(super) fn browse_clip_file(&mut self, kind: FileKind, clip: usize) -> Task<Message> {
        let start = self
            .clips
            .get(clip)
            .and_then(|clip| parent_folder(&clip.video.path))
            .map(Path::to_path_buf);
        self.ask(move || {
            tasks::pick_file(kind, start, move |path| {
                Message::ClipFileChosen(kind, clip, path)
            })
        })
    }

    /// Starts opening a file. What it gives arrives as a message when it has been read. A video is the flight, and an
    /// `.osd` or an `.srt` is the file of the first video.
    pub(super) fn open_file(&mut self, kind: FileKind, path: PathBuf) -> Task<Message> {
        match kind {
            FileKind::Video => self.open_videos(vec![path], Placing::Replace),
            FileKind::Osd | FileKind::Srt => self.set_clip_file(kind, 0, path),
            FileKind::Font => {
                self.set_file_error(kind, None);
                self.busy.font = true;
                tasks::load_font(path)
            }
        }
    }

    /// Starts opening videos.
    pub(super) fn open_videos(&mut self, paths: Vec<PathBuf>, placing: Placing) -> Task<Message> {
        let Some(tools) = self.tools() else {
            return Task::none();
        };
        if paths.is_empty() {
            return Task::none();
        }
        self.set_file_error(FileKind::Video, None);
        self.busy.video = true;
        tasks::load_videos(tools, paths, placing)
    }

    /// Opens a file that was dropped on the window, by what it looks like. Videos dropped together are one flight, so
    /// a video waits a moment for the ones that come with it.
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
        match kind {
            FileKind::Video => {
                let first = self.dropped_videos.is_empty();
                self.dropped_videos.push(path);
                if first {
                    tasks::after(DROP_SETTLE_MS, Message::DropsSettled)
                } else {
                    Task::none()
                }
            }
            FileKind::Osd | FileKind::Srt => self.open_dropped_companion(kind, path),
            FileKind::Font => self.open_file(kind, path),
        }
    }

    /// The videos dropped together have all arrived: they are the flight, in the order of their names.
    pub(super) fn drops_settled(&mut self) -> Task<Message> {
        let mut paths = std::mem::take(&mut self.dropped_videos);
        paths.sort();
        paths.dedup();
        self.open_videos(paths, Placing::Replace)
    }

    /// An `.osd` or an `.srt` that was dropped goes with the video it is named after. A flight of one video takes any.
    fn open_dropped_companion(&mut self, kind: FileKind, path: PathBuf) -> Task<Message> {
        let named_after = |clip: &Clip| {
            let found = discover::companions(&clip.video.path);
            let found = if kind == FileKind::Osd {
                found.osd
            } else {
                found.srt
            };
            found.as_deref() == Some(path.as_path())
        };
        let clip = match (self.clips.iter().position(named_after), self.clips.len()) {
            (Some(index), _) => index,
            (None, 1) => 0,
            (None, 0) => {
                self.notices
                    .push("Open the video first, then the files that go with it".to_owned());
                return Task::none();
            }
            (None, _) => {
                self.notices.push(format!(
                    "{} is not named after any of the videos. Use the button next to the video it goes with.",
                    file_name(&path)
                ));
                return Task::none();
            }
        };
        self.set_clip_file(kind, clip, path)
    }

    /// Takes a file out, and what only made sense with it. A video is the whole flight, and an `.osd` or an `.srt` is
    /// the file of the first video.
    pub(super) fn clear_file(&mut self, kind: FileKind) -> Task<Message> {
        self.set_file_error(kind, None);
        match kind {
            FileKind::Video => self.clear_flight(),
            FileKind::Osd | FileKind::Srt => self.clear_clip_file(kind, 0),
            // What the user asked for stays asked for: what the files cannot give is left out of the render for now
            // (see `FlightReplay::availability`), and comes back with the file.
            FileKind::Font => {
                self.font = None;
                self.settings.font_path = None;
                self.settings.bundled_font = None;
                self.scaled_font = None;
                self.refresh_preview_if_open()
            }
        }
    }

    // ----- the videos of the flight

    /// Closes every video, and what was worked out from them.
    fn clear_flight(&mut self) -> Task<Message> {
        self.cancel_play();
        self.clips.clear();
        self.footage = None;
        self.drop_files_read();
        self.preview = Preview::default();
        self.rebuild_scaled_font();
        Task::none()
    }

    /// Forgets the `.osd` and `.srt` files that were read, and what was read from them. A read that is going on is
    /// overtaken.
    fn drop_files_read(&mut self) {
        self.osd = None;
        self.srt = None;
        self.telemetry = None;
        self.osd_load += 1;
        self.srt_load += 1;
        self.busy.osd = false;
        self.busy.srt = false;
        self.set_file_error(FileKind::Osd, None);
        self.set_file_error(FileKind::Srt, None);
    }

    /// Takes the video at this place out of the flight.
    pub(super) fn remove_video(&mut self, index: usize) -> Task<Message> {
        if index >= self.clips.len() {
            return Task::none();
        }
        if self.clips.len() == 1 {
            return self.clear_file(FileKind::Video);
        }
        let mut clips = self.clips.clone();
        clips.remove(index);
        self.use_clips(clips)
    }

    /// Plays the video at this place before the one in front of it.
    pub(super) fn move_video_up(&mut self, index: usize) -> Task<Message> {
        if index == 0 || index >= self.clips.len() {
            return Task::none();
        }
        let mut clips = self.clips.clone();
        clips.swap(index - 1, index);
        self.use_clips(clips)
    }

    /// The videos that were opened: they are the flight, or they follow the videos that are open.
    pub(super) fn videos_loaded(
        &mut self,
        placing: Placing,
        result: Result<Vec<VideoPart>, String>,
    ) -> Task<Message> {
        self.busy.video = false;
        let opened = match result {
            Ok(opened) => opened,
            Err(error) => {
                self.set_file_error(FileKind::Video, Some(error));
                return Task::none();
            }
        };
        let mut clips = match placing {
            Placing::Replace => Vec::new(),
            Placing::Append => self.clips.clone(),
        };
        for video in opened {
            if clips.iter().any(|clip| clip.video.path == video.path) {
                self.notices.push(format!(
                    "{} is in the flight already.",
                    file_name(&video.path)
                ));
                continue;
            }
            let found = discover::companions(&video.path);
            self.preferences.last_video_dir = parent_folder(&video.path).map(Path::to_path_buf);
            clips.push(Clip {
                video,
                osd: found.osd.into_iter().collect(),
                srt: found.srt,
            });
        }
        if clips.is_empty() {
            return Task::none();
        }
        if clips.len() > 1 && clips.iter().any(|clip| clip.osd.len() > 1) {
            self.set_file_error(
                FileKind::Video,
                Some(
                    "A video made of several recordings cannot be joined to other videos: take its extra .osd files out first."
                        .to_owned(),
                ),
            );
            return Task::none();
        }
        if placing == Placing::Replace && !self.clips.is_empty() {
            self.notices.push(
                "A new video was opened: its .osd and .srt were looked for again.".to_owned(),
            );
        }
        self.use_clips(clips)
    }

    /// Puts these videos in place of the ones in the flight, and reads the files of all of them again. Videos that
    /// are not alike (a picture of another size) are refused, and nothing changes.
    fn use_clips(&mut self, clips: Vec<Clip>) -> Task<Message> {
        let footage = match footage_of(&clips) {
            Ok(footage) => Arc::new(footage),
            Err(error) => {
                self.set_file_error(FileKind::Video, Some(error.to_string()));
                return Task::none();
            }
        };
        let first_changed = self.clips.first().map(|clip| &clip.video.path)
            != clips.first().map(|clip| &clip.video.path);
        self.cancel_play();
        self.clips = clips;
        self.footage = Some(footage);
        self.drop_files_read();
        self.file_errors.clear();
        self.preview = Preview::default();
        self.stretch_on = false;
        self.render = RenderState::Idle;
        self.rebuild_scaled_font();
        if first_changed {
            self.output_path = OutputPlan::default_path(&self.clips[0].video.path, false)
                .display()
                .to_string();
        }
        Task::batch([
            self.reload_osd(),
            self.reload_srt(),
            self.refresh_preview_if_open(),
        ])
    }

    // ----- the .osd and .srt files of each video

    /// Takes this file as the `.osd` or the `.srt` of the video at this place.
    pub(super) fn set_clip_file(
        &mut self,
        kind: FileKind,
        clip: usize,
        path: PathBuf,
    ) -> Task<Message> {
        let Some(entry) = self.clips.get_mut(clip) else {
            return Task::none();
        };
        match kind {
            FileKind::Osd => entry.osd = vec![path],
            FileKind::Srt => entry.srt = Some(path),
            FileKind::Video | FileKind::Font => return Task::none(),
        }
        self.set_file_error(kind, None);
        self.files_changed(kind)
    }

    /// Takes out the `.osd` or the `.srt` of the video at this place.
    pub(super) fn clear_clip_file(&mut self, kind: FileKind, clip: usize) -> Task<Message> {
        let Some(entry) = self.clips.get_mut(clip) else {
            return Task::none();
        };
        match kind {
            FileKind::Osd => entry.osd.clear(),
            FileKind::Srt => entry.srt = None,
            FileKind::Video | FileKind::Font => return Task::none(),
        }
        self.set_file_error(kind, None);
        self.files_changed(kind)
    }

    /// The `.osd` files or the `.srt` files that were chosen changed: they are read again, and the picture follows.
    fn files_changed(&mut self, kind: FileKind) -> Task<Message> {
        let reading = if kind == FileKind::Osd {
            self.reload_osd()
        } else {
            self.reload_srt()
        };
        Task::batch([reading, self.refresh_preview_if_open()])
    }

    /// Reads the `.osd` files of all the videos, as one recording. They are read when every video has one: until
    /// then there is no recording, and the reasons name the videos that lack one.
    fn reload_osd(&mut self) -> Task<Message> {
        self.osd_load += 1;
        self.set_file_error(FileKind::Osd, None);
        let paths: Vec<PathBuf> = self
            .clips
            .iter()
            .flat_map(|clip| clip.osd.iter().cloned())
            .collect();
        if self.clips.is_empty() || self.clips.iter().any(|clip| clip.osd.is_empty()) {
            self.osd = None;
            self.telemetry = None;
            self.busy.osd = false;
            return Task::none();
        }
        self.busy.osd = true;
        tasks::load_osd(self.osd_load, paths)
    }

    /// Reads the `.srt` files of all the videos, as one. They are read when every video has one.
    fn reload_srt(&mut self) -> Task<Message> {
        self.srt_load += 1;
        self.set_file_error(FileKind::Srt, None);
        let files: Option<Vec<(PathBuf, f64)>> = self.footage.as_ref().and_then(|footage| {
            self.clips
                .iter()
                .enumerate()
                .map(|(index, clip)| Some((clip.srt.clone()?, footage.start_s(index))))
                .collect()
        });
        match files {
            Some(files) if !files.is_empty() => {
                self.busy.srt = true;
                tasks::load_srt(self.srt_load, files)
            }
            _ => {
                self.srt = None;
                self.busy.srt = false;
                Task::none()
            }
        }
    }

    // ----- the .osd files of a video made of several recordings

    /// The video is made of several recordings, and so has several `.osd` files, only when it is the only video.
    fn several_recordings(&self) -> bool {
        self.clips.len() == 1
    }

    /// Asks the user for the `.osd` file of the next recording.
    pub(super) fn browse_more_osd(&mut self) -> Task<Message> {
        let start = self.video_folder();
        self.ask(move || tasks::pick_file(FileKind::Osd, start, Message::OsdAdded))
    }

    pub(super) fn add_osd(&mut self, path: PathBuf) -> Task<Message> {
        if !self.several_recordings() {
            return Task::none();
        }
        if self.clips[0].osd.contains(&path) {
            self.notices
                .push("That .osd file is already in the list.".to_owned());
            return Task::none();
        }
        self.clips[0].osd.push(path);
        self.reload_osd()
    }

    pub(super) fn remove_osd(&mut self, index: usize) -> Task<Message> {
        if !self.several_recordings() {
            return Task::none();
        }
        if index < self.clips[0].osd.len() {
            self.clips[0].osd.remove(index);
        }
        self.files_changed(FileKind::Osd)
    }

    /// Plays the file at `index` before the one in front of it.
    pub(super) fn move_osd_up(&mut self, index: usize) -> Task<Message> {
        if !self.several_recordings() || index == 0 || index >= self.clips[0].osd.len() {
            return Task::none();
        }
        self.clips[0].osd.swap(index - 1, index);
        self.reload_osd()
    }

    // ----- what the loading tasks answer

    pub(super) fn osd_loaded(
        &mut self,
        load: u64,
        result: Result<Arc<LoadedOsd>, String>,
    ) -> Task<Message> {
        if load != self.osd_load {
            return Task::none();
        }
        self.busy.osd = false;
        match result {
            Ok(osd) => {
                self.telemetry = None;
                self.busy.telemetry = true;
                let decoding = tasks::decode_telemetry(load, Arc::clone(&osd.file));
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

    pub(super) fn srt_loaded(
        &mut self,
        load: u64,
        result: Result<Arc<LoadedSrt>, String>,
    ) -> Task<Message> {
        if load != self.srt_load {
            return Task::none();
        }
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

    /// The telemetry of the `.osd` files that the load with this number read. It is dropped if the files were read
    /// again meanwhile.
    pub(super) fn telemetry_ready(
        &mut self,
        load: u64,
        result: Result<Arc<Recording>, String>,
    ) -> Task<Message> {
        if load != self.osd_load || self.osd.is_none() {
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
