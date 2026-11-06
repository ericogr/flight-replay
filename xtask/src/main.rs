// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Developer automation for Flight Replay. Run it with `cargo xtask <command>`, or through `make`.
//!
//! This is not a command-line interface for users of the application.

mod dist;
mod fixtures;

use anyhow::{Result, bail};

const HELP: &str = "\
Usage: cargo xtask <command> [options]

Commands:
  fixtures   Cut a short clip from a recording, with its .osd and .srt, and move the GPS to open ocean
  dist       Pack the release build for this system (archive and SHA256SUMS)
  app-bundle Write `Flight Replay.app` for macOS
  help       Show this help

Run `cargo xtask fixtures --help` for the options of a command.";

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("fixtures") => fixtures::run(args.collect()),
        Some("dist") => dist::run(args.collect()),
        Some("app-bundle") => dist::run_bundle(args.collect()),
        Some("help" | "--help" | "-h") | None => {
            println!("{HELP}");
            Ok(())
        }
        Some(other) => bail!("unknown command {other:?}\n\n{HELP}"),
    }
}
