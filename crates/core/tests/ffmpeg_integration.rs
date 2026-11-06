// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Integration tests that run the real `ffmpeg` and `ffprobe`. Each test prints a message and returns early
//! when the programs cannot be found.

use std::path::{Path, PathBuf};
use std::process::Command;

use flight_replay_core::ffmpeg::{
    AudioInput, EncodeJob, EncodeProcess, FrameReader, Tools, VideoEncoding, VideoInput, probe,
};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/clip-3s.mp4")
}

fn tools() -> Option<Tools> {
    match Tools::locate(None) {
        Ok(tools) => Some(tools),
        Err(error) => {
            eprintln!("skipping FFmpeg test: {error}");
            None
        }
    }
}

/// Stream-copy MD5 of the first video stream (the MD5 muxer hashes the packets as they are).
fn video_md5(tools: &Tools, path: &Path) -> String {
    let output = Command::new(tools.ffmpeg())
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-map", "0:v:0", "-c", "copy", "-f", "md5", "-"])
        .output()
        .expect("ffmpeg runs");
    let text = String::from_utf8_lossy(&output.stdout);
    text.trim().trim_start_matches("MD5=").to_owned()
}

/// Duration of each stream, in seconds, as ffprobe reports it.
fn stream_durations(tools: &Tools, path: &Path) -> Vec<(String, f64)> {
    let output = Command::new(tools.ffprobe())
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,duration",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .expect("ffprobe runs");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (kind, duration) = line.split_once(',')?;
            Some((kind.to_owned(), duration.trim().parse().ok()?))
        })
        .collect()
}

/// Three seconds of a 440 Hz tone in stereo, as signed 16-bit little-endian PCM.
fn tone_pcm(seconds: f64) -> Vec<u8> {
    let frames = (48_000.0 * seconds) as usize;
    let mut bytes = Vec::with_capacity(frames * 4);
    for n in 0..frames {
        let value = (f64::sin(std::f64::consts::TAU * 440.0 * n as f64 / 48_000.0) * 8000.0) as i16;
        for _ in 0..2 {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

fn copy_job(output: PathBuf, audio: AudioInput) -> EncodeJob {
    EncodeJob {
        video: VideoInput::Copy { path: fixture() },
        audio,
        encoding: None,
        audio_bitrate_kbps: 192,
        faststart: true,
        duration_s: None,
        output,
    }
}

#[test]
fn probe_reads_the_fixture() {
    let Some(tools) = tools() else { return };
    let info = probe(&tools, &fixture()).expect("probe works");
    assert_eq!(info.codec, "h264");
    assert_eq!((info.width, info.height), (1280, 720));
    assert_eq!(info.frame_count, 180);
    assert!((info.frame_rate - 60.0).abs() < 1e-6);
    assert_eq!(info.audio_streams, 0);
}

#[test]
fn frame_reader_yields_every_frame_then_stops() {
    let Some(tools) = tools() else { return };
    let info = probe(&tools, &fixture()).unwrap();
    let mut reader =
        FrameReader::open(&tools, &fixture(), info.width, info.height, 0.0, None).unwrap();
    let mut buffer = Vec::new();
    let mut count = 0;
    while reader.read_frame(&mut buffer).unwrap() {
        assert_eq!(buffer.len(), 1280 * 720 * 4);
        count += 1;
    }
    assert_eq!(count, info.frame_count);
    reader.finish().unwrap();
}

#[test]
fn copy_with_sound_keeps_the_video_stream_and_matches_the_durations() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("copy-with-sound.mp4");

    let job = copy_job(
        output.clone(),
        AudioInput::PcmStdin {
            sample_rate: 48_000,
            channels: 2,
        },
    );
    let mut encode = EncodeProcess::spawn(&tools, &job).unwrap();
    encode.write(&tone_pcm(3.0)).unwrap();
    let written = encode.finish().unwrap();
    assert_eq!(written, output);
    assert!(!job.partial_path().exists(), "the partial file is renamed");

    assert_eq!(
        video_md5(&tools, &fixture()),
        video_md5(&tools, &output),
        "video packets are copied"
    );

    let durations = stream_durations(&tools, &output);
    let video = durations
        .iter()
        .find(|(k, _)| k == "video")
        .expect("video stream")
        .1;
    let audio = durations
        .iter()
        .find(|(k, _)| k == "audio")
        .expect("audio stream")
        .1;
    assert!(
        (video - audio).abs() < 0.001,
        "video {video} s, audio {audio} s"
    );
}

#[test]
fn raw_frames_are_encoded_with_the_chosen_encoder() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("raw.mp4");
    let info = probe(&tools, &fixture()).unwrap();

    // Decode 0.5 s of the fixture and encode it again from raw frames.
    let mut reader =
        FrameReader::open(&tools, &fixture(), info.width, info.height, 0.0, Some(0.5)).unwrap();
    let job = EncodeJob {
        video: VideoInput::Raw {
            width: info.width,
            height: info.height,
            frame_rate: info.frame_rate,
        },
        audio: AudioInput::None,
        encoding: Some(VideoEncoding {
            encoder: "libx264".into(),
            bitrate_mbps: 8,
            extra_args: vec!["-preset".into(), "ultrafast".into()],
            upscale_1440p: false,
        }),
        audio_bitrate_kbps: 192,
        faststart: false,
        duration_s: None,
        output: output.clone(),
    };
    let mut encode = EncodeProcess::spawn(&tools, &job).unwrap();
    let mut buffer = Vec::new();
    let mut frames = 0;
    while reader.read_frame(&mut buffer).unwrap() {
        encode.write(&buffer).unwrap();
        frames += 1;
    }
    reader.finish().unwrap();
    encode.finish().unwrap();

    let encoded = probe(&tools, &output).unwrap();
    assert_eq!(encoded.codec, "h264");
    assert_eq!(encoded.frame_count, frames);
}

#[test]
fn an_aborted_encode_leaves_no_file_behind() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("aborted.mp4");
    let job = copy_job(
        output.clone(),
        AudioInput::PcmStdin {
            sample_rate: 48_000,
            channels: 2,
        },
    );
    let mut encode = EncodeProcess::spawn(&tools, &job).unwrap();
    encode.write(&tone_pcm(0.5)).unwrap();
    encode.abort();
    assert!(!output.exists());
    assert!(!job.partial_path().exists());
}

#[test]
fn a_copied_video_cannot_be_encoded_again() {
    let job = EncodeJob {
        video: VideoInput::Copy { path: fixture() },
        audio: AudioInput::None,
        encoding: Some(VideoEncoding {
            encoder: "libx264".into(),
            bitrate_mbps: 8,
            extra_args: Vec::new(),
            upscale_1440p: false,
        }),
        audio_bitrate_kbps: 192,
        faststart: false,
        duration_s: None,
        output: PathBuf::from("unused.mp4"),
    };
    assert!(job.validate().is_err());
}
