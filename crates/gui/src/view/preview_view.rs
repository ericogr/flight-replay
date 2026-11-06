// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The preview picture with its time slider, and the controls of the timing, which several steps share.

use iced::widget::{
    checkbox, column, container, image, mouse_area, pick_list, row, slider, text, text_input,
};
use iced::{Alignment, Element, Length};

use super::widgets::{action, hint, labeled, problem, section};
use crate::app::FlightReplay;
use crate::message::{Message, SyncEdit};
use crate::playback;
use crate::preview::shown_size;
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

    // The preview shows all the layers, as the render will draw them. The user can ask for only the layer of the step.
    let other_layers = checkbox(app.preferences.show_other_layers)
        .label(format!(
            "Also show the {} in the preview",
            app.step.other_layers_name()
        ))
        .on_toggle(Message::ShowOtherLayers);
    let mut items = column![
        container(picture).center_x(Length::Fill),
        time,
        other_layers
    ]
    .spacing(8);
    // The picture is a preview. Saying so keeps it from being taken for what the render will give.
    let (width, height) = shown_size(video.info.width, video.info.height);
    if width != video.info.width {
        items = items.push(hint(format!(
            "Preview at reduced quality: {width}×{height}, while the video is {}×{}. The video you render keeps its full quality.",
            video.info.width, video.info.height
        )));
    }
    // While the preview plays the picture is made again and again, and a note that comes and goes would shift the
    // controls below it.
    if app.preview.busy && app.preview.handle.is_some() && !app.is_playing() {
        items = items.push(hint("Updating…"));
    }
    if let Some(error) = &app.preview.error {
        items = items.push(problem(format!("The preview could not be made: {error}")));
    }
    items.into()
}

/// Says that the picture is shown at reduced quality while it plays. Every step that plays it says so.
pub fn play_quality_note() -> String {
    format!(
        "While it plays the picture is shown at reduced quality: at most {:.0} frames per second, and frames are skipped if the computer is too slow. The video you render is not affected.",
        playback::MAX_FPS
    )
}

/// Play and Stop, with what is going on and why the preview could not play. `play` is what the Play button sends, or
/// `None` when it cannot be pressed.
pub fn play_controls(app: &FlightReplay, play: Option<Message>) -> Element<'_, Message> {
    let status = if app.busy.audio {
        "Making the sound…"
    } else if app.is_playing() {
        "Playing…"
    } else {
        ""
    };
    let mut items = column![
        row![
            action("Play 10 seconds from here", play),
            action("Stop", app.is_playing().then_some(Message::StopPlay)),
            text(status),
        ]
        .spacing(10)
    ]
    .spacing(8);
    if let Some(error) = &app.play_error {
        items = items.push(problem(error.clone()));
    }
    items.into()
}

/// The picture played alone, without sound, to check the layer of the step in motion. `what` names the layer.
pub fn picture_controls<'a>(app: &'a FlightReplay, what: &str) -> Element<'a, Message> {
    let can_play = app.video.is_some() && !app.is_playing();
    column![
        hint(format!(
            "Choose the moment on the slider, then play it to see {what} in motion. There is no sound here; the motor sound has its own step. {}",
            play_quality_note()
        )),
        play_controls(app, can_play.then_some(Message::PlayPicture)),
    ]
    .spacing(8)
    .into()
}

/// How the OSD is timed against the video.
pub fn sync_block(app: &FlightReplay) -> Element<'_, Message> {
    let mut items = column![labeled(
        "Speed of the OSD",
        pick_list(ScaleChoice::ALL, Some(app.preferences.sync.choice), |c| {
            Message::Sync(SyncEdit::Scale(c))
        }),
    ),]
    .spacing(8);
    if app.preferences.sync.choice == ScaleChoice::Manual {
        items = items.push(labeled(
            "Scale",
            text_input("1.000", &app.preferences.sync.manual_scale)
                .on_input(|t| Message::Sync(SyncEdit::ManualScale(t)))
                .width(Length::Fixed(120.0)),
        ));
    }
    items = items.push(labeled(
        "Delay of the OSD (seconds)",
        text_input("0", &app.preferences.sync.offset)
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
