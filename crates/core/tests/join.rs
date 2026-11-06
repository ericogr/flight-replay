// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Several `.osd` files of one video, on the fixture clip. The joined video is the clip played twice, made with the
//! real FFmpeg; the tests that need it return early, with a message, when FFmpeg is not installed.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use flight_replay_core::ffmpeg::{Tools, probe};
use flight_replay_core::font::CharacterSize;
use flight_replay_core::osd::{Coverage, OsdFile, OsdOptions, join};
use flight_replay_core::render::{
    CancelToken, EncoderChoice, OsdLayer, OutputPlan, Overwrite, RenderPlan, VideoOutput, render,
};
use flight_replay_core::synth::SoundSettings;
use flight_replay_core::telemetry::{LoadOptions, Recording};
use flight_replay_core::timeline::SyncOptions;

fn twice() -> Vec<(String, OsdFile)> {
    ["first.osd", "second.osd"]
        .into_iter()
        .map(|name| {
            (
                name.to_owned(),
                OsdFile::open(&common::fixture("clip-3s.osd")).unwrap(),
            )
        })
        .collect()
}

/// The clip, twice, as one video of about six seconds.
fn joined_video(tools: &Tools, dir: &Path) -> PathBuf {
    let clip = std::fs::canonicalize(common::fixture("clip-3s.mp4")).unwrap();
    let list = dir.join("list.txt");
    std::fs::write(&list, format!("file '{0}'\nfile '{0}'\n", clip.display())).unwrap();
    let output = dir.join("joined.mp4");
    let status = Command::new(tools.ffmpeg())
        .args(["-v", "error", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list)
        .args(["-c", "copy"])
        .arg(&output)
        .status()
        .unwrap();
    assert!(status.success(), "FFmpeg joined the clip");
    output
}

#[test]
fn two_files_make_one_timeline() {
    let single = OsdFile::open(&common::fixture("clip-3s.osd")).unwrap();
    let joined = join(twice(), 0.0).unwrap();

    assert_eq!(joined.file.frames.len(), 2 * single.frames.len());
    assert!(
        (joined.file.duration_s() - 2.0 * single.duration_s()).abs() < 0.002,
        "the duration is the sum: {} against {}",
        joined.file.duration_s(),
        single.duration_s()
    );
    assert!(
        joined
            .file
            .frames
            .windows(2)
            .all(|w| w[0].time_ms <= w[1].time_ms),
        "the times never go back"
    );
    let second = &joined.parts[1];
    assert!((second.start_s - joined.parts[0].end_s).abs() < 0.002);
    // The second file shows the same screens as the first, one file later.
    let offset = single.frames.len();
    assert_eq!(joined.file.frames[offset].cells, single.frames[0].cells);
}

#[test]
fn the_sound_and_the_overlay_agree_about_where_the_second_file_starts() {
    let osd = common::fixture("clip-3s.osd");
    let paths = vec![osd.clone(), osd];
    let from_files = Recording::load_joined(&paths, LoadOptions::default(), 0.0).unwrap();
    let joined = join(twice(), 0.0).unwrap();
    let from_join = Recording::decode(&joined.file, LoadOptions::default()).unwrap();

    assert_eq!(from_files.frames, from_join.frames);
    let a = &from_files.segments;
    assert_eq!(a.len(), 2);
    let second = joined.parts[1].start_s;
    assert!(
        (a[1].start - second).abs() < 0.002,
        "the telemetry starts the second file at {} s and the overlay at {second} s",
        a[1].start
    );
}

#[test]
fn coverage_passes_for_the_whole_video_and_flags_a_missing_file() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let video = joined_video(&tools, dir.path());
    let info = probe(&tools, &video).unwrap();
    assert!((info.duration_s - 6.0).abs() < 0.2, "{}", info.duration_s);

    let both = join(twice(), 0.0).unwrap();
    assert_eq!(Coverage::new(&both.parts, info.duration_s).warning(), None);

    let one = join(twice().into_iter().take(1).collect(), 0.0).unwrap();
    let warning = Coverage::new(&one.parts, info.duration_s)
        .warning()
        .expect("half of the video has no OSD");
    assert!(warning.contains("missing"), "{warning}");
}

#[test]
fn a_joined_video_is_rendered_with_the_osd_and_the_sound_of_both_files() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let video = joined_video(&tools, dir.path());
    let info = probe(&tools, &video).unwrap();
    let joined = join(twice(), 0.0).unwrap();

    let plan = RenderPlan {
        video,
        info,
        osd: Some(joined.file),
        osd_path: Some(common::fixture("clip-3s.osd")),
        osd_layer: Some(OsdLayer {
            font: common::synthetic_sheet(CharacterSize::Small),
            options: OsdOptions::default(),
        }),
        srt_layer: None,
        chroma_key: None,
        sound: Some(SoundSettings::default()),
        sync: SyncOptions::default(),
        window: None,
        output: OutputPlan {
            path: dir.path().join("out.mp4"),
            video: VideoOutput::Encode(EncoderChoice {
                name: "libx264".into(),
                extra_args: vec!["-preset".into(), "ultrafast".into()],
                bitrate_mbps: 8,
                upscale_1440p: false,
            }),
            audio_bitrate_kbps: 192,
            faststart: true,
            overwrite: Overwrite::Refuse,
            replace_audio: false,
        },
    };
    let outcome = render(&tools, &plan, &CancelToken::new(), &mut |_| {}).unwrap();

    let check = outcome.check.expect("the output was checked");
    assert!(check.frames >= 350, "{check:?}");
    assert!(
        (check.audio_duration_s.unwrap() - check.video_duration_s).abs() < 0.05,
        "{check:?}"
    );
    assert!(
        outcome.warnings.iter().all(|w| !w.contains("OSD")),
        "no synchronisation warning: {:?}",
        outcome.warnings
    );
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
