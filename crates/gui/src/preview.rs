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
        let frame = timeline.osd_frame(&osd.file, request.time_s);
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

/// Where the cells of the OSD are on the video picture. The grid is drawn with it, and the pointer is read with it,
/// so what is clicked is what is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    cell_width: u32,
    cell_height: u32,
    x_offset: i32,
    y_offset: i32,
    columns: u32,
    rows: u32,
}

impl Grid {
    pub fn new(osd: &OsdFile, font: &ScaledFont, options: &OsdOptions) -> Self {
        Self {
            cell_width: font.size().width(),
            cell_height: font.size().height(),
            x_offset: options.x_offset,
            y_offset: options.y_offset,
            columns: osd.columns,
            rows: osd.rows,
        }
    }

    /// The top left corner of a cell, in pixels of the video picture.
    fn corner(&self, column: u32, row: u32) -> (i64, i64) {
        (
            i64::from(column) * i64::from(self.cell_width) + i64::from(self.x_offset),
            i64::from(row) * i64::from(self.cell_height) + i64::from(self.y_offset),
        )
    }

    /// The cell at a point of the video picture, if the point is in the grid.
    pub fn cell_at(&self, x: f64, y: f64) -> Option<(u32, u32)> {
        let column = ((x - f64::from(self.x_offset)) / f64::from(self.cell_width)).floor();
        let row = ((y - f64::from(self.y_offset)) / f64::from(self.cell_height)).floor();
        let inside = column >= 0.0
            && row >= 0.0
            && column < f64::from(self.columns)
            && row < f64::from(self.rows);
        inside.then_some((column as u32, row as u32))
    }
}

/// A point of the picture as it is shown, in pixels of the video picture. The picture is shown smaller than the video.
/// `None` when nothing is shown yet.
pub fn video_point(point: (f32, f32), shown: (u32, u32), video: (u32, u32)) -> Option<(f64, f64)> {
    if shown.0 == 0 || shown.1 == 0 {
        return None;
    }
    Some((
        f64::from(point.0) / f64::from(shown.0) * f64::from(video.0),
        f64::from(point.1) / f64::from(shown.1) * f64::from(video.1),
    ))
}

/// Draws the grid of the OSD cells over the picture, and fills the hidden cells in red.
fn draw_grid(canvas: &mut RgbaImage, osd: &OsdFile, font: &ScaledFont, options: &OsdOptions) {
    let grid = Grid::new(osd, font, options);
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
    for row in 0..osd.rows {
        for column in 0..osd.columns {
            let (left, top) = grid.corner(column, row);
            let masked = options.is_masked(column, row);
            for y in 0..i64::from(grid.cell_height) {
                for x in 0..i64::from(grid.cell_width) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Grid {
        Grid {
            cell_width: 24,
            cell_height: 36,
            x_offset: 10,
            y_offset: -6,
            columns: 53,
            rows: 20,
        }
    }

    #[test]
    fn a_point_is_in_the_cell_that_is_drawn_there() {
        let grid = grid();
        for (column, row) in [(0, 0), (5, 3), (52, 19)] {
            let (left, top) = grid.corner(column, row);
            // Every corner of the cell, and its middle, reads as the same cell.
            for (dx, dy) in [(0.0, 0.0), (23.9, 0.0), (0.0, 35.9), (12.0, 18.0)] {
                assert_eq!(
                    grid.cell_at(left as f64 + dx, top as f64 + dy),
                    Some((column, row)),
                    "cell {column},{row} at +{dx},+{dy}"
                );
            }
        }
    }

    #[test]
    fn a_point_outside_the_grid_is_in_no_cell() {
        let grid = grid();
        assert_eq!(grid.cell_at(9.9, 100.0), None, "left of the first column");
        assert_eq!(grid.cell_at(100.0, -6.1), None, "above the first row");
        let (left, top) = grid.corner(53, 0);
        assert_eq!(
            grid.cell_at(left as f64, top as f64 + 1.0),
            None,
            "right of the last column"
        );
        let (left, top) = grid.corner(0, 20);
        assert_eq!(
            grid.cell_at(left as f64 + 1.0, top as f64),
            None,
            "below the last row"
        );
    }

    #[test]
    fn the_picture_shown_is_scaled_back_to_the_video() {
        // A 1280x720 video shown at 640x360.
        assert_eq!(
            video_point((100.0, 50.0), (640, 360), (1280, 720)),
            Some((200.0, 100.0))
        );
        assert_eq!(
            video_point((1.0, 1.0), (0, 0), (1280, 720)),
            None,
            "nothing is shown yet"
        );
    }
}
