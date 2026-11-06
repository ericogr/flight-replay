// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use iced::Element;
use iced::widget::{checkbox, column};

use super::preview_view::preview_block;
use super::widgets::{hint, page, section, slider_row};
use crate::app::FlightReplay;
use crate::message::{Message, SrtEdit};

pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    let toggle = checkbox(app.settings.draw_srt)
        .label("Draw the SRT data on the video")
        .on_toggle_maybe(app.srt.is_some().then_some(Message::DrawSrt));
    let s = &app.settings.srt;

    let fields = column![
        checkbox(s.show_time)
            .label("Flight time")
            .on_toggle(|v| Message::Srt(SrtEdit::Time(v))),
        checkbox(s.show_sky_battery)
            .label("Aircraft battery")
            .on_toggle(|v| Message::Srt(SrtEdit::SkyBattery(v))),
        checkbox(s.show_ground_battery)
            .label("Goggles battery")
            .on_toggle(|v| Message::Srt(SrtEdit::GroundBattery(v))),
        checkbox(s.show_signal)
            .label("Signal")
            .on_toggle(|v| Message::Srt(SrtEdit::Signal(v))),
        checkbox(s.show_latency)
            .label("Latency")
            .on_toggle(|v| Message::Srt(SrtEdit::Latency(v))),
        checkbox(s.show_bitrate)
            .label("Bitrate")
            .on_toggle(|v| Message::Srt(SrtEdit::Bitrate(v))),
        checkbox(s.show_distance)
            .label("Distance")
            .on_toggle(|v| Message::Srt(SrtEdit::Distance(v))),
    ]
    .spacing(6);

    let layout = column![
        slider_row(
            "Horizontal position",
            "%",
            f64::from(s.position_percent.0),
            0.0..=100.0,
            0.5,
            "Percent of the width of the video.",
            |v| { Message::Srt(SrtEdit::PositionX(v as f32)) }
        ),
        slider_row(
            "Vertical position",
            "%",
            f64::from(s.position_percent.1),
            0.0..=100.0,
            0.5,
            "Percent of the height of the video.",
            |v| { Message::Srt(SrtEdit::PositionY(v as f32)) }
        ),
        slider_row(
            "Text size",
            "px",
            f64::from(s.scale),
            10.0..=100.0,
            1.0,
            "Height of the text on a 1080p video. It scales with the video.",
            |v| { Message::Srt(SrtEdit::Scale(v as f32)) }
        ),
    ]
    .spacing(8);

    let mut items = vec![toggle.into()];
    if app.srt.is_none() {
        items.push(hint(
            "There is no .srt file. Go back to the Input step to choose one.",
        ));
    }
    items.push(section("Preview", preview_block(app, false)));
    items.push(section("Fields", fields));
    items.push(section("Position and size", layout));
    page(items).into()
}
