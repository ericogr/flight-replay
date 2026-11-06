// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The views: the frame of the wizard and one module per step.

mod input;
mod output;
mod overlay;
mod preview_view;
mod render;
mod sound;
mod srt;
mod welcome;
mod widgets;

use iced::widget::{button, column, container, row, scrollable, space, text};
use iced::{Element, Length};

use crate::app::FlightReplay;
use crate::message::Message;
use crate::wizard::{self, Context, Step};

/// The whole window.
pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    let ctx = app.context();
    let header = column![
        text(flight_replay_core::APP_NAME).size(26),
        step_bar(app, &ctx)
    ]
    .spacing(10);

    let mut notices = column![].spacing(6);
    for (index, notice) in app.notices.iter().enumerate() {
        notices = notices.push(
            row![
                widgets::hint(notice.clone()),
                space::horizontal(),
                button(text("Dismiss").size(12))
                    .on_press(Message::DismissNotice(index))
                    .style(button::text),
            ]
            .spacing(8),
        );
    }

    let body: Element<'_, Message> = match app.step {
        Step::Welcome => welcome::view(app),
        Step::Input => input::view(app),
        Step::Overlay => overlay::view(app),
        Step::Srt => srt::view(app),
        Step::Sound => sound::view(app),
        Step::Output => output::view(app),
        Step::Render => render::view(app),
    };

    let column = column![
        header,
        notices,
        scrollable(container(body).padding(iced::Padding::from([0.0, 14.0]))).height(Length::Fill),
        footer(app, &ctx),
    ]
    .spacing(12)
    .padding(16);
    container(column)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn step_bar<'a>(app: &'a FlightReplay, ctx: &Context) -> Element<'a, Message> {
    let mut bar = row![].spacing(6);
    for step in Step::ALL {
        let reachable = !app.is_rendering() && wizard::can_go_to(app.step, step, ctx);
        let label = text(format!("{}  {}", step.number(), step.title())).size(14);
        bar = bar.push(
            button(label)
                .on_press_maybe(reachable.then_some(Message::GoTo(step)))
                .style(if step == app.step {
                    button::primary
                } else {
                    button::secondary
                }),
        );
    }
    bar.into()
}

fn footer<'a>(app: &'a FlightReplay, ctx: &Context) -> Element<'a, Message> {
    let problems = wizard::problems(app.step, ctx);
    let busy = app.is_rendering();
    let back = button(text("Back"))
        .on_press_maybe((app.step.previous().is_some() && !busy).then_some(Message::Back));
    let next: Element<'_, Message> = if app.step.next().is_some() {
        button(text("Next"))
            .on_press_maybe((problems.is_empty() && !busy).then_some(Message::Next))
            .into()
    } else {
        space::horizontal().width(Length::Shrink).into()
    };
    let mut what = column![].spacing(2);
    for problem in problems.iter().take(2) {
        what = what.push(widgets::hint(problem.clone()));
    }
    row![back, space::horizontal(), what, next]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .into()
}
