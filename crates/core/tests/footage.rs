// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! A flight of several videos, on the fixture clip: two copies of it, each with its own `.osd` and `.srt`, rendered
//! into one video with the real FFmpeg. The tests that need it return early, with a message, when FFmpeg is not
//! installed.

mod common;

use std::path::{Path, PathBuf};

use flight_replay_core::cancel::CancelToken;
use flight_replay_core::ffmpeg::{DecodeOptions, FrameReader, Tools, probe};
use flight_replay_core::font::CharacterSize;
use flight_replay_core::footage::{Footage, VideoPart};
use flight_replay_core::osd::{OsdFile, OsdOptions, join};
use flight_replay_core::render::{
    EncoderChoice, Event, Mode, OsdLayer, Outcome, OutputPlan, Overwrite, RenderPlan, SrtLayer,
    Stage, VideoOutput, Window, command_preview, render,
};
use flight_replay_core::srt::{SrtFile, SrtOptions};
use flight_replay_core::synth::SoundSettings;
use flight_replay_core::timeline::SyncOptions;
use image::RgbaImage;

/// How many frames the clip has, and how long it is.
const CLIP_FRAMES: u64 = 180;
const CLIP_SECONDS: f64 = 3.0;

fn tools() -> Option<Tools> {
    match Tools::locate(None) {
        Ok(tools) => Some(tools),
        Err(error) => {
            eprintln!("skipping the test: {error}");
            None
        }
    }
}

/// Two copies of the clip in a folder of their own, as two files of one flight.
fn two_videos(tools: &Tools, dir: &Path) -> Footage {
    let parts = ["AvatarG0001.mp4", "AvatarG0002.mp4"].map(|name| {
        let path = dir.join(name);
        std::fs::copy(common::fixture("clip-3s.mp4"), &path).unwrap();
        VideoPart {
            info: probe(tools, &path).unwrap(),
            path,
        }
    });
    Footage::new(parts.to_vec()).unwrap()
}

/// A plan on the two videos, with the recordings of both, that adds nothing yet.
fn plan(footage: Footage, output: PathBuf) -> RenderPlan {
    let files = ["AvatarG0001.osd", "AvatarG0002.osd"].map(|name| {
        (
            name.to_owned(),
            OsdFile::open(&common::fixture("clip-3s.osd")).unwrap(),
        )
    });
    let joined = join(files.to_vec(), 0.0).unwrap();
    RenderPlan {
        footage,
        osd: Some(joined.file),
        osd_parts: joined.parts,
        osd_layer: None,
        srt_layer: None,
        chroma_key: None,
        sound: None,
        sync: SyncOptions::default(),
        window: None,
        output: OutputPlan {
            path: output,
            video: VideoOutput::Copy,
            audio_bitrate_kbps: 192,
            faststart: true,
            overwrite: Overwrite::Refuse,
            replace_audio: false,
        },
    }
}

fn draw(plan: &mut RenderPlan) {
    plan.osd_layer = Some(OsdLayer {
        font: common::synthetic_sheet(CharacterSize::Small),
        options: OsdOptions::default(),
    });
    let clip = SrtFile::open(&common::fixture("clip-3s.srt")).unwrap();
    plan.srt_layer = Some(SrtLayer {
        file: SrtFile::join(&[(&clip, 0.0), (&clip, plan.footage.start_s(1))]),
        options: SrtOptions::default(),
    });
    plan.output.video = VideoOutput::Encode(EncoderChoice {
        name: "libx264".into(),
        extra_args: vec!["-preset".into(), "ultrafast".into()],
        bitrate_mbps: 8,
        upscale_1440p: false,
    });
}

fn run(tools: &Tools, plan: &RenderPlan) -> (Outcome, Vec<Event>) {
    let mut events = Vec::new();
    let outcome = render(tools, plan, &CancelToken::new(), &mut |e| events.push(e)).unwrap();
    (outcome, events)
}

/// The frame at `index` of a video.
fn frame(tools: &Tools, path: &Path, index: u64) -> RgbaImage {
    let info = probe(tools, path).unwrap();
    let mut reader =
        FrameReader::open(tools, path, &DecodeOptions::whole(info.width, info.height)).unwrap();
    let mut buffer = Vec::new();
    for _ in 0..=index {
        assert!(reader.read_frame(&mut buffer).unwrap(), "frame {index}");
    }
    reader.cancel();
    RgbaImage::from_raw(info.width, info.height, buffer).unwrap()
}

/// The mean difference between two pictures, in levels of a channel.
fn difference(a: &RgbaImage, b: &RgbaImage) -> f64 {
    let total: u64 = a
        .as_raw()
        .iter()
        .zip(b.as_raw())
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    total as f64 / a.as_raw().len() as f64
}

