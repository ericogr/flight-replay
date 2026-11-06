// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Whole renders on the fixture clip, with the real FFmpeg. Each test returns early, with a message, when FFmpeg is
//! not installed.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use flight_replay_core::ffmpeg::{DecodeOptions, FrameReader, Tools, VideoInfo, probe};
use flight_replay_core::font::CharacterSize;
use flight_replay_core::osd::{OsdFile, OsdOptions};
use flight_replay_core::render::{
    CancelToken, EncoderChoice, Event, Mode, OsdLayer, OutputPlan, Overwrite, RenderError,
    RenderPlan, SrtLayer, Stage, VideoOutput, Window, render,
};
use flight_replay_core::srt::{SrtFile, SrtOptions};
use flight_replay_core::synth::SoundSettings;
use flight_replay_core::timeline::SyncOptions;
use image::RgbaImage;

fn tools() -> Option<Tools> {
    match Tools::locate(None) {
        Ok(tools) => Some(tools),
        Err(error) => {
            eprintln!("skipping FFmpeg test: {error}");
            None
        }
    }
}

fn encoder() -> EncoderChoice {
    EncoderChoice {
        name: "libx264".into(),
        extra_args: vec!["-preset".into(), "ultrafast".into()],
        bitrate_mbps: 8,
        upscale_1440p: false,
    }
}

/// A plan on the fixture clip that does nothing yet: the test turns on what it wants.
fn plan(tools: &Tools, output: PathBuf) -> RenderPlan {
    let video = common::fixture("clip-3s.mp4");
    let info: VideoInfo = probe(tools, &video).unwrap();
    RenderPlan {
        video,
        info,
        osd: Some(OsdFile::open(&common::fixture("clip-3s.osd")).unwrap()),
        osd_path: Some(common::fixture("clip-3s.osd")),
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

fn draw_osd(plan: &mut RenderPlan) {
    plan.osd_layer = Some(OsdLayer {
        font: common::synthetic_sheet(CharacterSize::Small),
        options: OsdOptions::default(),
    });
    plan.output.video = VideoOutput::Encode(encoder());
}

fn run(
    tools: &Tools,
    plan: &RenderPlan,
) -> Result<flight_replay_core::render::Outcome, RenderError> {
    render(tools, plan, &CancelToken::new(), &mut |_| {})
}

fn video_md5(tools: &Tools, path: &Path) -> String {
    let output = Command::new(tools.ffmpeg())
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-map", "0:v:0", "-c", "copy", "-f", "md5", "-"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_start_matches("MD5=")
        .to_owned()
}

fn first_frame(tools: &Tools, path: &Path) -> RgbaImage {
    let info = probe(tools, path).unwrap();
    let mut reader =
        FrameReader::open(tools, path, &DecodeOptions::whole(info.width, info.height)).unwrap();
    let mut buffer = Vec::new();
    assert!(reader.read_frame(&mut buffer).unwrap());
    reader.cancel();
    RgbaImage::from_raw(info.width, info.height, buffer).unwrap()
}

fn leftovers(output: &Path) -> Vec<String> {
    let dir = output.parent().unwrap();
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn the_sound_alone_copies_the_video_and_adds_audio() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(&tools, dir.path().join("sound.mp4"));
    plan.sound = Some(SoundSettings::default());

    let mut events = Vec::new();
    let outcome = render(&tools, &plan, &CancelToken::new(), &mut |e| events.push(e)).unwrap();

    assert_eq!(outcome.mode, Mode::SoundOnly);
    assert_eq!(
        video_md5(&tools, &plan.video),
        video_md5(&tools, &outcome.output),
        "the video stream is untouched"
    );
    let check = outcome.check.unwrap();
    assert!(
        (check.audio_duration_s.unwrap() - check.video_duration_s).abs() < 0.005,
        "{check:?}"
    );
    assert_eq!(check.frames, 180);
    assert!(events.iter().any(
        |e| matches!(e, Event::Progress { stage: Stage::Audio, done, total } if done == total)
    ));
    assert_eq!(
        leftovers(&outcome.output),
        vec!["sound.mp4".to_owned()],
        "no temporary file is left"
    );
}

#[test]
fn the_osd_is_drawn_and_the_video_is_encoded_again() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(&tools, dir.path().join("osd.mp4"));
    draw_osd(&mut plan);

    let outcome = run(&tools, &plan).unwrap();
    assert_eq!(outcome.mode, Mode::Overlay);
    let check = outcome.check.unwrap();
    assert_eq!(check.frames, 180);
    assert_eq!(check.codec, "h264");
    assert_eq!(check.audio_duration_s, None);

    // The OSD changed the picture: many pixels of the first frame differ from the source's.
    let drawn = first_frame(&tools, &outcome.output);
    let source = first_frame(&tools, &plan.video);
    let changed = drawn
        .pixels()
        .zip(source.pixels())
        .filter(|(a, b)| {
            a.0[..3]
                .iter()
                .zip(&b.0[..3])
                .any(|(x, y)| x.abs_diff(*y) > 40)
        })
        .count();
    assert!(changed > 5_000, "only {changed} pixels changed");
}

#[test]
fn the_osd_the_srt_and_the_sound_together() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(&tools, dir.path().join("all.mp4"));
    draw_osd(&mut plan);
    plan.srt_layer = Some(SrtLayer {
        file: SrtFile::open(&common::fixture("clip-3s.srt")).unwrap(),
        options: SrtOptions::default(),
    });
    plan.sound = Some(SoundSettings::default());

    let outcome = run(&tools, &plan).unwrap();
    let check = outcome.check.unwrap();
    assert_eq!(check.frames, 180);
    assert!(
        (check.audio_duration_s.unwrap() - check.video_duration_s).abs() < 0.05,
        "{check:?}"
    );
    assert!(outcome.audio_stats.is_some());
    assert_eq!(
        leftovers(&outcome.output),
        vec!["all.mp4".to_owned()],
        "the temporary WAV is removed"
    );
}

