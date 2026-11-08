// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The settings screen: FFmpeg, the colour scheme, the reset and the facts about the program.

use flight_replay_core::display;
use flight_replay_core::ffmpeg::Tools;
use flight_replay_core::ffmpeg::install::{Build, Stage};
use iced::Element;
use iced::widget::{button, column, progress_bar, row, text};

use super::widgets::{action, good, hint, page, problem, section};
use crate::app::{FlightReplay, ToolsState};
use crate::message::Message;
use crate::preferences::ThemeChoice;
use crate::state::{InstallRun, InstallState};

pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    page(vec![
        section("FFmpeg", ffmpeg(app)),
        section("Appearance", theme_buttons(app)),
        section("Reset", reset_settings(app)),
        section("About", about(app)),
    ])
    .into()
}

fn ffmpeg(app: &FlightReplay) -> Element<'_, Message> {
    let installing = matches!(app.install, InstallState::Running(_));
    let busy = app.is_rendering() || installing;
    let missing = matches!(app.tools, ToolsState::Missing(_));
    let mut content = column![].spacing(8);

    content = match &app.tools {
        ToolsState::Searching => content.push(text("Looking for FFmpeg…")),
        ToolsState::Found(tools) => content
            .push(good(match tools.ffmpeg_version() {
                Some(version) => format!("FFmpeg {version} was found."),
                None => "FFmpeg was found.".to_owned(),
            }))
            .push(hint(format!("Origin: {}.", app.origin_of(tools).label())))
            .push(hint(format!("ffmpeg: {}", tools.ffmpeg().display())))
            .push(hint(format!("ffprobe: {}", tools.ffprobe().display())))
            .push(encoders(app)),
        ToolsState::Missing(error) => content.push(problem(error.clone())),
    };

    if missing || installing || matches!(app.install, InstallState::Failed(_)) {
        content = content.push(install_block(app));
    }
    if missing {
        content = content.push(text(install_hint()).size(14));
    }
    let choose = if missing {
        "Choose the folder of ffmpeg…"
    } else {
        "Choose another folder…"
    };
    content = content.push(
        row![
            action(
                choose,
                (!busy && !app.dialog.is_open()).then_some(Message::ChooseFfmpegFolder)
            ),
            action("Check again", (!busy).then_some(Message::RecheckFfmpeg)),
        ]
        .spacing(8),
    );
    if app.managed_installed {
        content = content.push(managed_copy(busy));
    }
    content.into()
}

fn encoders(app: &FlightReplay) -> Element<'_, Message> {
    match app.working_encoders() {
        None => hint("Checking which video encoders work…"),
        Some(working) if working.is_empty() => {
            problem("No video encoder works. FFmpeg needs libx264 or a hardware encoder.")
        }
        Some(working) => hint(format!("Encoders that work: {}", working.join(", "))),
    }
}

/// The button to install FFmpeg, or how far the install has come, or why it failed.
fn install_block(app: &FlightReplay) -> Element<'_, Message> {
    match &app.install {
        InstallState::Running(run) => running(run),
        InstallState::Failed(reason) => column![problem(reason.clone()), offer(app, true)]
            .spacing(6)
            .into(),
        InstallState::Idle => offer(app, false),
    }
}

fn offer(app: &FlightReplay, again: bool) -> Element<'_, Message> {
    let Some(build) = Build::for_this_system() else {
        return hint(
            "Flight Replay cannot install FFmpeg on this system. Use the command below, or choose the folder where it is.",
        );
    };
    let label = if again {
        "Try again".to_owned()
    } else {
        format!("Install FFmpeg ({})", display::size(build.download_bytes()))
    };
    let folder = Tools::managed_dir().map_or_else(
        || "your user folder".to_owned(),
        |dir| dir.display().to_string(),
    );
    column![
        button(text(label)).on_press_maybe((!app.is_rendering()).then_some(Message::InstallFfmpeg)),
        hint(format!(
            "Downloaded from {} with curl, only because you click. FFmpeg {} is free software under the {}. The copy is saved in {folder}.",
            build.provider, build.version, build.license
        )),
    ]
    .spacing(6)
    .into()
}