#[test]
fn two_videos_are_drawn_on_and_joined_with_the_sound_of_both() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(two_videos(&tools, dir.path()), dir.path().join("out.mp4"));
    draw(&mut plan);
    plan.sound = Some(SoundSettings::default());

    let (outcome, events) = run(&tools, &plan);

    assert_eq!(outcome.mode, Mode::Overlay);
    let check = outcome.check.as_ref().expect("the output was checked");
    assert_eq!(
        check.frames,
        2 * CLIP_FRAMES,
        "no frame lost or repeated at the join"
    );
    assert!(
        (check.video_duration_s - 2.0 * CLIP_SECONDS).abs() < 0.05,
        "{check:?}"
    );
    assert!(
        (check.audio_duration_s.unwrap() - check.video_duration_s).abs() < 0.05,
        "the sound is as long as the picture: {check:?}"
    );
    // The warnings about the signals the clip lacks are about the sound. None is about the timing.
    assert!(
        outcome
            .warnings
            .iter()
            .all(|w| !w.contains("mapped telemetry") && !w.contains("time scale")),
        "the recordings fit the videos: {:?}",
        outcome.warnings
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Progress { stage: Stage::Video, done, total } if done == total && *total == 2 * CLIP_FRAMES
    )));

    // The second video starts over with its own recording: its first picture is the first of the first video, with
    // the first screen of the OSD and the first line of the SRT data, and not a picture that goes on from the end of
    // the first one. (The pictures are encoded, so they are alike and not equal.)
    let (first, joined, before) = (
        frame(&tools, &outcome.output, 0),
        frame(&tools, &outcome.output, CLIP_FRAMES),
        frame(&tools, &outcome.output, CLIP_FRAMES - 1),
    );
    let (same, across) = (difference(&first, &joined), difference(&before, &joined));
    assert!(
        2.0 * same < across,
        "the second video starts like the first ({same}) and not like the end of it ({across})"
    );
}

#[test]
fn two_videos_are_joined_with_the_sound_alone_without_encoding_them_again() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(two_videos(&tools, dir.path()), dir.path().join("out.mp4"));
    plan.sound = Some(SoundSettings::default());

    let (outcome, _) = run(&tools, &plan);

    assert_eq!(outcome.mode, Mode::SoundOnly);
    let check = outcome.check.as_ref().expect("the output was checked");
    assert_eq!(check.codec, "h264");
    assert_eq!(check.frames, 2 * CLIP_FRAMES);
    assert!(
        (check.video_duration_s - 2.0 * CLIP_SECONDS).abs() < 0.1,
        "{check:?}"
    );
    assert!(
        (check.audio_duration_s.unwrap() - check.video_duration_s).abs() < 0.1,
        "{check:?}"
    );
    let name = plan
        .output
        .path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let leftovers: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|f| f.starts_with(&name) && *f != name)
        .collect();
    assert!(
        leftovers.is_empty(),
        "the list of videos is removed: {leftovers:?}"
    );
}

#[test]
fn a_stretch_across_the_join_has_the_frames_of_both_videos() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(two_videos(&tools, dir.path()), dir.path().join("out.mp4"));
    draw(&mut plan);
    plan.sound = Some(SoundSettings::default());
    // One second before the join and one second after it.
    plan.window = Some(Window {
        start_s: CLIP_SECONDS - 1.0,
        duration_s: Some(2.0),
    });

    let (outcome, _) = run(&tools, &plan);

    assert!(outcome.window.partial);
    let check = outcome.check.as_ref().expect("the output was checked");
    assert_eq!(check.frames, 120);
    assert!((check.video_duration_s - 2.0).abs() < 0.05, "{check:?}");
}

#[test]
fn a_copied_stretch_may_start_in_the_second_video() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(two_videos(&tools, dir.path()), dir.path().join("out.mp4"));
    plan.sound = Some(SoundSettings::default());
    plan.window = Some(Window {
        start_s: CLIP_SECONDS + 1.0,
        duration_s: Some(1.0),
    });

    let (outcome, _) = run(&tools, &plan);

    let check = outcome.check.as_ref().expect("the output was checked");
    assert!((check.video_duration_s - 1.0).abs() < 0.1, "{check:?}");
    assert!(check.frames >= 55, "{check:?}");
}

#[test]
fn the_command_reads_each_video_in_its_turn() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(two_videos(&tools, dir.path()), dir.path().join("out.mp4"));
    draw(&mut plan);
    let lines = command_preview(&plan, tools.ffmpeg());
    // One decoder for each video, then the encoder.
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[0].contains("AvatarG0001.mp4") && lines[1].contains("AvatarG0002.mp4"));
}
