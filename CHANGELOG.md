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
- Ready-made motor sounds for different electric motors and propellers (small park flyer, three blades, aerobatic
  power, large slow propeller, pusher, ducted fan), next to the default. They are a starting point: every value can
  still be changed, and the sound list says "Custom" when one was. Two more sliders, the whine pitch and the sharpness
  of the propeller noise.
- An option to show the FFmpeg command before rendering.
- OSD fonts that come with the program: the default font of INAV (GPL-3.0) and the Material OSD font for Betaflight
  (CC BY-SA 4.0), each with a sheet for 720p and one for 1080p. The input step has a list of them next to the choice
  of a file, remembers the choice, shows the credit of the font, and warns when the font is for another firmware than
  the `.osd` file. A video, or each video of a folder, is drawn with the sheet nearest to its size. The sheets, their
  licenses and where they come from are in `assets/osd-fonts/`; fonts whose license does not allow redistribution are
  left out, and the README there says which and why. The release archives carry the license texts.

### Changed

- A part of the output that the files cannot give is switched off, with the reason on its step, instead of holding
  the user at that step: a video with only an `.srt` file gets the SRT data, an `.osd` of Betaflight gets the OSD and
  no sound, and an OSD that does not show the throttle gets everything but the sound. The summary before the render
  lists what is left out. A folder is planned the same way, video by video, and a video is left out only when nothing
  is left to add. What the user asked for is kept for the next video.
- When the OSD lacks something the sound needs, the sound step names the element to turn on in the INAV Configurator
  (OSD tab), such as "Throttle Position" or "Current Draw".
- The FFmpeg command is no longer shown before the render unless "Show the FFmpeg command before rendering" is
  ticked. It is off by default.
- The picture and the OSD follow the sound while the sound preview plays, and the preview stops at the end of the
  video. It used to stay on one frame, and play past the end.
- The colour scheme and the last folder are kept by the window and not by the core library. The same
  `settings.toml` is used, and files written by earlier versions are read as before.

### Fixed

- A video with no `.osd` file, or with an OSD of a firmware without telemetry, could not go past the sound step: the
  sound was on by default and its box could not be unticked.
- A picture could not be made at the very end of the video, so dragging the preview slider there showed an error.
- A second run over a folder tried to render the videos that the first run had written.
- The tests of the window crashed one run in five with the GPU renderer; they use the software renderer now.

- Project skeleton: Cargo workspace with `flight-replay-core` and `flight-replay`, Makefile, license files,
  dependency policy and continuous integration.
