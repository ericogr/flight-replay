// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! What the window remembers between runs, besides the settings of a render.
//!
//! Both are kept in the one `settings.toml`: the preferences are keys at the top of the file, and the settings of the
//! render (see [`flight_replay_core::config`]) follow, so a file written by an older version is still read.

use std::path::{Path, PathBuf};

use flight_replay_core::config::{self, Settings, SettingsError};
use serde::{Deserialize, Serialize};

/// The colour scheme of the window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    /// Follow the system.
    #[default]
    System,
    /// A light scheme.
    Light,
    /// A dark scheme.
    Dark,
}

/// What the window remembers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// The colour scheme.
    pub theme: ThemeChoice,
    /// The folder the last video was opened from.
    pub last_video_dir: Option<PathBuf>,
    /// Show the FFmpeg command on the last step, before the render. Off until the user asks for it.
    pub show_command: bool,
}

/// The file as it is read: the preferences, then the settings of the render.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Stored {
    #[serde(flatten)]
    preferences: Preferences,
    #[serde(flatten)]
    settings: Settings,
}

/// The file as it is written. The preferences come first because they are plain values, and a table cannot be
/// followed by a plain value in TOML.
#[derive(Serialize)]
struct StoredRef<'a> {
    #[serde(flatten)]
    preferences: &'a Preferences,
    #[serde(flatten)]
    settings: &'a Settings,
}

/// Reads the file. When it cannot be used the defaults are given with a message, and the file is kept as `.broken`.
pub fn load(path: &Path) -> (Settings, Preferences, Option<String>) {
    let (stored, notice) = config::load_or_default::<Stored>(path);
    (stored.settings, stored.preferences, notice)
}

/// Writes the file.
pub fn save(
    path: &Path,
    settings: &Settings,
    preferences: &Preferences,
) -> Result<(), SettingsError> {
    config::save(
        &StoredRef {
            preferences,
            settings,
        },
        path,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preferences_and_the_settings_share_one_file_and_survive_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let mut settings = Settings::default();
        settings.output.bitrate_mbps = 22;
        let preferences = Preferences {
            theme: ThemeChoice::Dark,
            last_video_dir: Some(PathBuf::from("/flights")),
            show_command: true,
        };

        save(&path, &settings, &preferences).unwrap();
        let (read_settings, read_preferences, notice) = load(&path);

        assert_eq!(read_settings, settings);
        assert_eq!(read_preferences, preferences);
        assert_eq!(notice, None);
        // The render settings can still be read on their own, by something that knows nothing of the window.
        assert_eq!(
            config::load::<Settings>(&path).unwrap().output.bitrate_mbps,
            22
        );
    }

    #[test]
    fn a_file_from_before_the_split_is_still_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        std::fs::write(
            &path,
            "theme = \"light\"\nlast_video_dir = \"/old\"\ndraw_srt = true\n\n[output]\nbitrate_mbps = 12\n",
        )
        .unwrap();

        let (settings, preferences, _) = load(&path);
        assert_eq!(preferences.theme, ThemeChoice::Light);
        assert_eq!(preferences.last_video_dir, Some(PathBuf::from("/old")));
        assert!(settings.draw_srt);
        assert_eq!(settings.output.bitrate_mbps, 12);
        assert!(
            !preferences.show_command,
            "a file without the key does not show the command"
        );
    }

    #[test]
    fn the_command_is_hidden_until_the_user_asks_for_it() {
        assert!(!Preferences::default().show_command);
    }
}