#[test]
fn a_chroma_key_background_replaces_the_picture() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(&tools, dir.path().join("chroma.mp4"));
    draw_osd(&mut plan);
    plan.chroma_key = Some([1, 177, 64]);

    let outcome = run(&tools, &plan).unwrap();
    assert_eq!(outcome.check.unwrap().frames, 180);
    let frame = first_frame(&tools, &outcome.output);
    // a spot with no OSD glyph (the left edge, the middle of the picture), after lossy YUV encoding
    let pixel = frame.get_pixel(20, 400).0;
    for (got, want) in pixel[..3].iter().zip([1u8, 177, 64]) {
        assert!(got.abs_diff(want) < 14, "background pixel {pixel:?}");
    }
}

#[test]
fn the_picture_can_be_scaled_to_1440p() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(&tools, dir.path().join("big.mp4"));
    draw_osd(&mut plan);
    plan.output.video = VideoOutput::Encode(EncoderChoice {
        upscale_1440p: true,
        ..encoder()
    });
    let outcome = run(&tools, &plan).unwrap();
    let info = probe(&tools, &outcome.output).unwrap();
    assert_eq!((info.width, info.height), (2560, 1440));
}

#[test]
fn a_stretch_of_the_video_can_be_rendered_alone() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(&tools, dir.path().join("part.mp4"));
    draw_osd(&mut plan);
    plan.sound = Some(SoundSettings::default());
    plan.window = Some(Window {
        start_s: 1.0,
        duration_s: Some(1.0),
    });
    let outcome = run(&tools, &plan).unwrap();
    assert!(outcome.window.partial);
    let check = outcome.check.unwrap();
    assert_eq!(check.frames, 60);
    assert!((check.video_duration_s - 1.0).abs() < 0.05, "{check:?}");
    assert!(
        (check.audio_duration_s.unwrap() - 1.0).abs() < 0.05,
        "{check:?}"
    );
}

#[test]
fn cancelling_leaves_nothing_behind() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(&tools, dir.path().join("cancelled.mp4"));
    draw_osd(&mut plan);
    plan.sound = Some(SoundSettings::default());

    let cancel = CancelToken::new();
    let handle = cancel.clone();
    let result = render(&tools, &plan, &cancel, &mut |event| {
        if matches!(
            event,
            Event::Progress {
                stage: Stage::Video,
                ..
            }
        ) {
            handle.cancel();
        }
    });
    assert!(matches!(result, Err(RenderError::Cancelled)), "{result:?}");
    assert_eq!(
        leftovers(&plan.output.path),
        Vec::<String>::new(),
        "no output, partial or temporary file"
    );
}

#[test]
fn an_existing_output_is_only_replaced_when_the_plan_says_so() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("exists.mp4");
    std::fs::write(&output, b"old").unwrap();
    let mut plan = plan(&tools, output.clone());
    plan.sound = Some(SoundSettings::default());

    assert!(matches!(
        run(&tools, &plan),
        Err(RenderError::OutputExists(_))
    ));
    assert_eq!(
        std::fs::read(&output).unwrap(),
        b"old",
        "the file was not touched"
    );
    plan.output.overwrite = Overwrite::Replace;
    run(&tools, &plan).unwrap();
    assert!(std::fs::metadata(&output).unwrap().len() > 1000);
}

#[test]
fn the_sound_of_a_video_that_has_some_is_only_replaced_when_allowed() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(&tools, dir.path().join("replace.mp4"));
    plan.sound = Some(SoundSettings::default());
    plan.info.audio_streams = 1; // as if the source had sound
    assert!(matches!(
        run(&tools, &plan),
        Err(RenderError::SourceHasAudio)
    ));
    plan.output.replace_audio = true;
    run(&tools, &plan).unwrap();
}

#[test]
fn a_plan_that_cannot_run_lists_every_problem() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut plan = plan(&tools, dir.path().join("bad.mp4"));
    plan.osd = None;
    plan.sound = Some(SoundSettings::default());
    plan.osd_layer = Some(OsdLayer {
        font: common::synthetic_sheet(CharacterSize::Small),
        options: OsdOptions::default(),
    });
    // drawing, but the output is a copy
    let Err(RenderError::InvalidPlan(problems)) = run(&tools, &plan) else {
        panic!("the plan should be refused");
    };
    assert!(problems.len() >= 3, "{problems:?}");
}
