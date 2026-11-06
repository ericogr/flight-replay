// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The preview picture with its time slider, and the controls of the timing, which several steps share.

use iced::widget::{
    column, container, image, mouse_area, pick_list, row, slider, text, text_input,
};
use iced::{Alignment, Element, Length};

use super::widgets::{hint, labeled, problem, section};
use crate::app::FlightReplay;
use crate::message::{Message, SyncEdit};
use crate::state::ScaleChoice;
use flight_replay_core::display::clock_tenths;

/// The preview picture and the slider that chooses its time.
pub fn preview_block(app: &FlightReplay, masks: bool) -> Element<'_, Message> {
    let Some(video) = &app.video else {
        return hint("Open a video to see it here.");
    };
    let duration = video.info.duration_s.max(0.1);
    let time = row![
        text(clock_tenths(app.preview.time_s)).width(Length::Fixed(60.0)),
        slider(
            0.0..=video.last_frame_s().max(0.1),
            app.preview.time_s,
            Message::PreviewTime
        )
        .step(0.05)
        .on_release(Message::PreviewTimeReleased),
        text(clock_tenths(duration)).width(Length::Fixed(60.0)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let picture: Element<'_, Message> = match &app.preview.handle {
        Some(handle) => {
            let (width, height) = app.preview.shown;
            let img = image(handle.clone())
                .width(Length::Fixed(width as f32))
                .height(Length::Fixed(height as f32));
            if masks && app.preview.mask_mode {
                mouse_area(img)
                    .on_move(Message::PreviewHover)
                    .on_press(Message::PreviewPressed)
                    .on_exit(Message::PreviewExited)
                    .into()
            } else {
                img.into()
            }
        }
        None if app.preview.busy => text("Making the preview…").into(),
        None => hint("No picture yet."),
    };

    let mut items = column![container(picture).center_x(Length::Fill), time].spacing(8);
    // While the sound plays the picture is made again and again, and a note that comes and goes would shift the
    // controls below it.
    if app.preview.busy && app.preview.handle.is_some() && !app.is_playing() {
        items = items.push(hint("Updating…"));
    }
    if let Some(error) = &app.preview.error {
        items = items.push(problem(format!("The preview could not be made: {error}")));
    }
    items.into()
}

/// How the OSD is timed against the video.
pub fn sync_block(app: &FlightReplay) -> Element<'_, Message> {
    let mut items = column![labeled(
        "Speed of the OSD",
        pick_list(ScaleChoice::ALL, Some(app.sync.choice), |c| Message::Sync(
            SyncEdit::Scale(c)
        )),
    ),]
    .spacing(8);
    if app.sync.choice == ScaleChoice::Manual {
        items = items.push(labeled(
            "Scale",
            text_input("1.000", &app.sync.manual_scale)
                .on_input(|t| Message::Sync(SyncEdit::ManualScale(t)))
                .width(Length::Fixed(120.0)),
        ));
    }
    items = items.push(labeled(
        "Delay of the OSD (seconds)",
        text_input("0", &app.sync.offset)
            .on_input(|t| Message::Sync(SyncEdit::Offset(t)))
            .width(Length::Fixed(120.0)),
    ));
    match app.timeline() {
        Some(timeline) => {
            items = items.push(hint(format!(
                "Scale in use: {:.6}. The OSD recording and the video run at slightly different speeds; the automatic scale corrects it from their durations.",
                timeline.scale()
            )));
        }
        None => items = items.push(hint("There is no .osd file, so there is nothing to time.")),
    }
    section("Timing of the OSD", items)
}
