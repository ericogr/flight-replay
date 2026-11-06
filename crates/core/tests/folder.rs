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
    // `a` is complete; `b` has neither an .osd nor an .srt; `c` has its output already.
    let dir = folder(&["a", "b", "c"], &["a", "c"]);
    fs::write(dir.path().join("c_flight-replay.mp4"), b"already here").unwrap();
    let mut defaults = defaults();
    defaults.settings.draw_srt = true;

    let plans = plan_folder(&tools, dir.path(), &defaults, &[]).unwrap();

    let planned: Vec<_> = plans.plans.iter().map(|p| p.video.clone()).collect();
    assert_eq!(names(&planned), ["a.mp4"]);
    assert_eq!(
        plans.skipped.len(),
        2,
        "the output of an earlier run is not a video to plan"
    );
    let reasons: Vec<_> = plans
        .skipped
        .iter()
        .map(|s| (names(std::slice::from_ref(&s.video))[0].clone(), &s.reason))
        .collect();
    assert!(
        matches!(reasons[0], (ref n, FlightError::NothingToAdd(left_out)) if n == "b.mp4" && left_out.len() == 3),
        "{reasons:?}"
    );
    assert!(
        matches!(reasons[1], (ref n, FlightError::OutputExists) if n == "c.mp4"),
        "{reasons:?}"
    );
    let described = plans.skipped[0].describe();
    assert!(
        described.starts_with("b.mp4: nothing can be added. Left out: the OSD."),
        "{described}"
    );
}

#[test]
fn a_video_is_planned_with_what_its_files_give() {
    let Some(tools) = tools() else { return };
    // `a` has an .osd and no .srt; `b` has an .srt and no .osd.
    let dir = folder(&["a", "b"], &[]);
    fs::copy(common::fixture("clip-3s.osd"), dir.path().join("a.osd")).unwrap();
    fs::copy(common::fixture("clip-3s.srt"), dir.path().join("b.srt")).unwrap();
    let mut defaults = defaults();
    defaults.settings.draw_srt = true;

    let plans = plan_folder(&tools, dir.path(), &defaults, &[]).unwrap();

    assert!(plans.skipped.is_empty(), "{:?}", plans.skipped);
    let (a, b) = (&plans.plans[0], &plans.plans[1]);
    assert!(a.osd_layer.is_some() && a.sound.is_some() && a.srt_layer.is_none());
    assert!(b.srt_layer.is_some() && b.osd_layer.is_none() && b.sound.is_none());
    let left_out: Vec<_> = plans
        .left_out
        .iter()
        .map(|l| {
            (
                names(std::slice::from_ref(&l.video))[0].clone(),
                l.parts.len(),
            )
        })
        .collect();
    assert_eq!(left_out, [("a.mp4".to_owned(), 1), ("b.mp4".to_owned(), 2)]);
    assert!(
        plans.left_out[0].describe().contains("the SRT data"),
        "{}",
        plans.left_out[0].describe()
    );
}

#[test]
fn the_sound_is_left_out_when_the_firmware_has_no_telemetry_or_the_throttle_is_not_shown() {
    let Some(tools) = tools() else { return };
    let dir = folder(&["betaflight", "inav"], &[]);
    let mut osd = fs::read(common::fixture("clip-3s.osd")).unwrap();
    fs::write(dir.path().join("inav.osd"), &osd).unwrap();
    osd[..4].copy_from_slice(b"BTFL");
    fs::write(dir.path().join("betaflight.osd"), &osd).unwrap();

    let mut defaults = defaults();
    let plans = plan_folder(&tools, dir.path(), &defaults, &[]).unwrap();
    assert!(plans.skipped.is_empty(), "{:?}", plans.skipped);
    let (betaflight, inav) = (&plans.plans[0], &plans.plans[1]);
    assert!(
        betaflight.osd_layer.is_some(),
        "the OSD is drawn for any firmware"
    );
    assert!(
        betaflight.sound.is_none(),
        "there are no Betaflight rules yet"
    );
    assert!(inav.sound.is_some());
    assert_eq!(plans.left_out.len(), 1);

    // A column that is not in the recording stands for an OSD that does not show the throttle.
    defaults.settings.sound.columns.throttle = "osd_not_shown".into();
    let plans = plan_folder(&tools, dir.path(), &defaults, &[]).unwrap();
    assert!(plans.plans.iter().all(|p| p.sound.is_none()));
    assert_eq!(plans.left_out.len(), 2);
    let described = plans.left_out[1].describe();
    assert!(described.contains("the motor sound"), "{described}");
    assert!(described.contains("Throttle Position"), "{described}");
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
