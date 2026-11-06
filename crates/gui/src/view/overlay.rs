// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use iced::Element;
use iced::widget::{checkbox, column, row, text};

use super::preview_view::{preview_block, sync_block};
use super::widgets::{action, hint, page, section, slider_row};
use crate::app::FlightReplay;
use crate::message::{Message, OsdEdit};

pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    let reason = app.availability().osd;
    let toggle = checkbox(app.draw_osd_on())
        .label("Draw the OSD on the video")
        .on_toggle_maybe(reason.is_none().then_some(Message::DrawOsd));
    let missing = reason.map(|reason| {
        format!(
            "{reason} Go back to the Input step to choose one, or go on: the video is then rendered without the OSD."
        )
    });

    let osd = &app.settings.osd;
    let placement = column![
        slider_row(
            "Horizontal position",
            "px",
            f64::from(osd.x_offset),
            -200.0..=700.0,
            1.0,
            "Pixels from the left edge of the video.",
            |v| { Message::Osd(OsdEdit::X(v as i32)) }
        ),
        slider_row(
            "Vertical position",
            "px",
            f64::from(osd.y_offset),
            -200.0..=700.0,
            1.0,
            "Pixels from the top edge of the video.",
            |v| { Message::Osd(OsdEdit::Y(v as i32)) }
        ),
        row![
            action("Center", Some(Message::Osd(OsdEdit::Center))),
            action("Reset position", Some(Message::Osd(OsdEdit::Reset))),
        ]
        .spacing(8),
    ]
    .spacing(8);

    let masks = column![
        checkbox(app.preview.mask_mode).label("Hide elements by clicking them on the picture")
            .on_toggle(|on| Message::Osd(OsdEdit::MaskMode(on))),
        row![
            text(format!("{} cells hidden", osd.masked_cells.len())),
            action("Show all", (!osd.masked_cells.is_empty()).then_some(Message::Osd(OsdEdit::ClearMasks))),
        ]
        .spacing(12),
        hint("With this on, the picture shows the grid of the OSD. Click a cell to hide it, and click it again to show it."),
    ]
    .spacing(8);

    let mut items = vec![toggle.into()];
    if let Some(message) = missing {
        items.push(hint(message));
    }
    items.push(section("Preview", preview_block(app, true)));
    items.push(section("Position", placement));
    items.push(section("Hidden elements", masks));
    items.push(sync_block(app));
    page(items).into()
}
