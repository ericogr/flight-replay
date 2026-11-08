// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::io;

/// Errors from the external `ffmpeg` and `ffprobe` programs.
#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    /// The program could not be found, or it does not run.
    #[error("{tool} was not found: install FFmpeg, or set its folder in the settings")]
    NotFound {
        /// Name of the program (`ffmpeg` or `ffprobe`).
        tool: &'static str,
    },

    /// A program was found, but it is older than the application supports. A copy that works is still looked for
    /// in the other folders, so this is only the answer when there is none.
    #[error(
        "{tool} {version} is too old: FFmpeg {} or newer is needed. Install a newer one, or choose the folder of one",
        super::tools::MIN_MAJOR_VERSION
    )]
    TooOld {
        /// Name of the program (`ffmpeg` or `ffprobe`).
        tool: &'static str,
        /// The version it reports, such as `4.4.2`.
        version: String,
    },

    /// The program could not be started.
    #[error("could not start {tool}: {source}")]
    Spawn {
        /// Name of the program.
        tool: &'static str,
        /// Operating system error.
        #[source]
        source: io::Error,
    },

    /// The program ran and reported a failure.
    #[error("{tool} failed:\n{stderr}")]
    Failed {
        /// Name of the program.
        tool: &'static str,
        /// The last lines the program wrote to its error stream.
        stderr: String,
    },

    /// `ffprobe` returned something that is not a usable video description.
    #[error("unusable video description: {0}")]
    Probe(String),

    /// A raw frame could not be read in full.
    #[error("the video ended in the middle of a frame")]
    TruncatedFrame,

    /// A job was built in a way that cannot run.
    #[error("invalid encoding job: {0}")]
    InvalidJob(&'static str),

    /// A read or write on a pipe or a file failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl FfmpegError {
    /// For `map_err` on the start of a program: `command.spawn().map_err(FfmpegError::spawn("ffmpeg"))`.
    pub(crate) fn spawn(tool: &'static str) -> impl FnOnce(io::Error) -> Self {
        move |source| Self::Spawn { tool, source }
    }
}
