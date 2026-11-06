// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::path::Path;

use serde::Deserialize;

use super::tools::command;
use super::{FfmpegError, Tools};

/// What Flight Replay needs to know about a video file.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    /// Codec of the first video stream, such as `h264`.
    pub codec: String,
    /// Width of the picture, in pixels.
    pub width: u32,
    /// Height of the picture, in pixels.
    pub height: u32,
    /// Average frame rate, in frames per second: frames divided by duration. It is lower than the nominal rate
    /// when the video has gaps.
    pub frame_rate: f64,
    /// The rate the video declares (`r_frame_rate`), in frames per second. Frame times are multiples of its inverse.
    pub nominal_frame_rate: f64,
    /// Duration of the video, in seconds.
    pub duration_s: f64,
    /// Number of video frames.
    pub frame_count: u64,
    /// Number of audio streams in the file.
    pub audio_streams: usize,
    /// Duration of the first audio stream, in seconds.
    pub audio_duration_s: Option<f64>,
}

/// Reads the description of a video file with `ffprobe`.
pub fn probe(tools: &Tools, path: &Path) -> Result<VideoInfo, FfmpegError> {
    let output = command(tools.ffprobe())
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(FfmpegError::spawn("ffprobe"))?;
    if !output.status.success() {
        return Err(FfmpegError::Failed {
            tool: "ffprobe",
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    parse_probe_json(&output.stdout)
}

#[derive(Deserialize)]
struct Root {
    #[serde(default)]
    streams: Vec<Stream>,
    format: Option<Format>,
}

#[derive(Deserialize)]
struct Format {
    duration: Option<String>,
}

#[derive(Deserialize)]
struct Stream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    nb_frames: Option<String>,
    duration: Option<String>,
}

/// Turns the JSON written by `ffprobe -print_format json -show_format -show_streams` into a [`VideoInfo`].
pub fn parse_probe_json(json: &[u8]) -> Result<VideoInfo, FfmpegError> {
    let root: Root = serde_json::from_slice(json).map_err(|e| FfmpegError::Probe(e.to_string()))?;

    let audio_streams = root
        .streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("audio"))
        .count();
    let audio_duration_s = root
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("audio"))
        .and_then(|s| s.duration.as_deref())
        .and_then(|d| d.parse::<f64>().ok());
    let video = root
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"))
        .ok_or_else(|| FfmpegError::Probe("no video stream".into()))?;

    let width = video
        .width
        .ok_or_else(|| FfmpegError::Probe("no width".into()))?;
    let height = video
        .height
        .ok_or_else(|| FfmpegError::Probe("no height".into()))?;
    let frame_rate = video
        .avg_frame_rate
        .as_deref()
        .and_then(parse_rate)
        .or_else(|| video.r_frame_rate.as_deref().and_then(parse_rate))
        .ok_or_else(|| FfmpegError::Probe("no frame rate".into()))?;

    let nominal_frame_rate = video
        .r_frame_rate
        .as_deref()
        .and_then(parse_rate)
        .unwrap_or(frame_rate);

    let duration_s = video
        .duration
        .as_deref()
        .and_then(|d| d.parse::<f64>().ok())
        .or_else(|| {
            root.format
                .as_ref()
                .and_then(|f| f.duration.as_deref())
                .and_then(|d| d.parse::<f64>().ok())
        })
        .ok_or_else(|| FfmpegError::Probe("no duration".into()))?;

    // The container's count is exact. Without it, the count follows from the duration and the rate.
    let frame_count = video
        .nb_frames
        .as_deref()
        .and_then(|n| n.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or_else(|| (duration_s * frame_rate).round() as u64);

    Ok(VideoInfo {
        codec: video.codec_name.clone().unwrap_or_default(),
        width,
        height,
        frame_rate,
        nominal_frame_rate,
        duration_s,
        frame_count,
        audio_streams,
        audio_duration_s,
    })
}

/// Parses a rational rate such as `60/1` or `30000/1001`.
fn parse_rate(rate: &str) -> Option<f64> {
    let (num, den) = rate.split_once('/')?;
    let num: f64 = num.trim().parse().ok()?;
    let den: f64 = den.trim().parse().ok()?;
    (num > 0.0 && den > 0.0).then(|| num / den)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "streams": [
            {"codec_type": "video", "codec_name": "h264", "width": 1280, "height": 720,
             "avg_frame_rate": "60/1", "r_frame_rate": "60/1", "nb_frames": "180", "duration": "3.000000"}
        ],
        "format": {"duration": "3.010000"}
    }"#;

    #[test]
    fn parses_a_video_stream() {
        let info = parse_probe_json(SAMPLE.as_bytes()).unwrap();
        assert_eq!(info.codec, "h264");
        assert_eq!((info.width, info.height), (1280, 720));
        assert!((info.frame_rate - 60.0).abs() < 1e-9);
        assert!((info.nominal_frame_rate - 60.0).abs() < 1e-9);
        assert_eq!(info.frame_count, 180);
        assert_eq!(info.audio_duration_s, None);
        assert!((info.duration_s - 3.0).abs() < 1e-9);
        assert_eq!(info.audio_streams, 0);
    }

    #[test]
    fn a_video_with_gaps_has_a_lower_average_than_nominal_rate_and_reports_the_audio() {
        let json = r#"{"streams":[
            {"codec_type":"video","width":1280,"height":720,"avg_frame_rate":"482940/8461","r_frame_rate":"60/1",
             "nb_frames":"48294","duration":"846.100000"},
            {"codec_type":"audio","codec_name":"aac","duration":"846.050"}]}"#;
        let info = parse_probe_json(json.as_bytes()).unwrap();
        assert!(
            (info.frame_rate - 57.0785).abs() < 1e-3,
            "{}",
            info.frame_rate
        );
        assert!((info.nominal_frame_rate - 60.0).abs() < 1e-9);
        assert_eq!(info.audio_streams, 1);
        assert_eq!(info.audio_duration_s, Some(846.05));
    }

    #[test]
    fn counts_frames_from_duration_when_the_container_has_no_count() {
        let json = r#"{"streams":[{"codec_type":"video","width":640,"height":480,
            "avg_frame_rate":"30000/1001","duration":"2.0"}]}"#;
        let info = parse_probe_json(json.as_bytes()).unwrap();
        assert_eq!(info.frame_count, 60);
    }

    #[test]
    fn falls_back_to_the_format_duration() {
        let json = r#"{"streams":[{"codec_type":"video","width":2,"height":2,"avg_frame_rate":"25/1"}],
            "format":{"duration":"4.0"}}"#;
        assert!((parse_probe_json(json.as_bytes()).unwrap().duration_s - 4.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_a_file_without_video() {
        let json = r#"{"streams":[{"codec_type":"audio"}]}"#;
        assert!(matches!(
            parse_probe_json(json.as_bytes()),
            Err(FfmpegError::Probe(_))
        ));
    }

    #[test]
    fn rejects_zero_rates() {
        assert_eq!(parse_rate("0/0"), None);
        assert_eq!(parse_rate("60"), None);
    }
}
