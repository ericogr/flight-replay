// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The first step: what the program is, and a summary of the machine it runs on. Everything that can be changed
//! lives on the settings screen.

use flight_replay_core::display;
use iced::Element;
use iced::widget::{column, text};

use super::widgets::{action, good, hint, labeled, page, problem, section};
use crate::app::{FlightReplay, ToolsState};
use crate::message::Message;

/// Below this much free space the summary warns: a render of a long video can take several gigabytes.
const LOW_DISK_BYTES: u64 = 5_000_000_000;

pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    let intro = column![
        text("Flight Replay adds the flight controller OSD, the SRT data and a synthetic motor sound to a Walksnail Avatar recording.").size(16),
        hint("The wizard goes through the video, what to draw on it, the sound and how to encode the result."),
    ]
    .spacing(6);

    let mut summary = column![
        labeled(
            "Flight Replay",
            text(format!("version {}", env!("CARGO_PKG_VERSION")))
        ),
        labeled("FFmpeg", ffmpeg_line(app)),
        labeled("Video encoders", encoders_line(app)),
        labeled("Colour scheme", text(app.preferences.theme.label())),
        labeled("Free disk space", disk_line(app)),
    ]
    .spacing(8);
    if let Some(file) = app.settings_file() {
        summary = summary.push(labeled("Settings file", hint(file.display().to_string())));
    }
    if matches!(app.tools, ToolsState::Missing(_)) {
        summary = summary
            .push(hint(
                "FFmpeg is needed to make the video. The settings can install it, or point to where it is.",
            ))
            .push(action("Open settings", Some(Message::OpenSettings)));
    } else {
        summary = summary.push(hint(
            "All of this can be changed in Settings, at the top right.",
        ));
    }

    page(vec![intro.into(), section("Summary", summary)]).into()
}

fn ffmpeg_line(app: &FlightReplay) -> Element<'_, Message> {
    match &app.tools {
        ToolsState::Searching => text("Looking for FFmpeg…").into(),
        ToolsState::Found(tools) => good(format!(
            "{} · {}",
            tools.ffmpeg_version().unwrap_or("version unknown"),
            app.origin_of(tools).label()
        )),
        ToolsState::Missing(error) => problem(error.clone()),
    }
}

fn encoders_line(app: &FlightReplay) -> Element<'_, Message> {
    if app.tools().is_none() {
        return hint("Needs FFmpeg.");
    }
    match app.working_encoders() {
        None => hint("Checking which video encoders work…"),
        Some(working) if working.is_empty() => {
            problem("None works. FFmpeg needs libx264 or a hardware encoder.")
        }
        Some(working) => text(working.join(", ")).into(),
    }
}

fn disk_line(app: &FlightReplay) -> Element<'_, Message> {
    let Some(disk) = &app.disk else {
        return hint("Could not be read.");
    };
    let line = format!(
        "{} free, in {}",
        display::size(disk.free_bytes),
        disk.folder.display()
    );
    if disk.free_bytes < LOW_DISK_BYTES {
        column![problem(line), hint("A render can take several gigabytes.")].into()
    } else {
        text(line).into()
    }
}