fn running(run: &InstallRun) -> Element<'_, Message> {
    let (line, done, total) = match run.progress {
        None => ("Starting…".to_owned(), 0.0, 1.0),
        Some(progress) => {
            let line = match progress.stage {
                Stage::Downloading => format!(
                    "Downloading FFmpeg… {} of {}",
                    display::size(progress.done),
                    display::size(progress.total)
                ),
                Stage::Checking => "Checking the download…".to_owned(),
                Stage::Unpacking => "Unpacking…".to_owned(),
                Stage::Testing => "Testing FFmpeg…".to_owned(),
            };
            #[allow(
                clippy::cast_precision_loss,
                reason = "a progress bar does not need every byte of a download"
            )]
            let (done, total) = (progress.done as f32, progress.total.max(1) as f32);
            (line, done, total)
        }
    };
    column![
        text(line),
        progress_bar(0.0..=total, done),
        action("Cancel", Some(Message::CancelInstall)),
    ]
    .spacing(8)
    .into()
}

fn managed_copy(busy: bool) -> Element<'static, Message> {
    let folder = Tools::managed_dir().map_or_else(String::new, |dir| dir.display().to_string());
    column![
        hint(format!(
            "Flight Replay keeps a copy of FFmpeg that it installed, in {folder}."
        )),
        action(
            "Remove the FFmpeg that Flight Replay installed",
            (!busy).then_some(Message::RemoveInstalledFfmpeg)
        ),
    ]
    .spacing(6)
    .into()
}

fn install_hint() -> &'static str {
    if cfg!(target_os = "windows") {
        "Or install it yourself: winget install ffmpeg"
    } else if cfg!(target_os = "macos") {
        "Or install it yourself: brew install ffmpeg"
    } else {
        "Or install it yourself with your package manager, for example: sudo apt install ffmpeg"
    }
}

fn theme_buttons(app: &FlightReplay) -> Element<'_, Message> {
    row(ThemeChoice::ALL.map(|choice| {
        button(text(choice.label()))
            .on_press(Message::Theme(choice))
            .style(if app.preferences.theme == choice {
                button::primary
            } else {
                button::secondary
            })
            .into()
    }))
    .spacing(8)
    .into()
}

/// The way back to the default settings. It asks first: it forgets the saved sounds and the choices of every step.
fn reset_settings(app: &FlightReplay) -> Element<'_, Message> {
    if !app.confirming_reset {
        return column![
            hint(
                "Brings every choice of every step back to what it was the first time the program ran. The video and the files that are open stay as they are."
            ),
            action(
                "Reset all settings…",
                (!app.is_rendering()).then_some(Message::AskResetAll)
            ),
        ]
        .spacing(8)
        .into();
    }
    column![
        problem(
            "Reset all settings? The FFmpeg folder, the font, the colour scheme, the saved sounds and every choice of the steps are forgotten. This cannot be undone."
        ),
        row![
            button(text("Yes, reset everything"))
                .on_press(Message::ResetAll)
                .style(button::danger),
            action("Cancel", Some(Message::CancelResetAll)),
        ]
        .spacing(8),
    ]
    .spacing(8)
    .into()
}

fn about(app: &FlightReplay) -> Element<'_, Message> {
    column![
        text(format!(
            "{} {}",
            flight_replay_core::APP_NAME,
            env!("CARGO_PKG_VERSION")
        )),
        hint("Free software under the GNU General Public License, version 3 only."),
        hint(
            "FFmpeg is a separate program with its own license. A copy that Flight Replay installs has its license text next to it."
        ),
        row![
            action("Project page", Some(Message::OpenProjectPage)),
            action(
                "Open the settings folder",
                app.settings_folder().map(|_| Message::OpenSettingsFolder)
            ),
        ]
        .spacing(8),
    ]
    .spacing(8)
    .into()
}
