// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The preview picture: one frame of the video with the OSD and the SRT data drawn on it, as the render will draw
//! them.

use std::path::PathBuf;
use std::sync::Arc;

use ab_glyph::FontRef;
use flight_replay_core::assets::SRT_FONT;
use flight_replay_core::ffmpeg::{DecodeOptions, FrameReader, Tools, VideoInfo};
use flight_replay_core::font::ScaledFont;
use flight_replay_core::osd::{OsdFile, OsdOptions};
use flight_replay_core::overlay::{draw_osd, draw_srt};
use flight_replay_core::srt::{SrtFile, SrtOptions};
use flight_replay_core::timeline::Timeline;
use image::{Rgba, RgbaImage, imageops};

/// The widest picture that is shown. A wider frame is scaled down, so the preview stays fast at 4K and the options
/// of the step fit below it in the window.
pub const MAX_WIDTH: u32 = 640;

/// The OSD layer of a preview.
#[derive(Debug, Clone)]
pub struct OsdPreview {
    pub file: Arc<OsdFile>,
    pub font: Arc<ScaledFont>,
    pub options: OsdOptions,
}

/// The SRT layer of a preview.
#[derive(Debug, Clone)]
pub struct SrtPreview {
    pub file: Arc<SrtFile>,
    pub options: SrtOptions,
}

/// What to draw.
#[derive(Debug, Clone)]
pub struct PreviewRequest {
    pub sequence: u64,
    pub tools: Arc<Tools>,
    pub video: PathBuf,
    pub info: VideoInfo,
    pub time_s: f64,
    /// The frame of an earlier preview, when it is from the same time. It saves decoding the video again.
    pub base: Option<(f64, Arc<RgbaImage>)>,
    pub osd: Option<OsdPreview>,
    pub srt: Option<SrtPreview>,
    pub timeline: Option<Timeline>,
    /// Show the grid of the OSD, with the hidden cells marked.
    pub show_grid: bool,
}

/// The answer to a [`PreviewRequest`].
#[derive(Debug)]
pub struct PreviewResult {
    /// The frame without anything drawn on it, to keep for the next request.
    pub base: (f64, Arc<RgbaImage>),
    /// The picture to show.
    pub shown: RgbaImage,
}

/// Makes the preview picture. It decodes a frame if needed, so it must not run on the thread of the interface.
pub fn make(request: &PreviewRequest) -> Result<PreviewResult, String> {
    let base = match &request.base {
        Some((time, frame)) if (*time - request.time_s).abs() < 1e-9 => Arc::clone(frame),
        _ => Arc::new(decode_frame(request)?),
    };

    let mut canvas = (*base).clone();
    if let Some(osd) = &request.osd
        && let Some(timeline) = request.timeline
    {
        let telemetry_ms = timeline.telemetry_time(request.time_s) * 1000.0;
        let frame = &osd.file.frames[osd.file.frame_index_at(telemetry_ms)];
        draw_osd(&mut canvas, frame, &osd.font, &osd.options);
        if request.show_grid {
            draw_grid(&mut canvas, &osd.file, &osd.font, &osd.options);
        }
    }
    if let Some(srt) = &request.srt
        && let Some(data) = srt
            .file
            .cue_at(request.time_s)
            .and_then(|cue| cue.data.as_ref())
    {
        let font = FontRef::try_from_slice(SRT_FONT).map_err(|e| e.to_string())?;
        draw_srt(&mut canvas, data, &font, &srt.options);
    }

    let shown = if canvas.width() > MAX_WIDTH {
        let height =
            (u64::from(canvas.height()) * u64::from(MAX_WIDTH) / u64::from(canvas.width())) as u32;
        imageops::resize(
            &canvas,
            MAX_WIDTH,
            height.max(1),
            imageops::FilterType::Triangle,
        )
    } else {
        canvas
    };
    Ok(PreviewResult {
        base: (request.time_s, base),
        shown,
    })
}

fn decode_frame(request: &PreviewRequest) -> Result<RgbaImage, String> {
    let info = &request.info;
    let mut reader = FrameReader::open(
        &request.tools,
        &request.video,
        &DecodeOptions {
            width: info.width,
            height: info.height,
            start_s: request.time_s,
            duration_s: Some(2.0 / info.nominal_frame_rate),
            constant_rate: None,
        },
    )
    .map_err(|e| e.to_string())?;
    let mut buffer = Vec::new();
    let got = reader.read_frame(&mut buffer).map_err(|e| e.to_string())?;
    reader.cancel();
    if !got {
        return Err("the video has no frame at that time".to_owned());
    }
    RgbaImage::from_raw(info.width, info.height, buffer)
        .ok_or_else(|| "the frame has an unexpected size".to_owned())
}

/// Draws the grid of the OSD cells over the picture, and fills the hidden cells in red.
fn draw_grid(canvas: &mut RgbaImage, osd: &OsdFile, font: &ScaledFont, options: &OsdOptions) {
    let (cell_w, cell_h) = (
        i64::from(font.size().width()),
        i64::from(font.size().height()),
    );
    let (width, height) = (i64::from(canvas.width()), i64::from(canvas.height()));
    let mut blend = |x: i64, y: i64, color: [u8; 3], alpha: f32| {
        if x < 0 || y < 0 || x >= width || y >= height {
            return;
        }
        let pixel: &mut Rgba<u8> = canvas.get_pixel_mut(x as u32, y as u32);
        for channel in 0..3 {
            let under = f32::from(pixel[channel]);
            pixel[channel] =
                (under * (1.0 - alpha) + f32::from(color[channel]) * alpha).round() as u8;
        }
    };
    for row in 0..i64::from(osd.rows) {
        for column in 0..i64::from(osd.columns) {
            let left = column * cell_w + i64::from(options.x_offset);
            let top = row * cell_h + i64::from(options.y_offset);
            let masked = options.is_masked(column as u32, row as u32);
            for y in 0..cell_h {
                for x in 0..cell_w {
                    let edge = x == 0 || y == 0;
                    if masked {
                        blend(
                            left + x,
                            top + y,
                            [230, 40, 40],
                            if edge { 0.8 } else { 0.45 },
                        );
                    } else if edge {
                        blend(left + x, top + y, [255, 255, 255], 0.18);
                    }
                }
            }
        }
    }
}
