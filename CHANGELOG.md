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
- Play and Stop in the OSD step and in the SRT data step: ten seconds of the video from the time on the slider, with
  the OSD or the SRT data drawn on it, to check the layer in motion. It has no sound; the motor sound keeps its own
  step.
- "Also show the SRT data in the preview" in the step of the OSD, "Also show the OSD in the preview" in the step of
  the SRT data and "Also show the SRT and OSD data in the preview" in the step of the sound. It is on, so every
  preview shows all the layers as the render will draw them. Switched off, each step shows only its own layer (the
  OSD, or the SRT data) and the step of the sound shows none, in the picture and when it plays. The choice is
  remembered. It does not change what is rendered.

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
- The picture of the sound preview plays as a stream instead of being asked for four times a second: one FFmpeg
  process decodes the stretch already shrunk to the size of the preview, and each frame is shown when the sound gets
  to it, at up to 30 frames per second. It used to start FFmpeg for every picture, so it looked slow. The preview
  says that its quality is reduced, under the picture and in the sound step; the render is not affected.
- A preview that plays stops when the user moves to another step, and the sound no longer goes on in the steps after
  the sound step. A change made while it plays (a position, a field, the timing) shows after a short moment, when
  the picture starts again from where it is.
- The colour scheme and the last folder are kept by the window and not by the core library. The same
  `settings.toml` is used, and files written by earlier versions are read as before.

### Fixed

- The grid of the OSD cells (the one for hiding elements by clicking) stayed on the previews of the SRT data step and
  of the sound step when the choice of hiding elements was left on. It is on the picture of the OSD step only.
- A motor sound that finished being made after the user had moved to another step started playing there. It is
  not played.
- A video with no `.osd` file, or with an OSD of a firmware without telemetry, could not go past the sound step: the
  sound was on by default and its box could not be unticked.
- A picture could not be made at the very end of the video, so dragging the preview slider there showed an error.
- A second run over a folder tried to render the videos that the first run had written.
- The tests of the window crashed one run in five with the GPU renderer; they use the software renderer now.

- Project skeleton: Cargo workspace with `flight-replay-core` and `flight-replay`, Makefile, license files,
  dependency policy and continuous integration.
