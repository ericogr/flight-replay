// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! What the committed fixture clip holds. These tests need nothing outside the repository.

mod common;

use flight_replay_core::ffmpeg::{Tools, probe};
use flight_replay_core::osd::{Firmware, OsdFile};
use flight_replay_core::srt::SrtFile;
use flight_replay_core::telemetry::catalog::{KEY_GPS_LAT, KEY_GPS_LON, KEY_THROTTLE_PCT};
use flight_replay_core::telemetry::{LoadOptions, Recording};
use flight_replay_core::timeline::{SyncOptions, Timeline};

#[test]
fn the_osd_of_the_clip_parses() {
    let osd = OsdFile::open(&common::fixture("clip-3s.osd")).unwrap();
    assert_eq!(osd.firmware, Firmware::Inav);
    assert_eq!((osd.columns, osd.rows), (53, 20));
    assert_eq!(osd.frames.len(), 22);
    assert_eq!(osd.frames[0].time_ms, 0);
    assert!(
        osd.duration_s() > 2.9 && osd.duration_s() < 3.3,
        "{}",
        osd.duration_s()
    );
}

#[test]
fn the_clip_has_a_flight_in_progress() {
    let recording =
        Recording::load(&common::fixture("clip-3s.osd"), LoadOptions::default()).unwrap();
    let throttle = recording.table.series(KEY_THROTTLE_PCT).unwrap();
    assert!(
        throttle.stats.min > 0.0 && throttle.stats.max < 100.0,
        "{:?}",
        throttle.stats
    );
}

/// The fixture must never hold a real position. The positions were moved to a fake point in the South Atlantic by
/// `cargo xtask fixtures`, and this test fails if a position anywhere else gets into the repository.
#[test]
fn the_positions_in_the_clip_are_the_fake_ones_in_the_south_atlantic() {
    let recording =
        Recording::load(&common::fixture("clip-3s.osd"), LoadOptions::default()).unwrap();
    for (key, low, high) in [(KEY_GPS_LAT, -36.0, -34.0), (KEY_GPS_LON, -33.0, -30.0)] {
        let series = recording
            .table
            .series(key)
            .expect("the clip shows the position");
        assert!(
            series.stats.min >= low && series.stats.max <= high,
            "{key} is outside the fake area: {} to {}",
            series.stats.min,
            series.stats.max
        );
    }
}

#[test]
fn the_srt_of_the_clip_has_data_on_every_cue() {
    let srt = SrtFile::open(&common::fixture("clip-3s.srt")).unwrap();
    assert_eq!(srt.cues.len(), 21);
    assert!(srt.cues.iter().all(|cue| cue.data.is_some()));
    assert!(srt.cues[0].start_s.abs() < 1e-9);
}

#[test]
fn the_osd_and_the_video_of_the_clip_line_up() {
    let Ok(tools) = Tools::locate(None) else {
        eprintln!("skipping: FFmpeg is not installed");
        return;
    };
    let video = probe(&tools, &common::fixture("clip-3s.mp4")).unwrap();
    let osd = OsdFile::open(&common::fixture("clip-3s.osd")).unwrap();
    let timeline = Timeline::new(SyncOptions::default(), video.duration_s, osd.duration_s());
    assert!(
        (timeline.scale() - 1.0).abs() < 0.05,
        "scale {}",
        timeline.scale()
    );
    // the last OSD frame is shown before the end of the video
    assert!(timeline.video_time(f64::from(osd.frames[21].time_ms) / 1000.0) < video.duration_s);
}
