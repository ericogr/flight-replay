// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The external `ffmpeg` and `ffprobe` programs.
//!
//! Flight Replay does not link FFmpeg. It starts the programs as child processes and exchanges data with them
//! through their standard streams and files. FFmpeg is not bundled: the user installs it, or points the
//! application at it.

mod decode;
mod encode;
mod encoders;
mod error;
mod probe;
mod process;
mod progress;
mod tools;

pub use decode::{DecodeOptions, FrameReader};
pub use encode::{AudioInput, EncodeJob, EncodeProcess, VideoEncoding, VideoInput};
pub use encoders::{Codec, EncoderInfo, candidates, detect, parse_encoder_list};
pub use error::FfmpegError;
pub use probe::{VideoInfo, parse_probe_json, probe};
pub use progress::{Progress, parse_progress_line};
pub use tools::Tools;

/// Writes a command line for showing to the user: arguments with unusual characters are single-quoted, as a shell
/// would need.
pub(crate) fn quote_command(program: &std::path::Path, args: &[std::ffi::OsString]) -> String {
    std::iter::once(program.as_os_str())
        .chain(args.iter().map(std::ffi::OsString::as_os_str))
        .map(|arg| quote(&arg.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-')
        });
    if plain {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_what_a_shell_would_need() {
        assert_eq!(quote("-c:v"), "-c:v");
        assert_eq!(quote("my file.mp4"), "'my file.mp4'");
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote(""), "''");
    }
}
