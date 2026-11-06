// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::FfmpegError;

/// Paths of the `ffmpeg` and `ffprobe` programs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}

impl Tools {
    /// Creates the pair from explicit paths, without checking them.
    #[must_use]
    pub fn new(ffmpeg: PathBuf, ffprobe: PathBuf) -> Self {
        Self { ffmpeg, ffprobe }
    }

    /// Finds `ffmpeg` and `ffprobe`, and checks that both run.
    ///
    /// `preferred` is a folder or a program path given by the user. The search continues with the
    /// `FLIGHT_REPLAY_FFMPEG` environment variable (same format), `PATH`, the usual install folders of each
    /// system, and the folder of the running executable.
    pub fn locate(preferred: Option<&Path>) -> Result<Self, FfmpegError> {
        let mut hints: Vec<PathBuf> = preferred.map(Path::to_path_buf).into_iter().collect();
        hints.extend(env::var_os("FLIGHT_REPLAY_FFMPEG").map(PathBuf::from));

        let ffmpeg = find("ffmpeg", &hints).ok_or(FfmpegError::NotFound { tool: "ffmpeg" })?;
        let ffprobe = find("ffprobe", &hints).ok_or(FfmpegError::NotFound { tool: "ffprobe" })?;
        Ok(Self { ffmpeg, ffprobe })
    }

    /// Path of `ffmpeg`.
    #[must_use]
    pub fn ffmpeg(&self) -> &Path {
        &self.ffmpeg
    }

    /// Path of `ffprobe`.
    #[must_use]
    pub fn ffprobe(&self) -> &Path {
        &self.ffprobe
    }
}

/// Builds a command for an external program.
#[cfg(windows)]
pub(crate) fn command(program: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Builds a command for an external program.
#[cfg(not(windows))]
pub(crate) fn command(program: &Path) -> Command {
    Command::new(program)
}

/// Runs a program with `-version` and reports whether it succeeded.
pub(crate) fn runs(program: &Path) -> bool {
    command(program)
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn find(name: &str, hints: &[PathBuf]) -> Option<PathBuf> {
    let file = format!("{name}{}", env::consts::EXE_SUFFIX);
    candidate_dirs(hints)
        .into_iter()
        .map(|dir| dir.join(&file))
        .find(|path| path.is_file() && runs(path))
}

fn candidate_dirs(hints: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = hints
        .iter()
        .map(|hint| {
            if hint.is_dir() {
                hint.clone()
            } else {
                hint.parent().map(Path::to_path_buf).unwrap_or_default()
            }
        })
        .collect();

    if let Some(path) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&path));
    }

    dirs.extend(WELL_KNOWN_DIRS.iter().map(PathBuf::from));

    #[cfg(windows)]
    if let Some(local) = env::var_os("LOCALAPPDATA") {
        // Folder where `winget install ffmpeg` puts its links.
        dirs.push(
            PathBuf::from(local)
                .join("Microsoft")
                .join("WinGet")
                .join("Links"),
        );
    }

    // A program that was copied next to Flight Replay is found as well.
    if let Some(parent) = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        dirs.push(parent);
    }
    dirs
}

#[cfg(target_os = "macos")]
const WELL_KNOWN_DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin"];

#[cfg(target_os = "linux")]
const WELL_KNOWN_DIRS: &[&str] = &["/usr/local/bin", "/usr/bin", "/snap/bin"];

#[cfg(windows)]
const WELL_KNOWN_DIRS: &[&str] = &["C:\\ffmpeg\\bin"];

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
const WELL_KNOWN_DIRS: &[&str] = &[];
