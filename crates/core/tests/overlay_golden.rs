// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The OSD and the SRT data drawn on a flat picture, compared with golden images.

mod common;

use ab_glyph::FontRef;
use flight_replay_core::assets::SRT_FONT;
use flight_replay_core::font::CharacterSize;
use flight_replay_core::osd::{OsdFile, OsdOptions};
use flight_replay_core::overlay::{DEFAULT_CHROMA_KEY, chroma_background, draw_osd, draw_srt};
use flight_replay_core::srt::{SrtFile, SrtOptions};
use image::{Rgba, RgbaImage};

fn clip_osd() -> OsdFile {
    OsdFile::open(&common::fixture("clip-3s.osd")).unwrap()
}

#[test]
fn an_osd_frame_with_the_srt_data_matches_its_golden_image() {
    let osd = clip_osd();
    let srt = SrtFile::open(&common::fixture("clip-3s.srt")).unwrap();
    let font = common::synthetic_font(CharacterSize::Small);
    let srt_font = FontRef::try_from_slice(SRT_FONT).unwrap();

    let mut canvas = RgbaImage::from_pixel(1280, 720, Rgba([60, 70, 80, 255]));
    draw_osd(&mut canvas, &osd.frames[10], &font, &OsdOptions::default());
    let data = srt
        .cue_at(1.5)
        .and_then(|cue| cue.data.as_ref())
        .expect("a cue with data");
    let options = SrtOptions {
        show_time: true,
        show_sky_battery: true,
        show_ground_battery: true,
        ..SrtOptions::default()
    };
    draw_srt(&mut canvas, data, &srt_font, &options);

    common::assert_golden("overlay-frame.png", &canvas, 2);
}

#[test]
fn a_masked_and_moved_osd_matches_its_golden_image() {
    let osd = clip_osd();
    let font = common::synthetic_font(CharacterSize::Small);
    let mut options = OsdOptions::default();
    options.center_on(&osd.frames[10], 1280, 720, &CharacterSize::Small);
    options.x_offset += 40;
    // hide the first ten columns of the top rows
    for x in 0..10 {
        for y in 0..3 {
            options.toggle_mask(x, y);
        }
    }
    let mut canvas = chroma_background(1280, 720, DEFAULT_CHROMA_KEY);
    draw_osd(&mut canvas, &osd.frames[10], &font, &options);
    common::assert_golden("osd-moved-masked-chroma.png", &canvas, 2);
}
