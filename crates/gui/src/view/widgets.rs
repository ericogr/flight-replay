// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Small pieces shared by the views.

use std::path::Path;

use iced::widget::{Column, Row, button, column, container, row, slider, text, tooltip};
use iced::{Alignment, Element, Length, Theme};

use crate::message::Message;

/// A titled block of controls.
pub fn section<'a>(
    title: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(column![text(title).size(18), content.into()].spacing(10))
        .padding(14)
        .width(Length::Fill)
        .style(container::rounded_box)
        .into()
}

/// A line of secondary text.
pub fn hint<'a>(content: impl Into<String>) -> Element<'a, Message> {
    text(content.into()).size(13).style(text::secondary).into()
}

/// A line of text that reports a problem.
pub fn problem<'a>(content: impl Into<String>) -> Element<'a, Message> {
    text(content.into())
        .style(|theme: &Theme| text::Style {
            color: Some(theme.extended_palette().danger.strong.color),
        })
        .into()
}

/// A line of text that reports success.
pub fn good<'a>(content: impl Into<String>) -> Element<'a, Message> {
    text(content.into())
        .style(|theme: &Theme| text::Style {
            color: Some(theme.extended_palette().success.strong.color),
        })
        .into()
}

/// A label on the left and a control on the right.
pub fn labeled<'a>(label: &'a str, control: impl Into<Element<'a, Message>>) -> Row<'a, Message> {
    row![text(label).width(Length::Fixed(190.0)), control.into()]
        .spacing(10)
        .align_y(Alignment::Center)
}

/// A slider with its label, the value and a tooltip.
pub fn slider_row<'a>(
    label: &'a str,
    unit: &'a str,
    value: f64,
    range: std::ops::RangeInclusive<f64>,
    step: f64,
    help: &'a str,
    on_change: impl Fn(f64) -> Message + 'a,
) -> Element<'a, Message> {
    let decimals = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else {
        3
    };
    let line = row![
        text(label).width(Length::Fixed(190.0)),
        slider(range, value, on_change)
            .step(step)
            .width(Length::Fill),
        text(format!("{value:.decimals$} {unit}")).width(Length::Fixed(90.0)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    tooltip(
        line,
        container(text(help).size(13))
            .padding(6)
            .style(container::rounded_box),
        tooltip::Position::Top,
    )
    .into()
}

/// A button, or the same button greyed out.
pub fn action(label: &str, message: Option<Message>) -> Element<'_, Message> {
    button(text(label)).on_press_maybe(message).into()
}

/// A path for showing: its name, with the folder in the tooltip.
pub fn path_text<'a>(path: &Path) -> Element<'a, Message> {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    tooltip(
        text(name),
        container(text(path.display().to_string()).size(13))
            .padding(6)
            .style(container::rounded_box),
        tooltip::Position::Bottom,
    )
    .into()
}

/// The time as minutes and seconds.
pub fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0);
    let minutes = (total / 60.0).floor();
    format!("{minutes:.0}:{:04.1}", total - minutes * 60.0)
}

/// A column with the usual spacing of a step.
pub fn page(items: Vec<Element<'_, Message>>) -> Column<'_, Message> {
    Column::with_children(items).spacing(14).width(Length::Fill)
}
