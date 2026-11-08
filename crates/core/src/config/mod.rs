// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/config.rs (GPL-3.0-only), Copyright (C) Alexander van Saase, and
// fpv-motorsound, internal/config (Apache-2.0, Copyright 2026 EricoGR). Modified for Flight Replay: TOML instead of
// RON and YAML, an atomic write, and named sound presets.

//! The settings that are remembered between runs, and how they are read and written.
//!
//! [`Settings`] is what a render needs, so every front-end shares it. A front-end that remembers more (the colour
//! scheme of a window, say) wraps it in a struct of its own with `#[serde(flatten)]` and reads and writes that with
//! [`load`] and [`save`], which work on any type.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::osd::OsdOptions;
use crate::overlay::DEFAULT_CHROMA_KEY;
use crate::paths::parent_folder;
use crate::srt::SrtOptions;
use crate::synth::SoundSettings;

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

/// What a render needs, and what the application remembers about it between runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Folder of `ffmpeg` and `ffprobe`, when they are not on `PATH`.
    pub ffmpeg_dir: Option<PathBuf>,
    /// The OSD font file that was used last.
    pub font_path: Option<PathBuf>,
    /// The id of the font that comes with the program, when that was used last (see
    /// [`crate::font::BUNDLED_FONTS`]). It and `font_path` are never both set.
    pub bundled_font: Option<String>,
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
            font_path: None,
            bundled_font: None,
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

/// The folders of the application in the profile of the user.
pub(crate) fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("io.github", "ericogr", "flight-replay")
}

/// The usual place of the settings file, in the configuration folder of the system.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.config_dir().join("settings.toml"))
}

/// Reads settings. A file that does not exist gives the defaults. Keys that are missing keep their default.
pub fn load<T: DeserializeOwned + Default>(path: &Path) -> Result<T, SettingsError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(T::default()),
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

/// Reads settings, or gives the defaults and a message when the file cannot be used. A file that is not valid is
/// kept next to the new one with the extension `.broken`, so nothing the user wrote is lost.
#[must_use]
pub fn load_or_default<T: DeserializeOwned + Default>(path: &Path) -> (T, Option<String>) {
    match load(path) {
        Ok(settings) => (settings, None),
        Err(error) => {
            let _ = fs::rename(path, path.with_extension("toml.broken"));
            (
                T::default(),
                Some(format!(
                    "the settings could not be read and were reset: {error}"
                )),
            )
        }
    }
}

/// Writes settings. The file is written to a temporary name and renamed, so a crash never leaves half of it.
pub fn save<T: Serialize>(settings: &T, path: &Path) -> Result<(), SettingsError> {
    let io_error = |source| SettingsError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = parent_folder(path) {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    let text = toml::to_string_pretty(settings).map_err(|error| SettingsError::Invalid {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, text).map_err(io_error)?;
    fs::rename(&temporary, path).map_err(io_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_survive_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config").join("settings.toml");
        let mut settings = Settings::default();
        settings.osd.x_offset = -12;
        settings.osd.toggle_mask(3, 4);
        settings.srt.show_time = true;
        settings.output.bitrate_mbps = 25;
        settings.sound.motor.f_max_hz = 600.0;
        settings
            .presets
            .insert("Mine".into(), settings.sound.clone());
        settings.font_path = Some(PathBuf::from("/fonts/WS_INAV_Europa_24.png"));

        save(&settings, &path).unwrap();
        assert_eq!(load::<Settings>(&path).unwrap(), settings);
        assert!(
            !path.with_extension("toml.tmp").exists(),
            "no temporary file is left"
        );

        // A font that comes with the program is kept by its id.
        settings.font_path = None;
        settings.bundled_font = Some("inav-default".into());
        save(&settings, &path).unwrap();
        assert_eq!(load::<Settings>(&path).unwrap(), settings);
    }

    #[test]
    fn a_missing_file_gives_the_defaults_and_missing_keys_keep_theirs() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load::<Settings>(&dir.path().join("none.toml")).unwrap(),
            Settings::default()
        );
        let path = dir.path().join("partial.toml");
        fs::write(&path, "draw_srt = true\n\n[output]\nbitrate_mbps = 12\n").unwrap();
        let settings = load::<Settings>(&path).unwrap();
        assert!(settings.draw_srt);
        assert_eq!(settings.output.bitrate_mbps, 12);
        assert_eq!(settings.output.encoder, "libx264");
        assert!(settings.draw_osd);
        assert_eq!(settings.bundled_font, None, "a file from before the key");
    }

    #[test]
    fn a_broken_file_is_kept_and_replaced_by_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        fs::write(&path, "this is = not [valid").unwrap();
        let (settings, message) = load_or_default::<Settings>(&path);
        assert_eq!(settings, Settings::default());
        assert!(message.unwrap().contains("reset"));
        assert!(path.with_extension("toml.broken").exists());
        assert!(!path.exists());
    }

    #[test]
    fn a_front_end_can_keep_more_in_the_same_file() {
        #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        struct Wrapped {
            colour: String,
            #[serde(flatten)]
            settings: Settings,
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let mut wrapped = Wrapped {
            colour: "dark".into(),
            ..Wrapped::default()
        };
        wrapped.settings.output.bitrate_mbps = 18;
        save(&wrapped, &path).unwrap();
        // The shared settings can still read the file, and ignore what is not theirs.
        assert_eq!(load::<Settings>(&path).unwrap().output.bitrate_mbps, 18);
        assert_eq!(load::<Wrapped>(&path).unwrap(), wrapped);
    }

    #[test]
    fn the_default_path_is_in_a_configuration_folder() {
        let path = default_path().expect("a home folder");
        assert_eq!(path.file_name().unwrap(), "settings.toml");
        assert!(path.to_string_lossy().contains("flight-replay"));
    }
}
