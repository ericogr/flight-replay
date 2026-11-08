// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool, backend/src/ffmpeg/encoders.rs (GPL-3.0-only), Copyright (C) Alexander van
// Saase. Modified for Flight Replay: encoder list parsing, a test encode per encoder, and the platform
// lists.

use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use std::thread;

use serde::{Deserialize, Serialize};

use super::Tools;
use super::tools::command;

/// Video codec families offered to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Codec {
    /// H.264 / AVC.
    H264,
    /// H.265 / HEVC.
    H265,
}

impl fmt::Display for Codec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Codec::H264 => f.write_str("H.264"),
            Codec::H265 => f.write_str("H.265"),
        }
    }
}

/// A video encoder that FFmpeg knows about on this system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderInfo {
    /// FFmpeg encoder name, such as `libx264`.
    pub name: String,
    /// Codec the encoder produces.
    pub codec: Codec,
    /// Whether the encoder uses dedicated hardware.
    pub hardware: bool,
    /// Options FFmpeg needs for this encoder, besides the bitrate.
    pub extra_args: Vec<String>,
    /// Whether a one-frame test encode succeeded. Set by [`detect`].
    pub works: bool,
}

fn encoder(name: &str, codec: Codec, hardware: bool, extra_args: &[&str]) -> EncoderInfo {
    EncoderInfo {
        name: name.to_owned(),
        codec,
        hardware,
        extra_args: extra_args.iter().map(|arg| (*arg).to_owned()).collect(),
        works: false,
    }
}

/// Every encoder Flight Replay can use on this operating system, not yet checked.
#[must_use]
pub fn candidates() -> Vec<EncoderInfo> {
    let mut list = vec![
        encoder("libx264", Codec::H264, false, &[]),
        encoder("libx265", Codec::H265, false, &[]),
    ];

    #[cfg(windows)]
    list.push(encoder("h264_amf", Codec::H264, true, &[]));
    #[cfg(any(windows, target_os = "linux"))]
    {
        list.push(encoder("h264_nvenc", Codec::H264, true, &[]));
        list.push(encoder("h264_qsv", Codec::H264, true, &[]));
    }
    #[cfg(target_os = "linux")]
    {
        list.push(encoder("h264_vaapi", Codec::H264, true, &[]));
        list.push(encoder("h264_v4l2m2m", Codec::H264, true, &[]));
    }
    #[cfg(target_os = "macos")]
    list.push(encoder("h264_videotoolbox", Codec::H264, true, &[]));

    #[cfg(windows)]
    list.push(encoder("hevc_amf", Codec::H265, true, &[]));
    #[cfg(any(windows, target_os = "linux"))]
    {
        list.push(encoder("hevc_nvenc", Codec::H265, true, &[]));
        list.push(encoder("hevc_qsv", Codec::H265, true, &[]));
    }
    #[cfg(target_os = "linux")]
    {
        list.push(encoder("hevc_vaapi", Codec::H265, true, &[]));
        list.push(encoder("hevc_v4l2m2m", Codec::H265, true, &[]));
    }
    #[cfg(target_os = "macos")]
    // QuickTime on macOS only plays HEVC tagged as hvc1.
    list.push(encoder(
        "hevc_videotoolbox",
        Codec::H265,
        true,
        &["-tag:v", "hvc1"],
    ));

    list
}

/// Lists the encoders that `ffmpeg -encoders` reports, then checks each candidate with a one-frame test encode.
/// Candidates that FFmpeg does not list are not tried.
#[must_use]
pub fn detect(tools: &Tools) -> Vec<EncoderInfo> {
    let listed = list_encoders(tools.ffmpeg());
    let mut all = candidates();
    thread::scope(|scope| {
        for info in &mut all {
            if !listed.is_empty() && !listed.contains(&info.name) {
                continue;
            }
            scope.spawn(move || info.works = test_encode(tools.ffmpeg(), &info.name));
        }
    });
    all
}

/// Names of the encoders listed by `ffmpeg -encoders`. Empty if the list cannot be read.
fn list_encoders(ffmpeg: &Path) -> HashSet<String> {
    let output = command(ffmpeg).args(["-hide_banner", "-encoders"]).output();
    match output {
        Ok(output) => parse_encoder_list(&String::from_utf8_lossy(&output.stdout)),
        Err(_) => HashSet::new(),
    }
}

/// Reads the names from the output of `ffmpeg -encoders`. Video encoders have a flag line that starts with `V`.
#[must_use]
pub fn parse_encoder_list(output: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut after_separator = false;
    for line in output.lines() {
        if line.starts_with(" ------") {
            after_separator = true;
            continue;
        }
        if !after_separator {
            continue;
        }
        let mut fields = line.split_whitespace();
        if let (Some(flags), Some(name)) = (fields.next(), fields.next())
            && flags.len() == 6
            && flags.starts_with('V')
        {
            names.insert(name.to_owned());
        }
    }
    names
}

fn test_encode(ffmpeg: &Path, name: &str) -> bool {
    let mut command = command(ffmpeg);
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "nullsrc=s=256x256:d=0.1",
            "-pix_fmt",
            "yuv420p",
            "-c:v",
            name,
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    command.status().is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Encoders:
 V..... = Video
 A..... = Audio
 ------
 V....D libx264              libx264 H.264 / AVC / MPEG-4 AVC (codec h264)
 V..... h264_nvenc           NVIDIA NVENC H.264 encoder (codec h264)
 A....D aac                  AAC (Advanced Audio Coding)
 V....D hevc_qsv             HEVC (Intel Quick Sync Video acceleration) (codec hevc)
";

    #[test]
    fn parses_only_video_encoders() {
        let names = parse_encoder_list(SAMPLE);
        assert!(names.contains("libx264"));
        assert!(names.contains("h264_nvenc"));
        assert!(names.contains("hevc_qsv"));
        assert!(!names.contains("aac"));
        assert!(!names.contains("Video"));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn software_encoders_are_always_candidates() {
        let list = candidates();
        assert!(list.iter().any(|e| e.name == "libx264" && !e.hardware));
        assert!(
            list.iter()
                .any(|e| e.name == "libx265" && e.codec == Codec::H265)
        );
    }
}
