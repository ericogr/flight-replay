// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The settings file: everything the user chose on the screen is written as soon as it changes.
//!
//! What is written is [`Settings`] (what a render needs) and [`Preferences`] (what the window remembers). Both are
//! compared with what the file holds after every message, so no handler has to remember to save, and a choice made
//! just before the program is killed is not lost.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use flight_replay_core::config::Settings;
use iced::Task;

use super::FlightReplay;
use crate::message::Message;
use crate::preferences::{self, Preferences};
use crate::tasks;

/// The least time between two writes. Dragging a slider makes many edits a second: the first is written at once, the
/// others a moment later, in one write that has the last of them.
const MIN_GAP: Duration = Duration::from_millis(300);

/// Where the settings are written, and what the file holds.
pub(super) struct SettingsFile {
    /// `None` when the settings are not kept, as in a test that asks for no file.
    path: Option<PathBuf>,
    /// What was written last, or read from the file when the program started.
    written: (Settings, Preferences),
    /// When it was written last.
    at: Option<Instant>,
    /// A write is waiting for the gap since the last one to pass.
    waiting: bool,
    /// The last write failed and the user was told, so the next failure is not told again.
    failing: bool,
}

impl SettingsFile {
    /// The file at `path`, which holds `settings` and `preferences` now.
    pub(super) fn new(
        path: Option<PathBuf>,
        settings: &Settings,
        preferences: &Preferences,
    ) -> Self {
        Self {
            path,
            written: (settings.clone(), preferences.clone()),
            at: None,
            waiting: false,
            failing: false,
        }
    }

    /// Where the settings are written, if they are kept.
    pub(super) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

impl FlightReplay {
    /// Whether what is chosen now is not what the file holds.
    fn has_unsaved_choices(&self) -> bool {
        self.file.written.0 != self.settings || self.file.written.1 != self.preferences
    }

    /// Writes the settings and the preferences now. A write that fails is not tried again until something changes, so
    /// a full disk does not make every message try it, and the user is told once.
    fn save_settings(&mut self) {
        let Some(path) = &self.file.path else {
            return;
        };
        self.file.written = (self.settings.clone(), self.preferences.clone());
        self.file.at = Some(Instant::now());
        match preferences::save(path, &self.settings, &self.preferences) {
            Ok(()) => self.file.failing = false,
            Err(error) => {
                tracing::warn!(%error, "could not save the settings");
                if !self.file.failing {
                    self.notices
                        .push(format!("The settings could not be saved: {error}"));
                }
                self.file.failing = true;
            }
        }
    }

    /// Writes what is chosen, if the file does not hold it yet.
    pub(super) fn flush_settings(&mut self) {
        if self.has_unsaved_choices() {
            self.save_settings();
        }
    }

    /// Writes what changed. It is written at once if the last write was a while ago, and otherwise when the gap has
    /// passed. Called after every message.
    pub(super) fn save_when_changed(&mut self) -> Option<Task<Message>> {
        if self.file.path.is_none() || self.file.waiting || !self.has_unsaved_choices() {
            return None;
        }
        let gap_left = self
            .file
            .at
            .and_then(|at| MIN_GAP.checked_sub(at.elapsed()));
        let Some(gap_left) = gap_left else {
            self.save_settings();
            return None;
        };
        self.file.waiting = true;
        Some(tasks::after(
            u64::try_from(gap_left.as_millis()).unwrap_or(u64::MAX),
            Message::SaveSettings,
        ))
    }

    /// The gap has passed: writes what is chosen now.
    pub(super) fn settings_save_due(&mut self) {
        self.file.waiting = false;
        self.flush_settings();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(path: &std::path::Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn the_first_change_is_written_at_once_and_a_burst_after_it_in_one_later_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let mut app = FlightReplay::new(Some(path.clone()));

        // Nothing changed: nothing is written.
        assert!(app.save_when_changed().is_none());
        assert_eq!(saved(&path), "");

        // The first change does not wait.
        app.preferences.show_command = true;
        assert!(app.save_when_changed().is_none());
        assert!(saved(&path).contains("show_command = true"));

        // The ones that follow at once wait for the gap, and are written together.
        app.preferences.show_other_layers = false;
        assert!(app.save_when_changed().is_some(), "a write is waiting");
        app.settings.output.bitrate_mbps = 23;
        assert!(
            app.save_when_changed().is_none(),
            "one is already waiting; the others go in it"
        );
        assert!(!saved(&path).contains("show_other_layers = false"));
        app.settings_save_due();
        let text = saved(&path);
        assert!(text.contains("show_other_layers = false"), "{text}");
        assert!(text.contains("bitrate_mbps = 23"), "{text}");

        // Written and unchanged since: nothing more to write.
        assert!(app.save_when_changed().is_none());
    }

    #[test]
    fn a_file_that_cannot_be_written_is_said_once_until_it_can() {
        let dir = tempfile::tempdir().unwrap();
        // The folder of the file is a file, so the settings cannot be written there.
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "not a folder").unwrap();
        let mut app = FlightReplay::new(Some(blocker.join("settings.toml")));
        let told = |app: &FlightReplay| {
            app.notices
                .iter()
                .filter(|notice| notice.starts_with("The settings could not be saved"))
                .count()
        };

        app.preferences.show_command = true;
        assert!(app.save_when_changed().is_none());
        assert_eq!(told(&app), 1, "{:?}", app.notices);

        // Another change tries again, and does not say it again.
        app.preferences.show_command = false;
        app.settings_save_due();
        assert_eq!(told(&app), 1);

        // Once the write works the next failure is said again.
        std::fs::remove_file(&blocker).unwrap();
        app.preferences.show_command = true;
        app.settings_save_due();
        assert!(saved(&blocker.join("settings.toml")).contains("show_command = true"));
        std::fs::remove_dir_all(&blocker).unwrap();
        std::fs::write(&blocker, "not a folder again").unwrap();
        app.preferences.show_command = false;
        app.settings_save_due();
        assert_eq!(told(&app), 2);
    }

    #[test]
    fn nothing_is_written_when_the_settings_have_no_file() {
        let mut app = FlightReplay::new(None);
        app.preferences.show_command = true;
        assert!(app.save_when_changed().is_none());
    }
}
