// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The preview picture of steps 3, 4 and 5: the frame of the video at a time, with the OSD and the SRT data on it.

use std::sync::Arc;

use iced::Task;

use super::FlightReplay;
use crate::message::Message;
use crate::preview::{Grid, OsdPreview, PreviewRequest, PreviewResult, SrtPreview, video_point};
use crate::tasks;

impl FlightReplay {
    /// Asks for a new picture, at the time of the preview with what is chosen now.
    pub(super) fn refresh_preview(&mut self) -> Task<Message> {
        let (Some(tools), Some(video)) = (self.tools(), self.video.clone()) else {
            return Task::none();
        };
        let osd = match (&self.osd, &self.scaled_font, self.settings.draw_osd) {
            (Some(osd), Some(font), true) => Some(OsdPreview {
                file: Arc::clone(&osd.file),
                font: Arc::clone(font),
                options: self.settings.osd.clone(),
            }),
            _ => None,
        };
        let srt = match (&self.srt, self.settings.draw_srt) {
            (Some(srt), true) => Some(SrtPreview {
                file: Arc::clone(&srt.file),
                options: self.settings.srt.clone(),
            }),
            _ => None,
        };
        self.preview.sequence += 1;
        self.preview.busy = true;
        self.preview.error = None;
        let show_grid = self.preview.mask_mode && osd.is_some();
        tasks::make_preview(PreviewRequest {
            sequence: self.preview.sequence,
            tools,
            video: video.path.clone(),
            info: video.info.clone(),
            time_s: self.preview.time_s,
            base: self.preview.base.clone(),
            osd,
            srt,
            timeline: self.timeline(),
            show_grid,
        })
    }

    pub(super) fn refresh_preview_if_open(&mut self) -> Task<Message> {
        if self.video.is_some() {
            self.refresh_preview()
        } else {
            Task::none()
        }
    }

    /// The slider moves the time of the picture. While the sound plays, the picture follows it, so the slider is not
    /// the user's to move.
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
        match result {
            Ok(made) => {
                let shown = &made.shown;
                self.preview.shown = (shown.width(), shown.height());
                self.preview.handle = Some(iced::widget::image::Handle::from_rgba(
                    shown.width(),
                    shown.height(),
                    shown.as_raw().clone(),
                ));
                self.preview.base = Some((made.base.0, Arc::clone(&made.base.1)));
                self.preview.error = None;
            }
            Err(error) => self.preview.error = Some(error),
        }
        Task::none()
    }

    /// A click on the picture hides or shows the OSD cell under the pointer, when that is what the user is doing.
    pub(super) fn preview_pressed(&mut self) -> Task<Message> {
        if self.preview.mask_mode
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
