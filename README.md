# Flight Replay

Flight Replay renders the flight controller OSD and the SRT data onto a Walksnail Avatar recording, and adds a
synthesized motor and propeller sound driven by the telemetry in the `.osd` file. It supports INAV and
Betaflight OSD layouts. The application is a step-by-step wizard.

Flight Replay runs on Linux, Windows and macOS.

> **Status:** early development. The project skeleton is in place. The features are described in
> [PLAN.md](PLAN.md).

## Requirements

- Rust 1.88 or newer. The toolchain pinned in `rust-toolchain.toml` is installed automatically by `rustup`.
- `ffmpeg` and `ffprobe` 6.0 or newer, on `PATH`. They are needed once rendering is implemented.
- GNU Make.

## Build and run

```sh
make build      # debug build of all crates
make run        # run the application
make check      # formatting, lints, tests and dependency policy
make release    # optimised build
```

`make help` lists all the targets.

## License

Flight Replay is free software, licensed under the GNU General Public License, version 3 (GPL-3.0-only).
See [LICENSE](LICENSE). The notices for the code it adapts from other projects are in [NOTICE](NOTICE).

## Disclaimer

This project is not affiliated with Walksnail or Caddx.
