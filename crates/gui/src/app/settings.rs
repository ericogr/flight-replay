// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The settings screen: opening and closing it, and installing FFmpeg from it.
//!
//! The install only starts when the user clicks the button for it. It runs on its own thread (see
//! [`tasks::install_ffmpeg`]) and reports back with the number of the install, so that what a cancelled install says
//! while it winds down is not mistaken for the one that was started after it.

use std::path::{Path, PathBuf};

use flight_replay_core::cancel::CancelToken;
use flight_replay_core::ffmpeg::Tools;
use flight_replay_core::ffmpeg::install::{Build, InstallError, remove_installed};
use flight_replay_core::paths;
use iced::Task;

use super::{FlightReplay, ToolsState};
use crate::message::{InstallMessage, Message};
use crate::state::{DiskSpace, InstallRun, InstallState};
use crate::tasks;

/// The page of the project, from the manifest.
const PROJECT_PAGE: &str = env!("CARGO_PKG_REPOSITORY");

impl FlightReplay {
    pub(super) fn open_settings(&mut self) -> Task<Message> {
        self.settings_screen.open = true;
        self.refresh_environment();
        Task::none()
    }

    pub(super) fn close_settings(&mut self) -> Task<Message> {
        self.settings_screen.open = false;
        // A question left open is not waiting when the user comes back.
        self.confirming_reset = false;
        Task::none()
    }

    /// Starts the install of FFmpeg, if none is going on and no render is.
    pub(super) fn install_ffmpeg(&mut self) -> Task<Message> {
        if self.is_rendering() || matches!(self.install, InstallState::Running(_)) {
            return Task::none();
        }
        let Some(build) = Build::for_this_system() else {
            self.install = InstallState::Failed(InstallError::Unsupported.to_string());
            return Task::none();
        };
        self.install_runs += 1;
        let cancel = CancelToken::new();
        self.install = InstallState::Running(InstallRun {
            id: self.install_runs,
            cancel: cancel.clone(),
            progress: None,
        });
        tasks::install_ffmpeg(self.install_runs, build, cancel)
    }

    /// Stops the install at once, as far as the screen goes: it winds down by itself, and nothing it says after this
    /// is taken in.
    pub(super) fn cancel_install(&mut self) -> Task<Message> {
        if let InstallState::Running(run) = &self.install {
            run.cancel.cancel();
            self.install = InstallState::Idle;
        }
        Task::none()
    }

    pub(super) fn install_message(&mut self, id: u64, message: InstallMessage) -> Task<Message> {
        let InstallState::Running(run) = &mut self.install else {
            return Task::none();
        };
        if run.id != id {
            return Task::none();
        }
        match message {
            InstallMessage::Progress(progress) => {
                run.progress = Some(progress);
                Task::none()
            }
            InstallMessage::Done(Ok(())) => {
                self.install = InstallState::Idle;
                self.refresh_environment();
                self.notices.push("FFmpeg was installed.".to_owned());
                self.look_for_ffmpeg()
            }
            InstallMessage::Done(Err(reason)) => {
                self.install = InstallState::Failed(reason);
                self.refresh_environment();
                Task::none()
            }
        }
    }

    /// Deletes the copy of FFmpeg that the application installed, and looks again for the one to use.
    pub(super) fn remove_installed_ffmpeg(&mut self) -> Task<Message> {
        if self.is_rendering() || matches!(self.install, InstallState::Running(_)) {
            return Task::none();
        }
        if let Err(error) = remove_installed() {
            self.notices.push(format!(
                "Could not remove the FFmpeg that was installed: {error}"
            ));
        }
        self.refresh_environment();
        self.look_for_ffmpeg()
    }

    pub(super) fn recheck_ffmpeg(&mut self) -> Task<Message> {
        if self.is_rendering() {
            return Task::none();
        }
        self.look_for_ffmpeg()
    }

    pub(super) fn open_settings_folder(&mut self) -> Task<Message> {
        if let Some(folder) = self.settings_folder() {
            tasks::open_folder(&folder);
        }
        Task::none()
    }

    pub(super) fn open_project_page() -> Task<Message> {
        if let Err(error) = opener::open_browser(PROJECT_PAGE) {
            tracing::warn!(%error, "could not open the page of the project");
        }
        Task::none()
    }

    /// Searches for FFmpeg again, in the folder the user chose and then the usual places.
    fn look_for_ffmpeg(&mut self) -> Task<Message> {
        self.tools = ToolsState::Searching;
        self.encoders = None;
        tasks::locate_tools(self.settings.ffmpeg_dir.clone())
    }

    /// The folder of the settings file, if the settings are kept.
    pub fn settings_folder(&self) -> Option<PathBuf> {
        self.file
            .path()
            .and_then(paths::parent_folder)
            .map(Path::to_path_buf)
    }

    /// The file of the settings, if they are kept.
    pub fn settings_file(&self) -> Option<PathBuf> {
        self.file.path().map(Path::to_path_buf)
    }

    /// Reads again what the screens show about the machine: the free space where the output goes, and whether
    /// there is a copy of FFmpeg that the application installed.
    pub(super) fn refresh_environment(&mut self) {
        self.disk = self.disk_folder().and_then(|folder| {
            paths::free_space(&folder).map(|free_bytes| DiskSpace { folder, free_bytes })
        });
        self.managed_installed = Tools::managed_dir().is_some_and(|dir| dir.is_dir());
    }

    /// The folder whose free space matters: where the output will be written, else where the last video was, else
    /// the home folder.
    fn disk_folder(&self) -> Option<PathBuf> {
        let output = self.output_path.trim();
        if !output.is_empty() {
            return Some(paths::folder_or_current(Path::new(output)).to_path_buf());
        }
        self.preferences
            .last_video_dir
            .clone()
            .or_else(paths::home_folder)
    }
}
