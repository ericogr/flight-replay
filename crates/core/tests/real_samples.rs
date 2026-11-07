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
        // The Go program decodes 32 columns here. This decoder reads 25: six of the Go columns (the ground speed,
        // the power and four raw ones) only ever came from the statistics screen shown after disarming, which this
        // decoder does not read (INAV 7 titles it `--- STATS ---`, which the Go program does not recognise, and then
        // takes its maximum and average for readings of the flight). One more column that the Go program has was
        // already missing before this was changed.
        assert_eq!(recording.table.header.len(), 25);

        // column, readings, minimum, maximum, mean. The Go program had 4789 readings, up to 36.70, mean 7.91: the
        // extra 22 are "MAX CURRENT" of the statistics screen.
        let expected = [
            ("osd_current_a", 4767, 0.74, 36.50, 7.78),
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
        // The throttle is the one column that reads more than the Go program: where INAV shows STOP, IDLE or DARM
        // (or a negative percentage) instead of a percentage, this decoder reads 0 (or the negative number) where
        // Go reads nothing (see docs/telemetry/inav.md). The Go program counted 5005 readings, from 3 to 59 with a
        // mean of 36.86. This one keeps all of those and adds the others, so the count cannot be smaller, the
        // minimum cannot be above 3, the maximum is the same, and the mean can only go down (it is not checked).
        let throttle = recording.table.series("osd_throttle_pct").unwrap().stats;
        assert!(
            throttle.valid >= 5005,
            "throttle readings {}",
            throttle.valid
        );
        assert!(throttle.min <= 3.0, "throttle min {}", throttle.min);
        assert!(
            (throttle.max - 59.0).abs() < 0.005,
            "throttle max {}",
            throttle.max
        );
        // The statistics screen of this recording (INAV 7) was the only place that showed a ground speed: 22 readings
        // of 63 km/h, the average of the flight, which the Go program takes as the speed of the aircraft for the
        // whole video. Without it there is no speed here.
        for column in [
            "osd_gps_speed_kmh",
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
        // 25 columns without the layout detection, 27 with it (flight mode and message)
        assert_eq!(recording.table.header.len(), 27);
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

mod sound {
    use std::path::PathBuf;
    use std::process::Command;

    use flight_replay_core::synth::{Engine, SignalSet, SoundSettings};
    use flight_replay_core::telemetry::{LoadOptions, Recording};
    use flight_replay_core::timeline::{ScaleMode, SyncOptions, Timeline};

    use super::samples;

    /// The lines `motorsound --dry-run -v AvatarG0153.mp4 AvatarG0153.osd` prints about the signals.
    #[test]
    fn the_signal_report_matches_the_go_dry_run() {
        let Some(dir) = samples() else { return };
        let recording =
            Recording::load(&dir.join("AvatarG0153.osd"), LoadOptions::default()).unwrap();
        let signals = SignalSet::collect(
            &recording.table,
            &SoundSettings::default(),
            "AvatarG0153.osd",
            recording.frames,
        )
        .unwrap();
        // The Go program reports the ground speed here too, from the 22 readings of the statistics screen (see
        // `the_signal_statistics_match_the_go_dry_run`). This decoder has none, so the wind has nothing to follow.
        assert_eq!(
            signals.describe(),
            "found throttle, current, cell_voltage; not in the .osd: air_speed, gps_speed, gforce, roll, pitch"
        );
        assert_eq!(
            signals.warnings,
            vec![
                "no airspeed or ground speed readings in AvatarG0153.osd (osd_air_speed_kmh, osd_gps_speed_kmh): the wind sound will be off",
                "no roll readings in AvatarG0153.osd (osd_roll_deg): the wind will not pan between left and right with the bank angle",
                "no pitch readings in AvatarG0153.osd (osd_pitch_deg): the load will not follow the climb angle",
            ]
        );
    }

    /// Renders ten seconds from the second 120 of `AvatarG0153` with the Go motorsound (named by
    /// `FLIGHT_REPLAY_MOTORSOUND`) and with the Rust engine, on the same telemetry and settings, and compares the
    /// samples.
    ///
    /// The wind is off on both sides. The Go program takes the 63 km/h average of the statistics screen of this
    /// INAV 7 recording for the speed of the aircraft, and makes a wind of it for the whole video, which this
    /// decoder does not (there is no speed in this recording once that screen is left out). What is compared is the
    /// motor and the propeller.
    #[test]
    fn the_audio_matches_the_go_program_sample_by_sample() {
        let Some(dir) = samples() else { return };
        let Some(go) = std::env::var_os("FLIGHT_REPLAY_MOTORSOUND").map(PathBuf::from) else {
            eprintln!("skipping: FLIGHT_REPLAY_MOTORSOUND is not set");
            return;
        };
        let temp = tempfile::tempdir().unwrap();
        let reference = temp.path().join("go.wav");
        let status = Command::new(&go)
            .args([
                "--quiet",
                "--force",
                "--set",
                "wind.gain=0",
                "--start",
                "00:02:00",
                "--duration",
                "10s",
                "--time-scale",
                "1.000059",
                "-o",
            ])
            .arg(&reference)
            .arg(dir.join("AvatarG0153.mp4"))
            .arg(dir.join("AvatarG0153.osd"))
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let wav = std::fs::read(&reference).unwrap();
        let expected: Vec<i16> = wav[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b))
            .collect();

        let recording =
            Recording::load(&dir.join("AvatarG0153.osd"), LoadOptions::default()).unwrap();
        let mut settings = SoundSettings::default();
        settings.wind.gain = 0.0;
        let signals = SignalSet::collect(
            &recording.table,
            &settings,
            "AvatarG0153.osd",
            recording.frames,
        )
        .unwrap();
        let sync = SyncOptions {
            scale: ScaleMode::Manual { scale: 1.000_059 },
            offset_s: 0.0,
        };
        let timeline = Timeline::new(sync, 846.1, 845.893);
        let mut engine = Engine::new(settings, signals.controls(), timeline, 120.0).unwrap();
        let got = engine.render_s16(480_000);

        assert_eq!(got.len(), expected.len(), "the same number of samples");
        let differences: Vec<u32> = got
            .iter()
            .zip(&expected)
            .map(|(a, b)| u32::from(a.abs_diff(*b)))
            .collect();
        let worst = *differences.iter().max().unwrap();
        let different = differences.iter().filter(|&&d| d > 0).count();
        eprintln!(
            "worst difference {worst} LSB; {different} of {} samples differ",
            got.len()
        );
        assert!(
            worst <= 2,
            "the Rust audio differs from the Go audio by up to {worst} LSB"
        );
        let render_stats = engine.stats();
        // the Go program reported, with the wind off: peak -8.4 dBFS, rms -18.6 dBFS
        assert!(
            (20.0 * render_stats.peak.log10() + 8.4).abs() < 0.1,
            "peak {} dBFS",
            20.0 * render_stats.peak.log10()
        );
        assert!(
            (20.0 * render_stats.rms.log10() + 18.6).abs() < 0.1,
            "rms {} dBFS",
            20.0 * render_stats.rms.log10()
        );
    }
}
