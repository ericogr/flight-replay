// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use iced::Element;
use iced::widget::{button, column, row, text};

use super::widgets::{action, good, hint, page, problem, section};
use crate::app::{FlightReplay, ToolsState};
use crate::message::Message;
use crate::preferences::ThemeChoice;

pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    let intro = column![
        text("Flight Replay adds the flight controller OSD, the SRT data and a synthetic motor sound to a Walksnail Avatar recording.").size(16),
        hint("The wizard goes through the video, what to draw on it, the sound and how to encode the result."),
    ]
    .spacing(6);

    let ffmpeg: Element<'_, Message> = match &app.tools {
        ToolsState::Searching => text("Looking for FFmpeg…").into(),
        ToolsState::Found(tools) => column![
            good("FFmpeg was found."),
            hint(format!("ffmpeg: {}", tools.ffmpeg().display())),
            hint(format!("ffprobe: {}", tools.ffprobe().display())),
            encoders(app),
            action("Choose another folder…", Some(Message::ChooseFfmpegFolder)),
        ]
        .spacing(6)
        .into(),
        ToolsState::Missing(error) => column![
            problem(error.clone()),
            text(install_hint()).size(14),
            action(
                "Choose the folder of ffmpeg…",
                Some(Message::ChooseFfmpegFolder)
            ),
        ]
        .spacing(8)
        .into(),
    };

    page(vec![
        intro.into(),
        section("FFmpeg", ffmpeg),
        section("Appearance", theme_buttons(app)),
    ])
    .into()
}

fn encoders(app: &FlightReplay) -> Element<'_, Message> {
    match &app.encoders {
        None => hint("Checking which video encoders work…"),
        Some(list) => {
            let working: Vec<String> = list
                .iter()
                .filter(|e| e.works)
                .map(|e| {
                    format!(
                        "{} ({})",
                        e.name,
                        if e.hardware { "hardware" } else { "software" }
                    )
                })
                .collect();
            if working.is_empty() {
                problem("No video encoder works. FFmpeg needs libx264 or a hardware encoder.")
            } else {
                hint(format!("Encoders that work: {}", working.join(", ")))
            }
        }
    }
}

fn theme_buttons(app: &FlightReplay) -> Element<'_, Message> {
    let choice = |label: &'static str, value: ThemeChoice| {
        let selected = app.preferences.theme == value;
        button(text(label))
            .on_press(Message::Theme(value))
            .style(if selected {
                button::primary
            } else {
                button::secondary
            })
    };
    row![
        choice("System", ThemeChoice::System),
        choice("Light", ThemeChoice::Light),
        choice("Dark", ThemeChoice::Dark)
    ]
    .spacing(8)
    .into()
}

fn install_hint() -> &'static str {
    if cfg!(target_os = "windows") {
        "Install it with: winget install ffmpeg"
    } else if cfg!(target_os = "macos") {
        "Install it with: brew install ffmpeg"
    } else {
        "Install it with your package manager, for example: sudo apt install ffmpeg"
    }
}
