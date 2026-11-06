// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Checks against the real Walksnail files of the owner. They are not in the repository. The tests run when the
//! environment variable `FLIGHT_REPLAY_SAMPLES` names the folder of the recordings, which holds a `userfont`
//! folder and the `.osd` and `.srt` files. Without it, each test prints a message and passes.

use std::path::PathBuf;

use flight_replay_core::font::{CharacterSize, FontLayout, FontSheet};
use flight_replay_core::osd::{Firmware, OsdFile};
use flight_replay_core::srt::SrtFile;

fn samples() -> Option<PathBuf> {
    let dir = std::env::var_os("FLIGHT_REPLAY_SAMPLES").map(PathBuf::from);
    if dir.is_none() {
        eprintln!("skipping: FLIGHT_REPLAY_SAMPLES is not set");
    }
    dir
}

#[test]
fn the_goggle_font_sheets_have_the_expected_layouts() {
    let Some(dir) = samples() else { return };
    let fonts = dir.join("userfont");
    let expected = [
        ("WS_INAV_Europa_24.png", CharacterSize::Small, 1, 512),
        ("WS_INAV_Europa_36.png", CharacterSize::Large, 1, 512),
        ("WS_BFx4_Europa_24.png", CharacterSize::Small, 4, 256),
        ("WS_ARDU_Europa_24.png", CharacterSize::Small, 1, 256),
    ];
    for (name, size, pages, characters_per_page) in expected {
        let sheet = FontSheet::open(&fonts.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            sheet.layout(),
            FontLayout {
                size,
                pages,
                characters_per_page
            },
            "{name}"
        );
    }
}

#[test]
fn an_inav_recording_parses_with_its_duration() {
    let Some(dir) = samples() else { return };
    let osd = OsdFile::open(&dir.join("AvatarG0153.osd")).unwrap();
    assert_eq!(osd.firmware, Firmware::Inav);
    assert_eq!((osd.columns, osd.rows), (53, 20));
    assert_eq!(osd.frames.len(), 5372);
    assert!(
        (osd.duration_s() - 845.9).abs() < 0.2,
        "duration {}",
        osd.duration_s()
    );
}

#[test]
fn the_srt_of_a_recording_has_data_on_every_cue() {
    let Some(dir) = samples() else { return };
    let srt = SrtFile::open(&dir.join("AvatarG0153.srt")).unwrap();
    assert!(srt.cues.len() > 1000);
    let with_data = srt.cues.iter().filter(|cue| cue.data.is_some()).count();
    assert!(
        with_data as f64 > srt.cues.len() as f64 * 0.99,
        "{with_data} of {}",
        srt.cues.len()
    );
}

mod telemetry {
    use flight_replay_core::telemetry::{LoadOptions, ProbePosition, Recording, probe_at};
    use flight_replay_core::timeline::{ScaleMode, SyncOptions, Timeline};

    use super::samples;

    fn recording(detect_layout: bool) -> Option<Recording> {
        let dir = samples()?;
        Some(Recording::load(&dir.join("AvatarG0153.osd"), LoadOptions { detect_layout }).unwrap())
    }

    /// Numbers printed by `motorsound --dry-run -v AvatarG0153.mp4 AvatarG0153.osd` (the Go program).
    #[test]
    fn the_signal_statistics_match_the_go_dry_run() {
        let Some(recording) = recording(false) else {
            return;
        };
        assert_eq!(recording.frames, 5372);
        assert_eq!((recording.columns, recording.rows), (53, 20));
        // columns decoded: 32 without the layout detection, 34 with it (flight mode and message)
        assert_eq!(recording.table.header.len(), 32);

        // column, readings, minimum, maximum, mean
        let expected = [
            ("osd_throttle_pct", 5005, 3.0, 59.0, 36.86),
            ("osd_current_a", 4789, 0.74, 36.70, 7.91),
            ("osd_gps_speed_kmh", 22, 63.0, 63.0, 63.0),
            ("osd_batt_cell_v", 5322, 3.34, 3.92, 3.61),
        ];
        for (column, readings, min, max, mean) in expected {
            let stats = recording.table.series(column).unwrap().stats;
            assert_eq!(stats.valid, readings, "{column} readings");
            assert!(
                (stats.min - min).abs() < 0.005,
                "{column} min {}",
                stats.min
            );
            assert!(
                (stats.max - max).abs() < 0.005,
                "{column} max {}",
                stats.max
            );
            assert!(
                (stats.mean - mean).abs() < 0.005,
                "{column} mean {}",
                stats.mean
            );
        }
        for column in [
            "osd_air_speed_kmh",
            "osd_gforce",
            "osd_roll_deg",
            "osd_pitch_deg",
        ] {
            assert!(
                !recording.table.has_column(column),
                "{column} is not in this recording"
            );
        }
    }

    #[test]
    fn the_layout_detection_finds_the_flight_mode_and_the_message_row() {
        let Some(recording) = recording(true) else {
            return;
        };
        assert_eq!(recording.table.header.len(), 34);
        assert_eq!(
            recording.layout,
            "flight mode at 45,16,4; status messages on row 13"
        );
    }

    /// Frames and values printed by `motorsound probe --at 120,300,600 --time-scale 1.000059` (the Go program).
    #[test]
    fn the_probe_picks_the_same_frames_as_the_go_probe() {
        let Some(recording) = recording(true) else {
            return;
        };
        let sync = SyncOptions {
            scale: ScaleMode::Manual { scale: 1.000_059 },
            offset_s: 0.0,
        };
        let timeline = Timeline::new(sync, 846.1, 845.893);
        let columns = [
            "osd_flight_time_s",
            "osd_throttle_pct",
            "osd_current_a",
            "osd_altitude_m",
            "osd_mah_used",
            "osd_flight_mode",
        ];

        let expected = [
            (
                120.0,
                802,
                119_996.0,
                "osd_flight_time_s=111  osd_throttle_pct=34  osd_current_a=6.51  osd_altitude_m=234  osd_mah_used=477  osd_flight_mode=ACRO",
            ),
            (
                300.0,
                1729,
                299_981.0,
                "osd_flight_time_s=291  osd_throttle_pct=42  osd_current_a=11.6*  osd_altitude_m=134  osd_mah_used=856  osd_flight_mode=ACRO",
            ),
            (
                600.0,
                3730,
                600_026.0,
                "osd_flight_time_s=591  osd_throttle_pct=36  osd_current_a=7.55  osd_altitude_m=512  osd_mah_used=1546  osd_flight_mode=ACRO",
            ),
        ];
        for (video_time, row, row_ms, values) in expected {
            let report = probe_at(&recording.table, &timeline, video_time, &columns);
            let ProbePosition::Inside {
                row: got_row,
                row_time,
            } = report.position
            else {
                panic!("{video_time} s is outside the recording");
            };
            assert_eq!(got_row, row, "frame at {video_time} s");
            assert!(
                (row_time * 1000.0 - row_ms).abs() < 0.5,
                "frame time at {video_time} s"
            );
            assert_eq!(report.values_text(), values, "values at {video_time} s");
        }
    }
}
