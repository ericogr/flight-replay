# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The wizard: seven steps from the video to the render, with the settings saved between runs, a frame preview, a
  sound preview, drag and drop, and light, dark and system colour schemes.
- The OSD and the SRT data drawn on the picture, with the font of the goggles, masks, centring and timing.
- A synthesized motor, propeller and wind sound driven by the INAV telemetry in the `.osd` file, with presets.
- Encoding with libx264, libx265 and the hardware encoders FFmpeg offers, 1440p scaling, a chroma-key background,
  cancelling without leaving a file behind, and a check of the output.
- A queue of videos, with every video of a folder, and videos made of several `.osd` files.
- Archives for Linux, Windows and macOS (`make dist`), a Linux desktop entry and icons, and `Flight Replay.app`.

- Project skeleton: Cargo workspace with `flight-replay-core` and `flight-replay`, Makefile, license files,
  dependency policy and continuous integration.
