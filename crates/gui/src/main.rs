// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Flight Replay application entry point.

// Release builds on Windows do not open a console window.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;
mod audio;
mod logo;
mod message;
mod playback;
mod preferences;
mod preview;
mod sound_params;
mod state;
mod tasks;
mod view;
mod wizard;

#[cfg(test)]
mod tests;

use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    iced::application(
        app::FlightReplay::boot,
        app::FlightReplay::update,
        view::view,
    )
    .title(flight_replay_core::APP_NAME)
    .theme(app::FlightReplay::theme)
    .subscription(|_: &app::FlightReplay| app::FlightReplay::subscription())
    .window(window_settings())
    .exit_on_close_request(false)
    .run()?;
    Ok(())
}

/// The window: its size, its icon and, on Linux, the id that ties it to `flight-replay.desktop`.
fn window_settings() -> iced::window::Settings {
    iced::window::Settings {
        size: iced::Size::new(1100.0, 820.0),
        min_size: Some(iced::Size::new(900.0, 600.0)),
        icon: logo::window_icon(),
        #[cfg(target_os = "linux")]
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "flight-replay".to_owned(),
            ..Default::default()
        },
        ..Default::default()
    }
}
