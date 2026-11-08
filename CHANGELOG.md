# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A **Close** button on the last step, where Next is on the others, once a render has ended well and no queue is being
  worked through. It closes the program, saving the settings as the close button of the window does.
- A settings screen, reached by a button at the top right of every step (Escape closes it). It has FFmpeg, the colour
  scheme, the reset of the settings and an About. It opens by itself, once, when FFmpeg is missing or too old.
- **Install FFmpeg** in the settings: downloads one fixed GPL build for the system (Windows x86-64, macOS Apple
  Silicon, Linux x86-64) with `curl`, only when the button is clicked, checks its SHA-256, runs it once and keeps it in
  the data folder of the program. It shows the progress, can be cancelled, leaves the previous copy as it was when it
  fails, and can be removed from the settings. An FFmpeg of the user is always used before it.

### Fixed

- A second click on a button that opens a file dialog no longer opens a second dialog. The dialog of the system is a
  window of its own and the window of the program is not frozen under it, so the buttons could be clicked again and
  again. Now only one is open at a time, and the buttons that open one wait until it is closed.
- Looking for FFmpeg again (choosing its folder, "Check again", or the end of an install) no longer opens again the
  video given on the command line or the font used last, over what you have opened since.
- The check of free space before a render also works when the folder of the output does not exist yet.

### Changed

- The welcome step is a summary: the version of the program, FFmpeg (version and where it came from), the encoders
  that work, the colour scheme, the free disk space and the settings file. The colour scheme, the FFmpeg folder and the
  reset moved to the settings.

- An FFmpeg older than version 6 is skipped, and the search goes on to the next one, instead of the first FFmpeg that
  runs being taken. When every copy found is too old, step 1 now says so, with the version, instead of saying that
  FFmpeg was not found. Builds made from git, whose version cannot be read, are still accepted.
- The search looks last in the `ffmpeg` folder of the application's data folder, so an FFmpeg of the user always
  wins over a copy the application keeps.

## [0.1.0] - 2026-11-07

First release. The archives are for Linux (x86_64), Windows (x86_64) and macOS (Apple Silicon), and are not signed.

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
- Ready-made motor sounds for different electric motors and propellers (small park flyer, small high-KV motor, three
  blades, aerobatic power, large slow propeller, pusher, ducted fan), next to the default. They are a starting point:
  every value can still be changed, and the sound list says "Custom" when one was. Two more sliders, the whine pitch
  and the sharpness of the propeller noise.
- A choice of aircraft in the sound step, a fixed wing or a drone. A drone has its own sounds (5-inch freestyle, 7-inch
  long range, ducted cinewhoop) and its own settings: four motors that run at slightly different speeds and beat
  against one another, that never stop while it is armed (they idle where INAV shows `IDLE`, and are silent for `STOP`
  and `DARM`), and a current for full volume of 60 A. A section "Several motors" sets how many, how much they differ,
  how much they drift, and how much they follow the roll and pitch. A fixed wing sounds exactly as before, and settings
  files from before read as a fixed wing.
- An option to show the FFmpeg command before rendering.
- "Reset all settings…" in the welcome step. It asks for a confirmation, and then brings every setting and preference
  back to the default: the colour scheme, the FFmpeg folder, the OSD font, the saved sounds and the choices of every
  step. The video and the files that are open stay open.
- Play and Stop in the OSD step and in the SRT data step: ten seconds of the video from the time on the slider, with
  the OSD or the SRT data drawn on it, to check the layer in motion. It has no sound; the motor sound keeps its own
  step.
- "Also show the SRT data in the preview" in the step of the OSD, "Also show the OSD in the preview" in the step of
  the SRT data and "Also show the SRT and OSD data in the preview" in the step of the sound. It is on, so every
  preview shows all the layers as the render will draw them. Switched off, each step shows only its own layer (the
  OSD, or the SRT data) and the step of the sound shows none, in the picture and when it plays. The choice is
  remembered. It does not change what is rendered.

### Changed

- Every choice on the screens is saved as soon as it is made, and not only when the user moves to another step or
  closes the window, so nothing is lost if the program is killed. Several things were not saved at all and now are:
  the speed and the delay of the OSD, "Replace the file if it exists", "Replace it with the new sound", the start and
  the length of the stretch, and the saved sound that was chosen. A stretch is still off whenever the program opens.
  When the settings cannot be written (a folder that is read-only, a full disk) the window says so, once.
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

- A noise seed above 2^61 in `settings.toml` made a debug build stop with an arithmetic overflow, and a release build
  silently wrap the number. The seed is now any 64-bit number, and the sound is the same in both builds.
- The wind blew hard at the end of the video, with the aircraft already stopped. The statistics screen that INAV shows
  after disarming was not recognised on INAV 7 and earlier (its title is `--- STATS ---`, and only `*** STATS`, the
  title of INAV 8, was looked for), so its numbers were read as readings of the flight: the average speed (72 km/h)
  became the speed of the aircraft until the last frame. The same happened to the other totals on that screen (the
  maximum current, say). Both titles are known now. On a recording with no other ground speed the wind used to blow at
  the average speed over the whole video; there is now no wind, and the sound step says so.
- The motor sounded while it was stopped. The decoder read only a number after the throttle icon, so a recording that
  shows `STOP`, `IDLE` or `DARM` there (or a negative percentage) had no reading for those frames, and the sound held
  the last throttle through them. The frames before the first reading were also given the first reading. A
  recording that began with `STOP` on the screen had the motor at the first percentage seen for those seconds, and a
  flight whose throttle shows ` -1` while gliding had it at the last positive value. The words are now read as a
  throttle of 0, a negative percentage as shown, and the throttle is 0 until its first reading.
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
