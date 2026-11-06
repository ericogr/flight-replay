// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The iced application state, messages and views.

use flight_replay_core::APP_NAME;
use iced::Element;
use iced::widget::{center, column, text};

/// Application state. The wizard steps are added in phase P7.
#[derive(Debug, Default)]
pub struct FlightReplay;

/// Messages handled by [`FlightReplay::update`]. There are none yet.
#[derive(Debug, Clone)]
pub enum Message {}

impl FlightReplay {
    /// Window title.
    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "iced calls this method; the state is added in phase P7"
    )]
    pub fn title(&self) -> String {
        APP_NAME.to_owned()
    }

    /// Applies a message to the state.
    #[expect(
        clippy::needless_pass_by_value,
        clippy::unused_self,
        reason = "iced calls this method; the message type is empty until phase P7"
    )]
    pub fn update(&mut self, message: Message) {
        match message {}
    }

    /// Builds the view.
    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "iced calls this method; the state is added in phase P7"
    )]
    pub fn view(&self) -> Element<'_, Message> {
        center(
            column![
                text(APP_NAME).size(32),
                text(format!("version {}", flight_replay_core::version())),
            ]
            .spacing(12),
        )
        .into()
    }
}
