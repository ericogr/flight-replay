// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/config.rs (GPL-3.0-only), Copyright (C) Alexander van Saase, and
// fpv-motorsound, internal/config (Apache-2.0, Copyright 2026 EricoGR). Modified for Flight Replay: TOML instead of
// RON and YAML, an atomic write, and named sound presets.

//! The settings the application remembers between runs.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::osd::OsdOptions;
use crate::overlay::DEFAULT_CHROMA_KEY;
use crate::srt::SrtOptions;
use crate::synth::SoundSettings;

/// The colour scheme of the application.
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

/// How the video is encoded and the sound is stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one switch per option of the output step, stored as the user left it"
)]
pub struct OutputSettings {
    /// FFmpeg encoder name, such as `libx264`.
    pub encoder: String,
    /// Target video bitrate, in megabits per second.
    pub bitrate_mbps: u32,
    /// Scale the picture to 1440p.
    pub upscale_1440p: bool,
    /// Draw on a chroma-key background instead of the video.
    pub chroma_key: bool,
    /// The chroma-key colour.
    pub chroma_key_color: [u8; 3],
    /// AAC bitrate, in kilobits per second.
    pub audio_bitrate_kbps: u32,
    /// Move the MP4 index to the start of the file.
    pub faststart: bool,
    /// Offer the encoders that were not detected as working.
    pub show_undetected_encoders: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            encoder: "libx264".to_owned(),
            bitrate_mbps: 40,
            upscale_1440p: false,
            chroma_key: false,
            chroma_key_color: DEFAULT_CHROMA_KEY,
            audio_bitrate_kbps: 192,
            faststart: true,
            show_undetected_encoders: false,
        }
    }
}

/// Everything the application remembers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Folder of `ffmpeg` and `ffprobe`, when they are not on `PATH`.
    pub ffmpeg_dir: Option<PathBuf>,
    /// The colour scheme.
    pub theme: ThemeChoice,
    /// The folder the last video was opened from.
    pub last_video_dir: Option<PathBuf>,
    /// The OSD font file that was used last.
    pub font_path: Option<PathBuf>,
    /// Draw the OSD.
    pub draw_osd: bool,
    /// Draw the SRT data.
    pub draw_srt: bool,
    /// Make the sound.
    pub make_sound: bool,
    /// Position and masks of the OSD.
    pub osd: OsdOptions,
    /// The SRT fields.
    pub srt: SrtOptions,
    /// The sound parameters.
    pub sound: SoundSettings,
    /// The encoding of the output.
    pub output: OutputSettings,
    /// Sound presets saved by name.
    pub presets: BTreeMap<String, SoundSettings>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ffmpeg_dir: None,
            theme: ThemeChoice::default(),
            last_video_dir: None,
            font_path: None,
            draw_osd: true,
            draw_srt: false,
            make_sound: true,
            osd: OsdOptions::default(),
            srt: SrtOptions::default(),
            sound: SoundSettings::default(),
            output: OutputSettings::default(),
            presets: BTreeMap::new(),
        }
    }
}

/// Errors when reading or writing the settings.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// The file could not be read or written.
    #[error("{}: {source}", .path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// What went wrong.
        #[source]
        source: io::Error,
    },
    /// The file is not valid TOML for the settings.
    #[error("{}: {message}", .path.display())]
    Invalid {
        /// The file.
        path: PathBuf,
        /// What is wrong with it.
        message: String,
    },
}

impl Settings {
    /// The usual place of the settings file, in the configuration folder of the system.
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        ProjectDirs::from("io.github", "ericogr", "flight-replay")
            .map(|dirs| dirs.config_dir().join("settings.toml"))
    }

    /// Reads the settings. A file that does not exist gives the defaults. Keys that are missing keep their default.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(SettingsError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        toml::from_str(&text).map_err(|error| SettingsError::Invalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
    }

    /// Reads the settings, or gives the defaults and a message when the file cannot be used. A file that is not
    /// valid is kept next to the new one with the extension `.broken`, so nothing the user wrote is lost.
    #[must_use]
    pub fn load_or_default(path: &Path) -> (Self, Option<String>) {
        match Self::load(path) {
            Ok(settings) => (settings, None),
            Err(error) => {
                let _ = fs::rename(path, path.with_extension("toml.broken"));
                (
                    Self::default(),
                    Some(format!(
                        "the settings could not be read and were reset: {error}"
                    )),
                )
            }
        }
    }

    /// Writes the settings. The file is written to a temporary name and renamed, so a crash never leaves half of it.
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let io_error = |source| SettingsError::Io {
            path: path.to_path_buf(),
            source,
        };
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        let text = toml::to_string_pretty(self).map_err(|error| SettingsError::Invalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        let temporary = path.with_extension("toml.tmp");
        fs::write(&temporary, text).map_err(io_error)?;
        fs::rename(&temporary, path).map_err(io_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_survive_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config").join("settings.toml");
        let mut settings = Settings {
            theme: ThemeChoice::Dark,
            ..Settings::default()
        };
        settings.osd.x_offset = -12;
        settings.osd.toggle_mask(3, 4);
        settings.srt.show_time = true;
        settings.output.bitrate_mbps = 25;
        settings.sound.motor.f_max_hz = 600.0;
        settings
            .presets
            .insert("Mine".into(), settings.sound.clone());
        settings.font_path = Some(PathBuf::from("/fonts/WS_INAV_Europa_24.png"));

        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);
        assert!(
            !path.with_extension("toml.tmp").exists(),
            "no temporary file is left"
        );
    }

    #[test]
    fn a_missing_file_gives_the_defaults_and_missing_keys_keep_theirs() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Settings::load(&dir.path().join("none.toml")).unwrap(),
            Settings::default()
        );
        let path = dir.path().join("partial.toml");
        fs::write(&path, "theme = \"light\"\n\n[output]\nbitrate_mbps = 12\n").unwrap();
        let settings = Settings::load(&path).unwrap();
        assert_eq!(settings.theme, ThemeChoice::Light);
        assert_eq!(settings.output.bitrate_mbps, 12);
        assert_eq!(settings.output.encoder, "libx264");
        assert!(settings.draw_osd);
    }

    #[test]
    fn a_broken_file_is_kept_and_replaced_by_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        fs::write(&path, "this is = not [valid").unwrap();
        let (settings, message) = Settings::load_or_default(&path);
        assert_eq!(settings, Settings::default());
        assert!(message.unwrap().contains("reset"));
        assert!(path.with_extension("toml.broken").exists());
        assert!(!path.exists());
    }

    #[test]
    fn the_default_path_is_in_a_configuration_folder() {
        let path = Settings::default_path().expect("a home folder");
        assert_eq!(path.file_name().unwrap(), "settings.toml");
        assert!(path.to_string_lossy().contains("flight-replay"));
    }
}
