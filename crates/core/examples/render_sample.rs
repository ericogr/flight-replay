// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Renders a stretch of a real recording with the OSD, the SRT data and the sound, and prints the time it took.
//! A developer tool to measure the speed of the pipeline.
//!
//! ```text
//! cargo run --release -p flight-replay-core --example render_sample -- \
//!     --video a.mp4 --osd a.osd --srt a.srt --font WS_INAV_Europa_24.png \
//!     --start 120 --duration 30 --encoder libx264 --out out.mp4
//! ```

use std::path::PathBuf;
use std::time::Instant;

use flight_replay_core::cancel::CancelToken;
use flight_replay_core::ffmpeg::{Tools, probe};
use flight_replay_core::font::FontSheet;
use flight_replay_core::osd::{OsdFile, OsdOptions};
use flight_replay_core::render::{
    EncoderChoice, Event, OsdLayer, OutputPlan, Overwrite, RenderPlan, SrtLayer, VideoOutput,
    Window, render,
};
use flight_replay_core::srt::{SrtFile, SrtOptions};
use flight_replay_core::synth::SoundSettings;
use flight_replay_core::timeline::SyncOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (mut video, mut osd, mut srt, mut font, mut out) =
        (None, None, None, None, PathBuf::from("out.mp4"));
    let (mut start, mut duration, mut encoder, mut sound) = (0.0, None, "libx264".to_owned(), true);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("missing value")?;
        match flag.as_str() {
            "--video" => video = Some(PathBuf::from(value)),
            "--osd" => osd = Some(PathBuf::from(value)),
            "--srt" => srt = Some(PathBuf::from(value)),
            "--font" => font = Some(PathBuf::from(value)),
            "--out" => out = PathBuf::from(value),
            "--start" => start = value.parse()?,
            "--duration" => duration = Some(value.parse()?),
            "--encoder" => encoder = value,
            "--sound" => sound = value == "yes",
            other => return Err(format!("unknown option {other}").into()),
        }
    }
    let video = video.ok_or("--video is required")?;
    let osd_path = osd.ok_or("--osd is required")?;
    let tools = Tools::locate(None)?;
    let info = probe(&tools, &video)?;

    let plan = RenderPlan {
        video,
        info,
        osd: Some(OsdFile::open(&osd_path)?),
        osd_path: Some(osd_path),
        osd_layer: Some(OsdLayer {
            font: FontSheet::open(&font.ok_or("--font is required")?)?,
            options: OsdOptions::default(),
        }),
        srt_layer: match srt {
            Some(path) => Some(SrtLayer {
                file: SrtFile::open(&path)?,
                options: SrtOptions::default(),
            }),
            None => None,
        },
        chroma_key: None,
        sound: sound.then(SoundSettings::default),
        sync: SyncOptions::default(),
        window: Some(Window {
            start_s: start,
            duration_s: duration,
        }),
        output: OutputPlan {
            path: out,
            video: VideoOutput::Encode(EncoderChoice {
                name: encoder,
                extra_args: Vec::new(),
                bitrate_mbps: 40,
                upscale_1440p: false,
            }),
            audio_bitrate_kbps: 192,
            faststart: true,
            overwrite: Overwrite::Replace,
            replace_audio: true,
        },
    };

    let started = Instant::now();
    let outcome = render(&tools, &plan, &CancelToken::new(), &mut |event| {
        if let Event::Warning(text) = event {
            eprintln!("warning: {text}");
        }
    })?;
    let seconds = started.elapsed().as_secs_f64();
    let frames = outcome.check.as_ref().map_or(0, |c| c.frames);
    println!(
        "{} s of video ({frames} frames) in {seconds:.1} s: {:.1}x real time, {:.0} frames per second",
        outcome.window.duration_s,
        outcome.window.duration_s / seconds,
        frames as f64 / seconds
    );
    println!("wrote {}", outcome.output.display());
    Ok(())
}
