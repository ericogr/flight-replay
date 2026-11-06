// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The preview picture of steps 3, 4 and 5: the frame of the video at a time, with the layers that the step shows on
//! it (the OSD, the SRT data).

use std::sync::Arc;

use iced::Task;

use super::FlightReplay;
use crate::message::Message;
use crate::preview::{
    Grid, OsdPreview, PreviewRequest, PreviewResult, Scene, SrtPreview, video_point,
};
use crate::tasks;
use crate::wizard::PreviewLayers;

impl FlightReplay {
    /// What the step being shown wants on its picture, which is not all that is there to be drawn: see
    /// [`Step::preview_layers`].
    fn step_layers(&self) -> PreviewLayers {
        self.step.preview_layers(self.preferences.show_other_layers)
    }

    /// The OSD layer of a picture, if the OSD is drawn and this step wants it.
    fn osd_layer(&self) -> Option<OsdPreview> {
        let (osd, font) = (self.osd.as_ref()?, self.scaled_font.as_ref()?);
        (self.draw_osd_on() && self.step_layers().osd).then(|| OsdPreview {
            file: Arc::clone(&osd.file),
            font: Arc::clone(font),
            options: self.settings.osd.clone(),
        })
    }

    /// The SRT layer of a picture, if the SRT data are drawn and this step wants them.
    fn srt_layer(&self) -> Option<SrtPreview> {
        let srt = self.srt.as_ref()?;
        (self.draw_srt_on() && self.step_layers().srt).then(|| SrtPreview {
            file: Arc::clone(&srt.file),
            options: self.settings.srt.clone(),
        })
    }

    /// What a picture made now would have on it. The grid of the OSD cells is there only in the step where the masks
    /// are chosen, and only while the user is choosing them.
    pub(super) fn layers_wanted(&self) -> PreviewLayers {
        let osd = self.osd_layer().is_some();
        PreviewLayers {
            osd,
            srt: self.srt_layer().is_some(),
            grid: osd && self.preview.mask_mode && self.step_layers().grid,
        }
    }

    /// What a picture is made from now: the video and the layers of this step. `None` without a video or FFmpeg.
    pub(super) fn scene(&self) -> Option<Scene> {
        let video = self.video.as_ref()?;
        Some(Scene {
            tools: self.tools()?,
            video: video.path.clone(),
            info: video.info.clone(),
            osd: self.osd_layer(),
            srt: self.srt_layer(),
            timeline: self.timeline(),
        })
    }

    /// Asks for a new picture, at the time of the preview with what is chosen now. While the preview plays the picture
    /// is the stream's, and it is the stream that starts again.
    pub(super) fn refresh_preview(&mut self) -> Task<Message> {
        if self.is_playing() {
            return self.restart_playback();
        }
        let Some(scene) = self.scene() else {
            return Task::none();
        };
        let layers = self.layers_wanted();
        self.preview.layers = layers;
        self.preview.sequence += 1;
        self.preview.busy = true;
        self.preview.error = None;
        tasks::make_preview(PreviewRequest {
            sequence: self.preview.sequence,
            scene,
            time_s: self.preview.time_s,
            base: self.preview.base.clone(),
            show_grid: layers.grid,
        })
    }

    pub(super) fn refresh_preview_if_open(&mut self) -> Task<Message> {
        if self.video.is_some() {
            self.refresh_preview()
        } else {
            Task::none()
        }
    }

    /// The slider moves the time of the picture. While the preview plays, the picture follows the clock, so the slider
    /// is not the user's to move.
    pub(super) fn set_preview_time(&mut self, seconds: f64) -> Task<Message> {
        if !self.is_playing() {
            self.preview.time_s = seconds;
        }
        Task::none()
    }

    /// A picture that was asked for. An older answer than the last request is ignored.
    pub(super) fn preview_ready(
        &mut self,
        sequence: u64,
        result: Result<Arc<PreviewResult>, String>,
    ) -> Task<Message> {
        if sequence != self.preview.sequence {
            return Task::none();
        }
        self.preview.busy = false;
        // A picture that was asked for before the preview started playing is too late: the stream has the picture
        // now, and the moment it was made for has passed.
        if self.is_playing() {
            return Task::none();
        }
        match result {
            Ok(made) => {
                let shown = &made.shown;
                self.preview
                    .set_picture(shown.width(), shown.height(), shown.as_raw().clone());
                self.preview.base = Some((made.base.0, Arc::clone(&made.base.1)));
                self.preview.error = None;
            }
            Err(error) => self.preview.error = Some(error),
        }
        Task::none()
    }

    /// A click on the picture hides or shows the OSD cell under the pointer, when that is what the user is doing. The
    /// picture that plays has no grid to click on.
    pub(super) fn preview_pressed(&mut self) -> Task<Message> {
        if self.preview.mask_mode
            && !self.is_playing()
            && let Some((column, row)) = self.cell_under_pointer()
        {
            self.settings.osd.toggle_mask(column, row);
            return self.refresh_preview();
        }
        Task::none()
    }

    /// The cell of the OSD under the pointer, if any.
    fn cell_under_pointer(&self) -> Option<(u32, u32)> {
        let (video, osd, font) = (
            self.video.as_ref()?,
            self.osd.as_ref()?,
            self.scaled_font.as_ref()?,
        );
        let hover = self.preview.hover?;
        let (x, y) = video_point(
            (hover.x, hover.y),
            self.preview.shown,
            (video.info.width, video.info.height),
        )?;
        Grid::new(&osd.file, font, &self.settings.osd).cell_at(x, y)
    }
}
