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
