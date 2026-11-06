// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Renders one frame of a recording with its OSD and SRT data, and saves it as PNG.
//!
//! ```text
//! cargo run -p flight-replay-core --example preview_frame -- \
//!     --video clip.mp4 --osd clip.osd --srt clip.srt --font WS_INAV_Europa_24.png --at 1.5 --out frame.png
//! ```

use std::path::PathBuf;

use ab_glyph::FontRef;
use flight_replay_core::assets;
use flight_replay_core::ffmpeg::{DecodeOptions, FrameReader, Tools, probe};
use flight_replay_core::font::{CharacterSize, FontSheet};
use flight_replay_core::osd::{OsdFile, OsdOptions};
use flight_replay_core::overlay::{draw_osd, draw_srt};
use flight_replay_core::srt::{SrtFile, SrtOptions};
use flight_replay_core::timeline::{SyncOptions, Timeline};
use image::RgbaImage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut video = None;
    let mut osd_path = None;
    let mut srt_path = None;
    let mut font_path = None;
    let mut at = 0.0f64;
    let mut out = PathBuf::from("frame.png");
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("missing value")?;
        match flag.as_str() {
            "--video" => video = Some(PathBuf::from(value)),
            "--osd" => osd_path = Some(PathBuf::from(value)),
            "--srt" => srt_path = Some(PathBuf::from(value)),
            "--font" => font_path = Some(PathBuf::from(value)),
            "--at" => at = value.parse()?,
            "--out" => out = PathBuf::from(value),
            other => return Err(format!("unknown option {other}").into()),
        }
    }
    let video = video.ok_or("--video is required")?;
    let tools = Tools::locate(None)?;
    let info = probe(&tools, &video)?;
    let osd = OsdFile::open(&osd_path.ok_or("--osd is required")?)?;
    let sheet = FontSheet::open(&font_path.ok_or("--font is required")?)?;
    let size = CharacterSize::for_video_height(info.height);
    let font = sheet.scaled(size);

    let timeline = Timeline::new(SyncOptions::default(), info.duration_s, osd.duration_s());
    let frame = timeline.osd_frame(&osd, at);
    println!(
        "video {}x{} {:.3} fps, osd {} ({} frames), scale {:.6}, osd frame at {} ms",
        info.width,
        info.height,
        info.frame_rate,
        osd.firmware,
        osd.frames.len(),
        timeline.scale(),
        frame.time_ms
    );

    let mut reader = FrameReader::open(
        &tools,
        &video,
        &DecodeOptions {
            start_s: at,
            duration_s: Some(1.0 / info.nominal_frame_rate),
            ..DecodeOptions::whole(info.width, info.height)
        },
    )?;
    let mut buffer = Vec::new();
    if !reader.read_frame(&mut buffer)? {
        return Err("no frame at that time".into());
    }
    reader.cancel();
    let mut canvas =
        RgbaImage::from_raw(info.width, info.height, buffer).ok_or("bad frame size")?;

    draw_osd(&mut canvas, frame, &font, &OsdOptions::default());

    if let Some(path) = srt_path {
        let srt = SrtFile::open(&path)?;
        if let Some(data) = srt.cue_at(at).and_then(|cue| cue.data.as_ref()) {
            let font = FontRef::try_from_slice(assets::SRT_FONT)?;
            let options = SrtOptions {
                show_time: true,
                show_sky_battery: true,
                show_ground_battery: true,
                ..SrtOptions::default()
            };
            draw_srt(&mut canvas, data, &font, &options);
        }
    }

    canvas.save(&out)?;
    println!("wrote {}", out.display());
    Ok(())
}
