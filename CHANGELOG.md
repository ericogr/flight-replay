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
- `docs/architecture.md`.

### Changed

- The picture and the OSD follow the sound while the sound preview plays, and the preview stops at the end of the
  video. It used to stay on one frame, and play past the end.
- The colour scheme and the last folder are kept by the window and not by the core library. The same
  `settings.toml` is used, and files written by earlier versions are read as before.

### Fixed

- A picture could not be made at the very end of the video, so dragging the preview slider there showed an error.
- A second run over a folder tried to render the videos that the first run had written.
- The tests of the window crashed one run in five with the GPU renderer; they use the software renderer now.

- Project skeleton: Cargo workspace with `flight-replay-core` and `flight-replay`, Makefile, license files,
  dependency policy and continuous integration.
