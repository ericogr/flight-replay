// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! What the window remembers between runs, besides the [`Settings`] of the core.
//!
//! These are the choices of the screens that the core does not know: the colour scheme, the options of the previews,
//! and what a render is given on each run rather than as a setting (the timing of the OSD, the stretch, whether to
//! replace a file). Both are kept in the one `settings.toml`: the preferences are keys at the top of the file, and the
//! settings of the render (see [`flight_replay_core::config`]) follow, so a file written by an older version is still
//! read.

use std::path::{Path, PathBuf};

use flight_replay_core::config::{self, Settings, SettingsError};
use serde::{Deserialize, Serialize};

use crate::state::{StretchInput, SyncInput};

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

impl ThemeChoice {
    /// Every choice, in the order the buttons show them.
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    /// The name of the choice, on its button and in the summary.
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

/// What the window remembers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one switch for each choice of the screens that the window remembers"
)]
pub struct Preferences {
    /// The colour scheme.
    pub theme: ThemeChoice,
    /// The folder the last video was opened from.
    pub last_video_dir: Option<PathBuf>,
    /// Show the FFmpeg command on the last step, before the render. Off until the user asks for it.
    pub show_command: bool,
    /// The preview of a step also shows the layers of the other steps (the OSD in the step of the SRT data, the SRT
    /// data in the step of the OSD, both in the step of the sound), as the render will draw them. On until the user
    /// switches it off; then each step shows only its own layer, and the step of the sound none.
    pub show_other_layers: bool,
    /// The saved sound that was chosen in the list of the sound step. It stays chosen while its values are tweaked, so
    /// that it can be saved over or deleted, and it is dropped when another kind of motor or the defaults are chosen.
    /// It always names a sound that is in the settings (see [`load`]).
    pub selected_preset: Option<String>,
    /// How the OSD is timed against the video, as typed.
    pub sync: SyncInput,
    /// The start and the length of the stretch to render, as typed. Whether a stretch is asked for is not kept.
    pub stretch: StretchInput,
    /// Replace the output file if it exists.
    pub overwrite: bool,
    /// Replace the sound of a video that already has some.
    pub replace_audio: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::default(),
            last_video_dir: None,
            show_command: false,
            show_other_layers: true,
            selected_preset: None,
            sync: SyncInput::default(),
            stretch: StretchInput::default(),
            overwrite: false,
            replace_audio: false,
        }
    }
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
/// What is read is made to agree with itself: a saved sound that was chosen but is no longer in the file (it was
/// edited by hand) is not left chosen.
pub fn load(path: &Path) -> (Settings, Preferences, Option<String>) {
    let (stored, notice) = config::load_or_default::<Stored>(path);
    let (settings, mut preferences) = (stored.settings, stored.preferences);
    if preferences
        .selected_preset
        .as_ref()
        .is_some_and(|name| !settings.presets.contains_key(name))
    {
        preferences.selected_preset = None;
    }
    (settings, preferences, notice)
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
    use crate::state::ScaleChoice;

    #[test]
    fn the_preferences_and_the_settings_share_one_file_and_survive_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let mut settings = Settings::default();
        settings.output.bitrate_mbps = 22;
        settings
            .presets
            .insert("Loud".to_owned(), settings.sound.clone());
        // Every value is the opposite of the default, so that a key that is not kept shows.
        let preferences = Preferences {
            theme: ThemeChoice::Dark,
            last_video_dir: Some(PathBuf::from("/flights")),
            show_command: true,
            show_other_layers: false,
            selected_preset: Some("Loud".to_owned()),
            sync: SyncInput {
                choice: ScaleChoice::Manual,
                manual_scale: "1,02".to_owned(),
                offset: "2,5".to_owned(),
            },
            stretch: StretchInput {
                start: "12".to_owned(),
                duration: "45".to_owned(),
            },
            overwrite: true,
            replace_audio: true,
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
        assert!(
            preferences.show_other_layers,
            "a file without the key shows every layer"
        );
    }

    #[test]
    fn a_saved_sound_that_is_chosen_must_be_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let mut settings = Settings::default();
        settings
            .presets
            .insert("Kept".to_owned(), settings.sound.clone());
        let mut preferences = Preferences {
            selected_preset: Some("Kept".to_owned()),
            ..Preferences::default()
        };
        save(&path, &settings, &preferences).unwrap();
        assert_eq!(load(&path).1.selected_preset.as_deref(), Some("Kept"));

        // The saved sound was taken out of the file by hand: it cannot stay chosen.
        preferences.selected_preset = Some("Gone".to_owned());
        save(&path, &settings, &preferences).unwrap();
        assert_eq!(load(&path).1.selected_preset, None);
    }

    #[test]
    fn the_command_is_hidden_until_the_user_asks_for_it() {
        assert!(!Preferences::default().show_command);
    }
}
