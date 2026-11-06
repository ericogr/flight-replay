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
