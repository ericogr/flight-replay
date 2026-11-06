// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Planning the videos of a folder, on copies of the fixture clip. Each test returns early, with a message, when
//! FFmpeg is not installed.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use flight_replay_core::config::Settings;
use flight_replay_core::ffmpeg::Tools;
use flight_replay_core::font::CharacterSize;
use flight_replay_core::render::{FlightDefaults, FlightError, plan_folder};
use flight_replay_core::timeline::SyncOptions;

fn tools() -> Option<Tools> {
    match Tools::locate(None) {
        Ok(tools) => Some(tools),
        Err(error) => {
            eprintln!("skipping FFmpeg test: {error}");
            None
        }
    }
}

/// A folder with `names` as copies of the clip, with the `.osd` and `.srt` where `with` says.
fn folder(names: &[&str], with: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in names {
        fs::copy(
            common::fixture("clip-3s.mp4"),
            dir.path().join(format!("{name}.mp4")),
        )
        .unwrap();
    }
    for name in with {
        for extension in ["osd", "srt"] {
            fs::copy(
                common::fixture(&format!("clip-3s.{extension}")),
                dir.path().join(format!("{name}.{extension}")),
            )
            .unwrap();
        }
    }
    dir
}

fn defaults() -> FlightDefaults {
    FlightDefaults {
        settings: Settings::default(),
        encoders: Vec::new(),
        font: Some(common::synthetic_sheet(CharacterSize::Small)),
        sync: SyncOptions::default(),
        overwrite: false,
        replace_audio: false,
    }
}

fn names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn every_video_with_its_files_gets_a_plan_next_to_it() {
    let Some(tools) = tools() else { return };
    let dir = folder(&["a", "b"], &["a", "b"]);
    let plans = plan_folder(&tools, dir.path(), &defaults(), &[]).unwrap();

    assert!(plans.skipped.is_empty(), "{:?}", plans.skipped);
    let outputs: Vec<_> = plans.plans.iter().map(|p| p.output.path.clone()).collect();
    assert_eq!(
        names(&outputs),
        ["a_flight-replay.mp4", "b_flight-replay.mp4"]
    );
    assert!(
        plans
            .plans
            .iter()
            .all(|p| p.osd.is_some() && p.osd_layer.is_some())
    );
}

#[test]
fn a_video_that_cannot_be_planned_is_left_out_with_its_reason() {
    let Some(tools) = tools() else { return };
    // `a` is complete; `b` has no .osd; `c` has its output already; `d` has no .srt and the SRT data are wanted.
    let dir = folder(&["a", "b", "c", "d"], &["a", "c"]);
    fs::write(dir.path().join("c_flight-replay.mp4"), b"already here").unwrap();
    fs::copy(common::fixture("clip-3s.osd"), dir.path().join("d.osd")).unwrap();
    let mut defaults = defaults();
    defaults.settings.draw_srt = true;

    let plans = plan_folder(&tools, dir.path(), &defaults, &[]).unwrap();

    let planned: Vec<_> = plans.plans.iter().map(|p| p.video.clone()).collect();
    assert_eq!(names(&planned), ["a.mp4"]);
    assert_eq!(
        plans.skipped.len(),
        3,
        "the output of an earlier run is not a video to plan"
    );
    let reasons: Vec<_> = plans
        .skipped
        .iter()
        .map(|s| (names(std::slice::from_ref(&s.video))[0].clone(), &s.reason))
        .collect();
    assert!(
        matches!(reasons[0], (ref n, FlightError::NoOsd) if n == "b.mp4"),
        "{reasons:?}"
    );
    assert!(
        matches!(reasons[1], (ref n, FlightError::OutputExists) if n == "c.mp4"),
        "{reasons:?}"
    );
    assert!(
        matches!(reasons[2], (ref n, FlightError::NoSrt) if n == "d.mp4"),
        "{reasons:?}"
    );
    assert_eq!(
        plans.skipped[0].describe(),
        "b.mp4: there is no .osd file next to it"
    );
}

#[test]
fn an_existing_output_is_planned_again_when_replacing_is_allowed() {
    let Some(tools) = tools() else { return };
    let dir = folder(&["a"], &["a"]);
    fs::write(dir.path().join("a_flight-replay.mp4"), b"already here").unwrap();
    let mut defaults = defaults();
    defaults.overwrite = true;

    let plans = plan_folder(&tools, dir.path(), &defaults, &[]).unwrap();
    assert_eq!(plans.plans.len(), 1, "{:?}", plans.skipped);
}

#[test]
fn an_output_that_another_plan_writes_is_left_out() {
    let Some(tools) = tools() else { return };
    let dir = folder(&["a", "b"], &["a", "b"]);
    let taken: &[PathBuf] = &[Path::new(dir.path()).join("a_flight-replay.mp4")];

    let plans = plan_folder(&tools, dir.path(), &defaults(), taken).unwrap();
    assert_eq!(plans.plans.len(), 1);
    assert!(matches!(
        plans.skipped[0].reason,
        FlightError::AlreadyPlanned
    ));
}
