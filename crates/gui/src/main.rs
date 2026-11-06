// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Flight Replay application entry point.

// Release builds on Windows do not open a console window.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;

use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    iced::application(
        app::FlightReplay::default,
        app::FlightReplay::update,
        app::FlightReplay::view,
    )
    .title(app::FlightReplay::title)
    .run()?;
    Ok(())
}
