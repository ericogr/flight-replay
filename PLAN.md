# Flight Replay: Implementation Plan

Revision 2, 2026-11-05. Revision 1 asked the owner for decisions. Their answers are recorded in section 3.
Items marked **default** were not answered and use the recommendation. Nothing is implemented yet.

## 1. Goal

Flight Replay is a cross-platform desktop application (Linux, Windows, macOS). It takes a Walksnail Avatar
recording and produces a new video with:

1. the flight controller OSD and the SRT telemetry burned onto the picture (feature set of
   walksnail-osd-tool), and
2. a synthesized electric-motor and propeller soundtrack, driven by the telemetry stored in the `.osd`
   recording (feature set of fpv-motorsound), for INAV and Betaflight OSD layouts,

configured through a step-by-step wizard built with [iced](https://iced.rs), and exported with the encoding
options the user chooses at the end.

## 2. Scope

### 2.1 Video overlay (features of walksnail-osd-tool)

- Inputs: video (MP4 from the goggles or a VRX), `.osd` file, `.srt` file, OSD font file. Fonts: 256 and 512
  glyph layouts, 2-colour (INAV) and 4-colour (Betaflight 4.5) files.
- Auto-discover the `.osd`, `.srt` and font files next to the video, by name. Allow manual override.
- Show video information: resolution, codec, frame rate, duration, frame count.
- Show the firmware named in the `.osd` header (`BTFL`, `INAV`, and others).
- OSD options: horizontal and vertical position, automatic centring, masking of OSD elements, and optional
  playback-speed correction for older firmware with OSD lag.
- SRT options: choose the data fields to render (time, distance, latency, signal, battery and the others the
  SRT carries), position and size.
- Chroma-key output: OSD on a green background, for compositing in a video editor.
- Upscale to 1440p.
- Encoders: H.264 and H.265, software (libx264, libx265) and hardware (NVENC, QSV, AMF, VideoToolbox, VAAPI).
  Detect which ones work. Optionally offer the ones that were not detected.
- Bitrate in Mbps.
- Preview of one frame with the overlay applied, before rendering.
- Render with progress, cancel and an error log.
- Remember the last-used settings.

### 2.2 Motor sound (features of fpv-motorsound)

- Telemetry read from the `.osd`. The profile comes from the firmware in the header:
  - **INAV:** throttle (required), current, air speed, ground speed, cell voltage, roll, pitch, G-force
    (optional). Rules ported from fpv-motorsound and fpv-osd-decoder.
  - **Betaflight:** the same signals, with rules written for Betaflight's OSD layouts. This is phase P4b and
    needs Betaflight recordings (section 13).
  - **Other firmware** (ArduPilot, KISS, and so on): the overlay works, the sound is not available.
- Coverage report before rendering: which signals were found and what each missing one costs the sound.
  Warnings for low coverage (a signal in fewer than half of the frames), throttle never above the threshold,
  and an end-of-telemetry that does not match the end of the video.
- Synchronisation: automatic time scale (`video_duration / (frames / 60)`), manual time scale, and an offset in
  seconds. A probe mode extracts frames at chosen instants and shows the decoded OSD values next to each frame.
- Synthesis model: pitch from throttle (phase accumulator, asymmetric smoothing), volume and propeller noise
  from current, wind from air speed, stereo balance from roll. Output is deterministic.
- Audio parameters grouped as motor, tone, propeller, wind, loudness and smoothing, with named presets.
- Audio preview: render N seconds from a chosen start time and play them. The picture and the OSD follow the sound while it plays, so the timing can be judged by eye.
- Output: AAC audio muxed with the video. When no overlay is requested, the video stream is copied unchanged.

### 2.3 Non-goals for v1

- Video playback inside the GUI. The GUI shows single-frame previews only.
- Editing, trimming or concatenating videos. (Joined videos from several `.osd` files are in P8.)
- A command-line interface. The core is a library with no GUI dependency, so a CLI can be added later as a new
  crate without changing the core (section 5).
- CSV, KML and other exports that fpv-osd-decoder offers. The rules are ported, so an export can be added later.
- DJI or other video systems.
- Installers, code signing and notarisation. Phase 9 covers archives and the macOS app bundle.

## 3. Decisions

| ID | Topic | Decision | Status |
|----|-------|----------|--------|
| D1 | License | **GPL-3.0-only.** The owner accepts GPL-3.0 for the whole program. The text is in `LICENSE`. The `-only` form is required: walksnail-osd-tool is GPL-3.0-only, and code under it cannot be relicensed to a later version. | decided |
| D2 | Reuse of code | **Allowed.** Port and adapt walksnail-osd-tool's backend (GPL-3.0). Port fpv-motorsound (Go, Apache-2.0) and fpv-osd-decoder (Go, MIT); both are the owner's code. Notices are kept (section 4). | decided |
| D3 | Configuration format | TOML (`serde` + `toml`). fpv-motorsound uses YAML, but `serde_yaml` is deprecated. | default |
| D4 | Telemetry for the sound | **INAV and Betaflight.** INAV first (P4). Betaflight after the recordings are available (P4b). | decided |
| D5 | Command-line interface | **None in v1.** The core is designed so a CLI can be added later as a separate crate. No CLI code now. | decided |
| D6 | FFmpeg | External program, found on the system, in v1. Bundling is allowed under GPL-3.0. Decided in P9: not bundled in v0.1.0 (see `docs/build.md`). A folder next to the executable is searched, so a portable folder works. | default, confirmed in P9 |
| D7 | Test fixtures | **Committed.** Trimmed clips of a few seconds, each with a matching `.osd` and `.srt` slice and the font, under `tests/fixtures/`. Full recordings stay local. GPS coordinates in the committed `.osd` are replaced by fake open-ocean ones. Rules in section 10.4. | decided |
| D8 | Names | **Flight Replay** as the product name everywhere. Each context uses its own convention (table in section 5.0). | decided |
| D9 | Output container | MP4 only in v1. | default |
| D10 | Input video already has audio | Error by default, with a `replace` option (like `--replace-audio` in fpv-motorsound). | default |
| D11 | Update check at startup (walksnail-osd-tool has one) | Not in v1. It needs network access. | default |
| D12 | Audio preview in the GUI | Yes, with `rodio`. Needs ALSA headers on Linux at build time. | default |

## 4. Licensing and reuse

### 4.1 Origins of the code

| Source | License | How it is used | Obligations |
|--------|---------|----------------|-------------|
| walksnail-osd-tool, by Alexander van Saase | GPL-3.0-only | Backend ported and adapted: `ffmpeg`, `font`, `osd`, `overlay`, `srt`, `config`, `util`. The egui UI in `ui/` is a reference only; the iced views are written new. | Keep the copyright headers. Mark each changed file as modified. Keep the GPL text. Provide the source with every binary release. |
| fpv-motorsound, by EricoGR | Apache-2.0 | Ported to Rust: telemetry rules, synthesis, timing, media and render pipeline, configuration defaults. | Keep the Apache notice in `NOTICE`. Mark ported files as changed. Apache-2.0 is compatible with GPL-3.0. |
| fpv-osd-decoder, by EricoGR | MIT | INAV glyph table and telemetry rules (directly and through fpv-motorsound). | Keep the MIT copyright and permission notice. |
| Crates (section 5.5) | MIT, Apache-2.0, BSD, and so on | Libraries. | Listed in `THIRD_PARTY_NOTICES.md`. |
| FFmpeg | LGPL or GPL, depending on the build | Separate program, started as a process, never linked. | None while it is not bundled. |

### 4.2 Rules

- **Project license:** GPL-3.0-only. Every source file starts with an SPDX header, `// SPDX-License-Identifier: GPL-3.0-only`.
- **Ported files** start with a header that names the origin, the original license and the change. Example:
  `// Adapted from walksnail-osd-tool, backend/src/osd/osd_file.rs (GPL-3.0-only), Copyright (C) Alexander van Saase.`
  `// Modified for Flight Replay. Changes are listed in the git history.`
- **Dependencies** must be GPL-3.0-compatible. `cargo deny` allows: MIT, Apache-2.0 (with and without LLVM
  exception), BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, CC0-1.0, 0BSD, MPL-2.0, LGPL-2.1-or-later,
  LGPL-3.0-or-later, GPL-3.0-only. It denies AGPL, GPL-2.0-only, and any unknown or proprietary license.
  A dependency that fails the check fails the build. The allow list also contains BSL-1.0, Unicode-DFS-2016 and
  MIT-0, which are permissive.
- **Advisories:** six unmaintained crates come through iced 0.14 (`paste`, `pin-utils`, `ttf-parser`, `zvariant`,
  `zvariant_derive`, `zbus_names`). They are not vulnerabilities, and no safe upgrade exists. `deny.toml` lists
  each one as an exception with its reason. The exceptions are reviewed on each iced update.
- **Fonts** are data files. They ship with their own license text (SIL OFL 1.1), and are not compiled into code.
- **FFmpeg** is not bundled in v1. Its build options decide its license, so a bundled build needs its own
  notice and source offer, decided in P9.
- **Notices:** `NOTICE` (hand-written, lists the origins above). `THIRD_PARTY_NOTICES.md` is generated for the
  crates with `cargo about` (`make notices`).

## 5. Architecture

### 5.0 Names

The product name is **Flight Replay**. Each place uses the form its convention expects:

| Where | Form | Example |
|-------|------|---------|
| Product name: window title, About dialog, README title, documentation | Title Case, with a space | Flight Replay |
| Repository and folder | kebab-case | `flight-replay` |
| Cargo package for the GUI, and its binary | kebab-case | `flight-replay`, `flight-replay.exe` |
| Cargo package for the core | kebab-case (module path uses snake_case) | `flight-replay-core`, imported as `flight_replay_core` |
| Windows product name (version resource) | Title Case | Flight Replay |
| macOS app bundle | Title Case, with a space, and `.app` | `Flight Replay.app` |
| macOS bundle identifier | reverse DNS, lowercase | `io.github.ericogr.flight-replay` (**default**) |
| Linux desktop entry | kebab-case file, Title Case name | `flight-replay.desktop`, `Name=Flight Replay` |
| Release archives | kebab-case | `flight-replay-0.1.0-linux-x86_64.tar.gz` |
| Git tags | `v` and semantic version | `v0.1.0` |
| Environment variables | UPPER_SNAKE_CASE, with the prefix | `FLIGHT_REPLAY_FFMPEG` |
| Settings file | kebab-case | `settings.toml`, in the platform config folder |
| Default output file | the video name plus a kebab-case suffix | `clip_flight-replay.mp4` |
| Rust types | PascalCase | `FlightReplay` for the iced application type |
| Makefile variable | kebab-case | `NAME := flight-replay` |

### 5.1 Workspace layout

```text
flight-replay/
├── Cargo.toml              workspace: crates/*, xtask; shared dependencies and lints
├── Cargo.lock              committed (this is an application)
├── rust-toolchain.toml     pinned stable toolchain, with rustfmt and clippy
├── rustfmt.toml, clippy.toml, deny.toml (license, advisory and source policy, section 4.2)
├── about.toml, about.hbs   cargo-about configuration and template (make notices)
├── .cargo/config.toml      the `cargo xtask` alias, and the software renderer for the GUI tests
├── Makefile
├── PLAN.md                 this file
├── LICENSE, NOTICE         GPL-3.0 text; origins and copyright of ported code
├── THIRD_PARTY_NOTICES.md  generated (make notices)
├── README.md, CHANGELOG.md, CONTRIBUTING.md (CHANGELOG follows Keep a Changelog)
├── assets/                 fonts (OFL) and the icon (SVG, PNG sizes, ICO)
├── packaging/              Linux desktop entry and install.sh; macOS Info.plist
├── docs/                   architecture.md, build.md, audio-model.md, performance.md, formats/, telemetry/
├── tests/
│   ├── fixtures/           trimmed clips (D7)
│   └── golden/             reference images for overlay tests
├── xtask/                  developer automation written in Rust (fixtures, packaging)
├── crates/
│   ├── core/   (flight-replay-core)  library, no GUI dependency
│   └── gui/    (flight-replay)       iced application, binary `flight-replay`
└── .github/workflows/      ci.yml, release.yml
```

`xtask` is a developer tool, not a user CLI. Make calls it for tasks that are awkward in shell (cutting
fixtures, building archives). The golden images are rewritten by a test run with
`FLIGHT_REPLAY_UPDATE_GOLDEN=1` (`make golden`). `docs/architecture.md` describes the code as it is built.

### 5.2 Core modules (`crates/core`)

Where each module comes from. What each one does is in `docs/architecture.md`.

| Module | Origin |
|--------|--------|
| `ffmpeg` | adapted from walksnail `ffmpeg/` (dependencies, encoders, video_info, message); probe and the stream-copy mux from fpv-motorsound `internal/media` |
| `osd` | merges walksnail `osd/` and fpv-motorsound `internal/osd`; `join` is new |
| `font` | adapted from walksnail `font/` |
| `srt` | adapted from walksnail `srt/` |
| `overlay` | adapted from walksnail `overlay/`, without the egui types |
| `telemetry` | the decoder, the INAV glyph table and rules, the series and the table are ported from fpv-motorsound `internal/osddecode`, `internal/osdsource`, `internal/inav` and `internal/telemetry`; the glyph table also from fpv-osd-decoder |
| `timeline` | merges walksnail `overlay/iter.rs` and fpv-motorsound `internal/timesync` |
| `synth` | ported from fpv-motorsound `internal/synth` |
| `audio` | ported from fpv-motorsound `internal/media/wav.go` |
| `config` | adapted from walksnail `config.rs` (confy replaced by TOML); defaults from fpv-motorsound `internal/config` |
| `render` | adapted from walksnail `ffmpeg/render.rs`; ported from fpv-motorsound `internal/pipeline`; `builder` and `folder` are new |
| `discover` | adapted from walksnail and from fpv-motorsound `internal/pipeline/discover.go` |
| `paths`, `display` | new |

A Betaflight table and rules (phase P4b) would be a `telemetry::betaflight` module next to `inav`.

Rules for the core crate:

- `#![forbid(unsafe_code)]`.
- Typed errors with `thiserror`. The GUI uses `anyhow` where it needs it.
- No printing to stdout or stderr. Logs go through `tracing`. Progress goes out as events on a channel.
- No type from iced or any other GUI crate appears in the public API.
- Every stage can be tested without the GUI.

### 5.3 Render pipeline

The wizard chooses one of two modes.

**A. Audio only (no overlay).** The video stream is copied with `-c:v copy`, so it stays bit-identical.
The synthesized PCM is written to the stdin of an ffmpeg process that encodes AAC and muxes the result.
This is the approach of fpv-motorsound.

**B. Overlay, with or without audio.**

1. A decoder process runs `ffmpeg -i <video> -f rawvideo -pix_fmt rgba -`, which yields frames.
2. For each frame, `overlay::compose(frame, timeline, osd, srt, style)` returns an RGBA frame. Compositing runs
   in parallel over row bands with `rayon`. The frame is optionally scaled to 1440p.
3. The composed frames are written to an encoder process:
   `ffmpeg -f rawvideo -pix_fmt rgba -s WxH -r FPS -i - -i <audio.wav> -map 0:v -map 1:a -c:v <encoder> ... -c:a aac <out>`.
4. When audio is requested, it is first rendered to a temporary 16-bit PCM WAV in the output directory. A
   32-minute stereo file is about 370 MB, so free space is checked before the render starts. Named pipes are a
   later optimisation.

**Chroma-key mode** does not decode the source video. Frames are generated from the background colour, using
the frame count and duration from the timeline.

**Progress** comes from the decoder and the encoder, plus the audio blocks.

**Cancel** kills the child processes and deletes the partial output and the temporary WAV.

**Post-render check:** the result is probed. Video and audio durations must agree within 1 ms, as in
fpv-motorsound. Any mismatch is reported as a warning.

The walksnail backend already has a pipeline for the overlay case. It is the starting point for B, and the
audio step from fpv-motorsound is added to it.

### 5.4 Timeline and synchronisation

- One function maps video time to an OSD frame index (overlay) and to a telemetry time (sound).
- Telemetry time: `t_tel = (t_video - offset) / scale`. Automatic scale: `scale = video_duration / osd_duration`,
  where `osd_duration` is the duration of the `.osd` recording (last screen plus one average interval). This is
  walksnail-osd-tool's method. fpv-motorsound uses `nb_frames / 60` instead. Both agree within 0.02 % on its
  sample flights, but on a raw goggle recording (AvatarG0153) the frame-count method is 5 % off: the OSD is
  stamped in real time there, so the OSD duration is the right one. The decision is recorded in
  `crates/core/src/timeline.rs`.
- Overlay: the OSD screen is the last one recorded at or before the telemetry time. The option to turn the
  correction off (walksnail-osd-tool's "adjust playback speed") is `ScaleMode::Off`.
- Acceptance: on the sample flights, the flight timer on the video and the telemetry agree at several points.
  This covers both features at once and catches any disagreement between them.

### 5.5 Libraries

Versions were checked with `cargo info` on 2026-11-05. The toolchain was updated in P0 to 1.99.1 (the local
rustc was 1.84). The minimum supported Rust version is 1.88, set by iced 0.14. Versions are pinned in P0 and
checked again with `cargo deny`.

| Purpose | Crate | License |
|---------|-------|---------|
| GUI | `iced` 0.14 (edition 2024) | MIT |
| Native file dialogs | `rfd` | MIT |
| Audio playback (preview) | `rodio` 0.23 (needs Rust 1.87) | MIT OR Apache-2.0 |
| WAV writing | `hound` | Apache-2.0 |
| FFmpeg process helper | `ffmpeg-sidecar` 2.7 (used by walksnail; keep or replace in P1) | MIT |
| Images, decoding and PNG | `image` 0.25 (walksnail uses 0.24) | MIT OR Apache-2.0 |
| Image processing (if still needed) | `imageproc` 0.27 (walksnail uses 0.23) | MIT |
| SRT text rendering | `ab_glyph` 0.2 (maintained successor of `rusttype`, which walksnail uses) | Apache-2.0 |
| Configuration | `serde`, `toml`, `directories` | MIT OR Apache-2.0 |
| ffprobe JSON | `serde_json` | MIT OR Apache-2.0 |
| Errors | `thiserror` (core), `anyhow` (GUI) | MIT OR Apache-2.0 |
| Logging | `tracing`, `tracing-subscriber` | MIT |
| Parallel compositing | `rayon` | MIT OR Apache-2.0 |
| Temporary files | `tempfile` | MIT OR Apache-2.0 |
| Snapshot tests (optional) | `insta` | Apache-2.0 |
| Developer tools, not dependencies | `cargo-deny`, `cargo-about` | MIT OR Apache-2.0 |

The walksnail backend also depends on `srtparse`, `confy` and an `ffprobe` fork from git. Their licenses and
maintenance are checked in P0. The fork is replaced by a published crate or by the project's own probe code.

## 6. Wizard (GUI)

The wizard is a state machine in `crates/gui/src/wizard/`. It is plain Rust and is unit tested. Each step has
a view module in `crates/gui/src/views/`. Views hold no business logic. Each step sends iced messages and
reads the state.

| # | Step | Content | Can be skipped |
|---|------|---------|----------------|
| 1 | Welcome and environment | Are `ffmpeg` and `ffprobe` found? Versions. Encoders detected. | no |
| 2 | Input | Pick or drag the video. Auto-discover `.osd`, `.srt` and font. Video information. Firmware from the `.osd`. Warnings for missing files. | no |
| 3 | Video overlay | Enable OSD. Font. Position, centring, masks (drawn on the preview), latency correction. Frame preview with a time slider. | yes (audio only) |
| 4 | SRT data | Enable SRT. Fields. Position. Size. Preview. | yes |
| 5 | Motor sound | Enable sound. Profile (INAV, Betaflight, or not available). Motor preset (ready-made sounds for kinds of motor and propeller) and saved presets. Coverage report. Synchronisation (scale, offset, probe). Parameters, grouped and collapsed. Audio preview. | yes (overlay only) |
| 6 | Output | Container. Video: copy or encoder, bitrate, 1440p upscale, chroma key. Audio: AAC bitrate, sample rate, channels, gain. Output path. Overwrite policy. | no |
| 7 | Review and render | Summary. FFmpeg command line, copyable, shown only when the user ticks the option (off by default). Start, progress with an estimate, cancel, open folder when done. | no |

Rules:

- Overlay or sound must be enabled. Otherwise step 7 is blocked, with a message.
- When the firmware has no sound profile, step 5 says so and the sound cannot be enabled. The overlay still works.
- Going back keeps the state. Changing an input (for example the video) asks before resetting the steps that depend on it.
- Each step validates before moving on, and shows the message inline.
- Decoding and synthesis run in background tasks, so the UI never blocks.
- Settings persist in the user config folder. Named presets are stored there too.
- English only. No localisation in v1.

## 7. Output options (step 6)

| Option | Values | Default |
|--------|--------|---------|
| Container | MP4, with faststart | MP4 |
| Video codec | H.264, H.265 | H.264 |
| Encoder | Software, or a detected hardware encoder. A toggle shows undetected ones. | `libx264` |
| Bitrate | Mbps | 40 |
| Upscale | off, or 1440p | off |
| Background | Source video, or chroma-key green | source video |
| Audio codec | AAC | AAC |
| Audio bitrate | kbps | 192 |
| Sample rate | 44.1 kHz, 48 kHz | 48 kHz |
| Channels | mono, stereo | stereo |
| Master gain | dB | -8 dB |
| Output path | File dialog. Default `<name>_flight-replay.mp4`, next to the video. | next to the video |
| Overwrite | refuse, or ask | refuse |

## 8. Multiplatform

- **Build and test targets:** `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
  `x86_64-pc-windows-msvc`, `aarch64-apple-darwin` and `x86_64-apple-darwin`. Each target is built on a native
  runner where one is available. CI runs the tests on Linux, Windows and macOS.
- **FFmpeg discovery order:** (1) the path set in the settings or in `FLIGHT_REPLAY_FFMPEG`, (2) `PATH`,
  (3) well-known locations: `/opt/homebrew/bin`, `/usr/local/bin`, `C:\ffmpeg\bin`, the WinGet links folder,
  (4) the folder next to the executable. A macOS app started from Finder does not inherit the shell `PATH`,
  which is why step 3 exists.
- **Windows:** release builds use `windows_subsystem = "windows"`, so no console window opens. Spawned
  processes use `CREATE_NO_WINDOW`. Paths are `PathBuf` and `OsString`, and no command goes through a shell.
  Tests cover file names with spaces and non-ASCII characters. The executable has the Flight Replay icon and
  version resource.
- **macOS:** the `Flight Replay.app` bundle with `Info.plist` and icon. Unsigned releases are documented, with
  the first-run steps.
- **Linux:** a `flight-replay.desktop` entry and icons in the standard sizes. `wgpu` needs a Vulkan or GL-capable
  GPU. A software fallback is available through iced's `tiny-skia` renderer. `rfd` and `rodio` need their
  system packages, listed per distribution in `docs/build.md`.
- **Hardware encoders:** NVENC (NVIDIA, Windows and Linux), QSV (Intel, Windows and Linux), AMF (AMD, Windows),
  VideoToolbox (macOS), VAAPI (Linux). Detection runs a one-frame test encode. Listing the encoder is not enough.
- **Display:** HiDPI and fractional scaling must work. Test at 150 %.

## 9. Build, quality and automation

### 9.1 Makefile targets

GNU Make, in the same style as the fpv-motorsound Makefile (`make help` lists the targets).

| Target | Action |
|--------|--------|
| `build` | Debug build of all crates |
| `release` | Optimised build (`lto = "thin"`, `codegen-units = 1`, stripped) |
| `run` | `cargo run -p flight-replay`. Extra arguments in `ARGS` |
| `test` | Unit and integration tests. Tests that need FFmpeg print a message and skip when it is missing |
| `fmt` | Format with rustfmt |
| `fmt-check` | Fail if formatting differs |
| `lint` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `deny` | `cargo deny check` (licenses, advisories, sources, duplicates) |
| `check` | `fmt-check`, `lint`, `test`, `deny`. Run before every commit |
| `fixtures` | Cut the trimmed fixture clips from the local recordings (`xtask`). Explicit only |
| `golden` | Regenerate the reference images. Explicit only, never run by `test` |
| `notices` | Regenerate `THIRD_PARTY_NOTICES.md` with `cargo about` |
| `app-bundle` | macOS: build `Flight Replay.app` |
| `dist` | Release archives for the current platform, plus `SHA256SUMS` (same layout as fpv-motorsound) |
| `install` | `cargo install --path crates/gui` |
| `clean` | `cargo clean` and remove `dist/` |

The `fixtures`, `golden`, `notices`, `app-bundle` and `dist` targets are added in the phases that need them (P1, P2
and P9). Phase 0 provides the others.

Lints: `[workspace.lints]` sets `unsafe_code = "forbid"` and `clippy::pedantic` as warnings, with a short allow
list. The core crate also sets `missing_docs`.

### 9.2 Commit and contribution rules

- All code, comments, documentation and commit messages are in English.
- Conventional Commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.
- `make check` passes before each commit.
- No AI co-author trailers in commits or pull requests.
- A ported file keeps its origin header (section 4.2) in every commit that touches it.

### 9.3 CI and release

- `ci.yml`: a matrix over Linux, Windows and macOS. Installs FFmpeg and the system packages, then runs `make check`.
- `release.yml`: runs on tags `v*`. Each runner builds `make dist` and uploads the archives and checksums to a
  GitHub Release. The release includes the source archive, as GPL-3.0 requires.

## 10. Testing and validation

### 10.1 Unit tests

- **timeline:** offset, scale, the automatic scale formula, and edge cases (zero frames, very large offset).
- **synth:** same seed gives identical bytes; output does not depend on the block size; the frequency matches the
  throttle mapping; silence below the throttle threshold; the limiter keeps samples within ±1.0.
- **osd, font and srt parsers:** fixtures, and malformed input that must return errors and never panic.
  Random-input tests.
- **profile:** firmware string to profile, including unknown firmware.
- **telemetry:** holds, back-fill and coverage statistics.
- **config:** TOML round trip, defaults, validation messages.
- **wizard:** state transitions. A path with neither overlay nor sound is blocked. Skip and reset rules. Step 5
  is locked for firmware without a sound profile.

### 10.2 Integration tests (need FFmpeg)

- **copy mode:** the video stream MD5 equals the source's. Audio duration equals video duration within 1 ms.
- **overlay mode:** output frame count equals input frame count. Decoded frames at three timestamps match the
  golden images within a per-pixel tolerance.
- **chroma key:** background pixels equal the key colour exactly.
- **cancel:** no partial file and no temporary WAV remain.
- **encoder detection:** parsing of recorded `ffmpeg -encoders` output.

### 10.3 Reference checks against the originals

These use the original tools as oracles. Their outputs are data, so comparing against them does not copy code.

- **Telemetry rules (INAV):** the values of the eight sound signals equal the columns of `AvatarG0031.csv`
  (written by fpv-osd-decoder from `AvatarG0031.osd`) on the same frames.
- **Statistics:** `flight-replay`'s telemetry summary equals `motorsound --dry-run -v` on the same clip.
- **Probe values:** the decoded values at the probe instants equal `motorsound probe`.
- **Overlay:** position and content match walksnail-osd-tool's output for the same options (visual check).
- **Sound:** the frequency at sample points is within 0.1 % of the value predicted from the OSD frame.
- **Copy mode:** the video stream is unchanged (MD5).

### 10.4 Fixture rules (D7)

- **Source.** Clips come from raw goggle recordings in the owner's local folder. Files with `_with_osd` in the
  name are never used, because they can have the OSD burned into the picture.
- **Length and size.** A clip is about 3 seconds long. The raw recordings are about 14 Mbps, so a clip is about
  5 MB. The total size of `tests/fixtures/` stays under 15 MB.
- **Same window.** The `.osd` and `.srt` are sliced to the video's time window. The slicing uses the core's
  parser, and a test checks that the sliced `.osd` frame count equals the sliced video frame count.
- **GPS anonymization.** The `.osd` stores the GPS coordinates as glyphs in the screen grid
  (`osd_gps_lat` and `osd_gps_lon`), and the decoder reads them. The committed clip has them replaced:
  - each real track is moved by one constant offset to an open-ocean point in the South Atlantic, at least
    500 km from any coast. A constant offset keeps the track's shape, speed and distances, so the telemetry
    stays consistent. Random points would not;
  - the sign and the number of integer digits are kept, so each value keeps the width of its text on screen;
  - only the digit glyphs of those cells change. The rest of each screen is unchanged;
  - the offset is computed while the command runs: the fake target minus the first real position in the clip. It is
    never stored. The target is fake, so it is kept in the repository (`cargo xtask fixtures`).
- **Checks after anonymization.** Decoded latitude and longitude equal the expected values within 1e-6. Every
  other telemetry column is identical to the decode of the original clip. The command also refuses a clip whose
  OSD shows a plus code. The fixture tests read the clip itself, so there is no separate reference file.
- **Other content.** The SRT is checked for coordinates (none in the sample). The video picture is not
  anonymized: the raw recording shows the place from above, so the owner decides whether the clip may be
  committed (section 14).

### 10.5 Manual validation on real flights (not committed)

- Full flights: G0050 and G0051 (fpv-motorsound) and AvatarG0031 (fpv-osd-decoder).
- Performance baseline, measured in phase 6. The target is set after that measurement, not before.
- A walk-through of the wizard on each of the three operating systems.

## 11. Milestones

| Phase | Deliverable | Acceptance |
|-------|-------------|------------|
| P0 Bootstrap | Repository licensing (GPL-3.0 `LICENSE`, `NOTICE`), `deny.toml`, Makefile, toolchain update, CI on three OSes, workspace with `core` and an empty iced window | `make check` passes on the three OSes. The window opens on each OS |
| P1 Media layer | Port of the walksnail `ffmpeg` module and fpv-motorsound's probe and mux. Encoder detection. Decode and encode pipes. Copy-mode mux. Cancel and partial files | Copy-mode MD5 test passes. Encoder detection test passes |
| P2 OSD, font and overlay | Port of the walksnail `osd`, `font` and `overlay` modules, with one merged `.osd` parser. Masks, positions, centring, latency correction, firmware field, frame preview | Golden tests pass. Visual match with walksnail-osd-tool on a fixture |
| P3 SRT | Port of the walksnail `srt` module. Fields, text rendering, position and size | Fields match walksnail-osd-tool on a fixture. Golden tests pass |
| P4 INAV telemetry and timeline | Port of the INAV rules and glyph table. Automatic scale, offset, summary, probe | Values match `AvatarG0031.csv` for the eight signals. Statistics match `motorsound --dry-run -v` |
| P4b Betaflight telemetry | Betaflight glyph table from Betaflight's symbol header. Rules per element. Profile selection | On user-supplied Betaflight recordings, values match the readings on screen at sample points. **Blocked** until recordings are available (section 13) |
| P5 Sound engine | Port of the synth, WAV output, preview segment | Duration check passes. Frequency within 0.1 % at sample points. Output matches fpv-motorsound's within the tolerance set in P5. Determinism tests pass |
| P6 Render pipeline | Overlay, sound, encoder options, 1440p upscale, chroma key, post-render check | End-to-end run on a fixture with every option. Baseline performance measured |
| P7 GUI wizard | Seven steps, persistence, previews, audio preview, drag and drop, light and dark themes, desktop entry and app bundle | Wizard state tests pass. Manual walk-through on three OSes |
| P8 Batch and joined videos | Queue of several videos in the GUI. One video with several `.osd` files | A queue of fixtures renders. A joined fixture passes the coverage check |
| P9 Release | Archives, `.app` bundle, Windows zip (installer later), release workflow, source archive, documentation. FFmpeg bundling decision (D6) | Tagged `v0.1.0` installs and runs on three OSes |

Dependencies: P0 → P1 → (P2, P3 and P4 in parallel) → P4b (when the recordings are ready) → P5 → P6 → P7 → P8 → P9.
P4b does not block P5, P6 or P7. Until it is done, Betaflight files get the overlay and the SRT, and no sound.

### 11.1 Task checklist

Each task is `[ ]` until it is done, then `[x]`. The phases are the ones in the table above. Tasks marked with a
phase that is not started yet are not blocked by anything else unless the text says so.

**Planning and research**

- [x] Survey walksnail-osd-tool, fpv-motorsound and fpv-osd-decoder (code, docs, licenses)
- [x] Check the licenses of the three projects (GPL-3.0, Apache-2.0, MIT)
- [x] Check the versions and licenses of `ffmpeg-sidecar`, `image`, `imageproc`, `rusttype`, `ab_glyph` and `ffprobe`
- [x] Check the licenses of `srtparse` (MIT) and `confy` (MIT/X11 OR Apache-2.0)
- [x] Check that fpv-osd-decoder has no Betaflight rules (INAV only)
- [x] Read the `.osd` layout used by fpv-motorsound (40-byte header, `u32` timestamp, `u16` glyph codes per cell)
- [x] Check that the `.osd` holds GPS coordinates (`osd_gps_lat`, `osd_gps_lon`)
- [x] Check that the SRT holds no coordinates
- [x] Check that the raw goggle recording has no OSD burned in (one frame at 120 s checked)
- [x] Find the local recordings (SD-card folder); `AvatarG0007.mp4` is corrupt (no `moov` atom), so it is not used
- [x] Write `PLAN.md` (revision 1), then update it to revision 2 with the owner's answers, then revision 3
- [x] Record the decisions D1 to D12
- [ ] Commit `PLAN.md` (only when the owner asks)

**Phase 0: bootstrap**

- [x] Update rustc to 1.99.1 with `rustup update` (MSRV 1.88, set by iced 0.14)
- [x] Add `LICENSE` (GPL-3.0 text) and `NOTICE`
- [x] Add `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml` and `deny.toml` (section 4.2 allow list)
- [x] Create the workspace with `crates/core` (`flight-replay-core`) and `crates/gui` (`flight-replay`)
- [x] Add `xtask` (it cuts the fixtures; section 5.1)
- [x] Add the SPDX header and the origin header template (`CONTRIBUTING.md`, section 4.2)
- [x] Write the Makefile (phase 0 targets; the others are added in their phases, section 9.1)
- [x] Add `.gitignore`: `target/`, `dist/`, `*.partial`, temporary WAV files, `local/`
- [x] Add `README.md` (title "Flight Replay") and `CHANGELOG.md`
- [x] Open an empty iced window in `flight-replay`
- [x] Add the CI workflow `ci.yml` for Linux, Windows and macOS (written; not yet run on GitHub)
- [x] `make check` passes on Linux
- [ ] `make check` passes on Windows (CI)
- [ ] `make check` passes on macOS (CI)
- [x] The empty window opens on Linux
- [ ] The empty window opens on Windows
- [ ] The empty window opens on macOS

**Fixtures (clips from P1; the `.osd` parts need P2 and P4)**

- [x] Create `local/` (git-ignored) for the raw recordings and the anonymization offset (not needed: the offset is computed at run time and never stored)
- [x] Choose a clip of about 3 seconds from a raw recording (never a file with `_with_osd` in the name)
- [x] Cut the video with `-c copy`, so the stream is the source's
- [x] Cut the `.osd` to the same frames, with the core's parser
- [x] Cut the `.srt` to the same time window
- [x] Move the real track to a fake open-ocean point in the South Atlantic (about 35.2 S, 31.4 W) by a constant
      offset, keeping the sign and the number of integer digits. The offset is computed at run time
- [x] Rewrite the GPS digit glyphs in the `.osd` clip (same cells, same number of digits)
- [x] Verify: decoded latitude and longitude equal the expected values within 1e-6 on every frame that shows them
- [x] Verify: every other telemetry column equals the decode of the original clip
- [x] Regenerate the fixture's reference values from the anonymized clip (the fixture tests read the clip itself)
- [x] Check the clip's SRT for coordinates (none expected) and record the result
- [x] Owner decides whether the clip's picture (aerial view of a city) may be committed, or a cropped clip is used (owner: either is fine)
- [x] Record the fixture changes in `tests/fixtures/README.md`: which file, what changed, and why
- [x] Check that the total size of `tests/fixtures/` is under 15 MB (5.7 MB)

**P1: media layer**

- [x] Port the walksnail `ffmpeg` module (dependencies, encoders, video info)
- [x] Replace the `ffprobe` git fork with a crates.io release or with the project's own probe code
- [x] Encoder detection with a one-frame test encode
- [x] Decode pipe (RGBA frames)
- [x] Encode pipe (raw video on stdin, WAV input)
- [x] Copy-mode mux (`-c:v copy`)
- [x] Cancel, and remove partial files
- [x] Test: the copy-mode video MD5 equals the source's
- [x] Test: the audio duration equals the video duration within 1 ms
- [x] Test: FFmpeg tests skip with a message when FFmpeg is missing

**P2: OSD, font and overlay**

- [x] Merge the `.osd` parsers (walksnail `osd/` and fpv-motorsound `internal/osd`) into one
- [x] Select the telemetry profile from the firmware field (the field is read in the parser)
- [x] Port the font module (256 and 512 glyphs, 2-colour and 4-colour)
- [x] Port the overlay compositor
- [x] Masks, position, centring and latency correction
- [x] Frame preview in the core (the GUI comes in P7)
- [x] Golden tests for the overlay (synthetic font, flat picture)
- [ ] Visual comparison with walksnail-osd-tool, same settings
- [ ] Upgrade `image` to 0.25 and check that the output does not change (checked by eye only so far)

**P3: SRT**

- [x] Port the walksnail `srt` module
- [x] Check the maintenance of `srtparse`, and keep it or replace it (replaced: a small parser in the project)
- [x] Field selection
- [x] Text rendering with `ab_glyph` (replaces `rusttype`)
- [x] Position and size
- [x] Golden tests
- [ ] Fields match walksnail-osd-tool on the fixture

**P4: INAV telemetry and timeline**

- [x] Port the INAV glyph table (from fpv-motorsound's `symbols.go`, which comes from fpv-osd-decoder)
- [x] Port the INAV rules for the eight sound signals
- [x] Port the telemetry series, holds and coverage
- [x] Implement the automatic time scale, the offset and the shared `timeline` module
- [x] Values match the reference CSV of AvatarG0031 (full flight)
- [x] Summary matches `motorsound --dry-run -v`
- [x] Probe mode matches `motorsound probe`

**P4b: Betaflight telemetry (blocked on recordings)**

- [ ] Owner provides Betaflight recordings (`.osd`, video, font, Betaflight version)
- [ ] Obtain Betaflight's OSD symbol header and generate the glyph table
- [ ] Write the Betaflight rules in `docs/telemetry/betaflight.md`
- [ ] Implement the rules in the `betaflight` module
- [ ] Validate the values against the readings on screen, at sample points
- [ ] The profile enables the sound for `BTFL` files

**P5: sound engine**

- [x] Port the synth (blocks, smoothers, filters, limiter)
- [x] Port the WAV writer (a small writer in the project, so no `hound` dependency)
- [x] Determinism tests: same seed gives the same bytes; the output does not depend on the block size
- [x] Frequency tolerance test (within 0.1 %)
- [x] Compare with fpv-motorsound's output on the same telemetry
- [x] Audio preview segment in the core

**P6: render pipeline**

- [x] Combine overlay and sound (mode B)
- [x] 1440p upscale
- [x] Chroma-key background
- [x] Post-render check (durations within 1 ms)
- [x] Free disk-space check before the temporary WAV
- [x] Measure the performance baseline
- [x] End-to-end test on the fixture with every option

**P7: GUI wizard**

- [x] Wizard state machine with unit tests
- [x] Step views 1 to 7
- [x] Settings persistence (`settings.toml`)
- [x] Frame preview in steps 3 and 4
- [x] Audio preview in step 5 (`rodio`): checked on Linux in the real window: the player shows "Playing…" and PipeWire has a `flight-replay` playback stream; nobody has listened to it yet
- [x] Drag and drop: checked on Linux (X11) by dragging a file from Thunar onto the window
- [x] Light and dark themes
- [x] Linux desktop entry and macOS app bundle metadata
- [ ] Manual walk-through on Linux, Windows and macOS

**P8: batch and joined videos**

- [x] Queue of several videos in the GUI
- [x] One video with several `.osd` files (joined)
- [x] Coverage check for joined videos

**P9: release**

- [ ] `make dist` archives for each OS: the Linux archive was built and installed here; the Windows and macOS layouts are unit-tested, and their first real build is the release workflow
- [ ] Windows zip: written and unit-tested; not built on Windows yet
- [ ] macOS `Flight Replay.app`: written and unit-tested (Info.plist, icon, permissions); not built or opened on macOS yet
- [x] Source archive with each release
- [x] Review `NOTICE` and `THIRD_PARTY_NOTICES.md`
- [x] Decide on bundling FFmpeg (D6): not bundled in v0.1.0
- [ ] Release workflow `release.yml`: written; it has not run, since the repository has no remote yet
- [ ] Tag `v0.1.0`
- [ ] Install and run on three OSes

## 12. Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| A GPL notice or a changed-file mark is missing in a ported file | License non-compliance | Origin header rule (section 4.2). Review checklist at release. The `NOTICE` file is checked in P9 |
| The walksnail backend uses old dependencies (`image` 0.24, `rusttype`, a git fork of `ffprobe`) | Build problems, unmaintained code | Upgrade in P1 and P2. Check the visual output does not change |
| No Betaflight recordings yet | P4b cannot start | Sound is INAV-only until the recordings arrive. Overlay and SRT work for both |
| Telemetry rules have been verified on few recordings (the fpv-osd-decoder limitations list says so) | Wrong values for elements that are not in the samples | Report the coverage per signal. Warn when a signal is missing. Add the recordings as they arrive |
| `.osd` files contain GPS coordinates | Flight locations become public in fixtures or in bug reports | Fixture rule in section 10.4. Warn users before they share a file |
| iced is pre-1.0 and its API changes between releases | Rework | Pin the minor version. The GUI lives in its own crate |
| Unmaintained crates reached through iced 0.14 (six advisories today) | Policy failures on dependency updates | Listed as exceptions with reasons in `deny.toml`. Reviewed on each iced update |
| Full-frame RGBA pipe too slow at 1440p or 4K | Long renders | Measure in P6. If needed, composite in YUV 4:2:0, or redraw only changed regions |
| Temporary WAV is large (about 370 MB per 32 minutes) | Disk space | Check free space before rendering. Write next to the output. Delete on finish or cancel |
| An encoder is listed but fails | Render error | Verify with a test encode during detection. Show FFmpeg's error text |
| FFmpeg not found on macOS or Windows | Failure on first run | Discovery list, a path in settings, and a clear message in step 1 |
| No audio device, or no ALSA | No audio preview | Disable the preview with a message. Rendering is not affected |
| Overlay and sound drift apart | Wrong timing | One `timeline` module used by both. Probe check across both |
| The fixture picture shows the place (the raw recording is an aerial view of a city) | The location leaks even though the `.osd` is anonymized | The owner decides per clip (section 14). A clip with no landmarks, or a cropped clip, can be used |
| The shape of the track identifies the place, even after a translation | The location can be guessed from the path | Accept it, or add a rotation around the new point (distances are kept) |
| The real coordinates end up in the repository | The real location is public | The offset is never stored. The test `the_positions_in_the_clip_are_the_fake_ones_in_the_south_atlantic` fails if a position outside the fake area is in the fixture |

## 13. Prerequisites

- Rust 1.88 or newer (iced 0.14 requires 1.88; rodio 0.23 requires 1.87). The toolchain is pinned to 1.99.1 in
  `rust-toolchain.toml`, and `rustup` installs it on first use.
- `ffmpeg` and `ffprobe` 6.0 or newer on `PATH`. Tested locally with 8.0.1.
- GNU Make.
- Local recordings, outside the repository: the raw goggle recordings (the owner's SD-card folder) for fixtures,
  G0050 and G0051 (fpv-motorsound), and AvatarG0031 (fpv-osd-decoder,
  with its CSV as the reference for the INAV rules).
- **Betaflight recordings (for P4b):** at least one `.osd`, its video, and the font, from a flight with a known
  Betaflight version. Ideally with throttle, current, speed and voltage shown on the OSD.
- Go is not needed. The original Go projects are used as references and oracles only.

## 14. Items still open

These do not block P0. The defaults are used unless the owner changes them.

1. **Betaflight recordings** (section 13). They are needed for P4b.
2. **Copyright line** for `NOTICE` and new files: `Copyright (C) EricoGR` (**default**).
3. **macOS bundle identifier:** `io.github.ericogr.flight-replay` (**default**).
4. **Defaults taken** for D3 (TOML), D6 (external FFmpeg), D9 (MP4), D10 (`replace` option), D11 (no update check)
   and D12 (`rodio` preview), unless the owner changes them.
5. **Fixture clip choice.** Decided: the owner accepts the aerial view of a city in the picture of the clip.

## 15. Revision history

- Revision 1, 2026-11-05: first plan. Questions to the owner.
- Revision 2, 2026-11-05: owner's answers applied. GPL-3.0-only accepted. Reuse of the walksnail backend and of
  the Go projects allowed. INAV and Betaflight telemetry. No CLI in v1. Name Flight Replay. Fixtures committed.
- Revision 3, 2026-11-05: GPS anonymization of the committed `.osd` (constant offset to open ocean, digit glyphs
  rewritten, offset kept local). Video-picture privacy added as an owner decision. Task checklist added (section 11.1).
- Revision 4, 2026-11-05: phase P0 done on Linux. Toolchain 1.99.1, MSRV 1.88. `deny.toml` with the license
  allow list and documented advisory exceptions. Window opens on Linux. Windows and macOS are checked by CI.
- Revision 5, 2026-11-06: phase P7 built on Linux. Seven-step iced wizard, settings saved on every step change and on
  close, frame preview (640 px wide, so the options of the step fit below it), light, dark and system themes. The
  wizard is tested without a desktop: iced's headless simulator drives the real `update` and `view`, runs the tasks,
  and the full walk-through renders the fixture clip (`crates/gui/src/tests.rs`; `FLIGHT_REPLAY_SNAPSHOTS=<folder>`
  saves a picture of every step). Deviation from the plan: opening a new video starts over, with a notice, instead of
  asking a question in a dialog. Typing into the real window could not be tried: the screen saver was covering the
  window, and the session was left alone. Open: listening to the sound preview, a real drag and drop, and the manual
  walk-through on the three systems.
- Revision 6, 2026-11-06: phase P8 built and tested on Linux. `osd::join` puts the `.osd` files of one video on one
  timeline (each file starts where the one before it ends); the overlay, the sound and the telemetry loader share
  the same shifts, so they cannot disagree. `osd::Coverage` warns when the files do not add up to the video (a file
  may be missing, or belongs to another video). The plan is now built in one place, `render::build_plan`, for the
  wizard and the queue. The queue renders the videos one after the other with the settings of the wizard: add the
  current video, or every video of a folder (those whose output exists or that lack a needed file are left out, with
  the reason). The speed of the OSD is chosen for each video. Tests: `crates/core/tests/join.rs` (the fixture clip
  played twice, rendered with OSD and sound) and the queue and joined-file tests in `crates/gui/src/tests.rs`.
- Revision 7, 2026-11-06: phase P9 written on Linux. `cargo xtask dist` packs the release for the system it runs on
  (`.tar.gz` for Linux, `.zip` for Windows and macOS, `Flight Replay.app` with a generated `.icns`), writes
  `SHA256SUMS`, and `--source` adds the source archive. A Linux archive was built, checked and installed in a
  temporary prefix. `THIRD_PARTY_NOTICES.md` comes from `cargo about`. The application has an icon (window, Windows
  resource, Linux icons, macOS bundle). FFmpeg is not bundled (D6). The Windows and macOS packages, the release
  workflow and the tag `v0.1.0` wait for a remote and a CI run: they cannot be checked from this machine, and a tag
  pushed would publish a release.
- Revision 8, 2026-11-06: the real window was walked through on Linux (XFCE, X11, build `release`) with synthetic
  mouse and keyboard input. Checked: all seven steps, the preview with the real INAV font, the time slider, mask
  clicking on the OSD grid, light and dark themes, a render (libx264 and AAC, 180 frames, OSD burned in), the sound
  preview, a drag and drop from Thunar, and saving the settings when the window is closed. One bug found and fixed: a
  video given on the command line as a relative path left relative paths in the settings and in the output name.
  Still unchecked: listening to the sound, and everything on Windows and macOS (the manual walk-through item of P7).
- Revision 9, 2026-11-06: review of the whole code for coupling, duplication and clean code.
  - Sound preview (step 5): the picture and the OSD now follow the sound while it plays (they stayed on one frame
    before), the sound is limited to what is left of the video, the slider stops at the last frame (a picture at the
    very end of the video failed), and the preview goes back to where it started when the sound ends.
  - Core: removed code that nothing used (`Recording::load_joined`, `Segment`, `filled_pct`, `first_crossing`,
    `extra_fields`) and the second way to join `.osd` files; one `Timeline::osd_frame`, one `Sound` struct and one
    sound loop in the render (`sound_only` and `write_wav` were copies of each other, one with eight arguments), one
    helper for the spawn error, the path helpers (`paths`) and the time text (`display`); the planning of a folder
    moved from the GUI to the core (`render::plan_folder`) with typed reasons; discovery no longer lists the videos
    Flight Replay wrote (a second run over a folder tried to render its own output).
  - Settings: the colour scheme and the last folder are the window's, not the core's. `config` reads and writes any
    type, and the GUI keeps `Preferences` in the same `settings.toml` (the old files are read as before).
  - GUI: `app.rs` (1,400 lines, two levels of `match` with wildcards that could hide an unhandled message) became the
    `app/` folder with one flat `update` the compiler checks; the typed timing and stretch are read from the text
    when needed instead of being kept twice; the geometry of the OSD grid under the pointer left the app and has
    tests; the duplicated `audio_playing` flag, `LoadedOsd::path` and several copies of the same helpers are gone.
  - The GUI tests use the software renderer (`.cargo/config.toml`): with the GPU renderer one run in five crashed.
  - New `docs/architecture.md`.
- Revision 10, 2026-11-06: motor presets and the FFmpeg command made optional.
  - Seven built-in motor presets (`synth::MOTOR_PRESETS`): the default and six kinds of electric motor and propeller.
    A preset sets the character of the sound (pitch range, inertia, tone, propeller noise) and leaves the format, the
    columns, the load calibration, the wind and the battery as the user has them. The sound step reads which preset is
    in use from the settings, so it is right after a restart and says "Custom" after a change by hand. The saved
    presets (the user's own, whole `SoundSettings`) are unchanged. The levels were balanced by measuring the RMS of a
    throttle sweep (all within 0.4 dB of the default), and `crates/core/tests/presets.rs` keeps them within 1.5 dB,
    below the limiter, and different from each other by 3 dB or more in an octave band. Two sliders were added
    because presets vary what had none: the whine pitch and the sharpness of the propeller noise.
  - The FFmpeg command of step 7 is shown only when `Preferences::show_command` is on. It is off by default, kept in
    `settings.toml` with the other preferences, and a file from an earlier version reads as off.
  - Not done: the harmonics of a preset have no slider; a multirotor (four motors at once) is not modelled; and the
    presets were not listened to, only measured.
- Revision 11, 2026-11-06: the preview in motion.
  - The sound preview was slow because it asked for a new picture four times a second, and each one started FFmpeg
    and decoded from the nearest key frame (82 ms at 720p, 203 ms at 1080p, 549 ms at 4K, so 12, 5 and under 2
    pictures a second at best). It is now one FFmpeg process, decoded already shrunk to the size of the preview
    (`DecodeOptions::scale_to`) at up to 30 frames a second, with a thread that draws the layers and hands each frame
    over when the clock gets to it. Measured on synthetic 60 fps clips, release build: 28 to 30 frames a second at
    every size, 0.7 ms late on average against the clock of the sound, the first picture after 70 ms (720p), 141 ms
    (1080p) and 485 ms (4K). Against the still picture at the same time, the mean difference is 1.7 of 255.
  - The OSD is drawn on the small frame with glyphs and offsets made smaller by the same factor
    (`ScaledFont::resized`), with the distance between cells kept as a fraction so that it does not drift from where
    it is on the video.
  - The picture says that its quality is reduced: under every preview (when it is smaller than the video) and in the
    notes of the steps that play. The render is not affected.
  - Steps 3 (OSD) and 4 (SRT data) have Play and Stop too, without sound, for checking the layer in motion. Moving
    to another step stops a play.
  - New option "Also show the ... in the preview" (on by default, remembered): off, each step shows only its own
    layer and the step of the sound none.
  - Review of this work: the still picture and the one that plays share one `Scene` and one `Layers` (they had two
    copies of the drawing); the whole state of a play is one `Playing`; one timer helper; the rule of which layers a
    step wants is in `wizard.rs` with tests; the ticks and restarts carry the number of their play. Problems found
    and fixed: a decoder slower than the clock could freeze the picture (every frame was late and skipped), the grid
    of the masks stayed on the previews of other steps, a sound that was ready after the user left its step played
    there, and two chains of ticks could run after a stop followed by a play.
  - Not done: a decoder much slower than the clock (4K H.265 without hardware decoding on a weak machine) shows the
    picture late and in jumps of 200 ms instead of starting the stream again nearer to the clock; the OSD and SRT
    steps have no sound; nobody has watched the play in the real window, only in the simulator and through the
    messages of the application, and the timings above are of synthetic clips, not of Walksnail recordings.
- Revision 12, 2026-11-06: every choice on the screens is remembered.
  - Audit of the controls against `settings.toml`: the speed and the delay of the OSD, "Replace the file if it exists",
    "Replace it with the new sound", the start and the length of the stretch, and the saved sound that was chosen were
    not saved. They are now in `Preferences` (typed text is kept as typed), as the only copy: the fields of the
    application struct that held them are gone.
  - Settings were written only when the user changed step or closed the window. `app/persist.rs` compares `Settings`
    and `Preferences` with what the file holds after every message and writes the difference (the first change at
    once, a burst 300 ms later in one write), so a program that is killed loses at most that gap.
  - Left out on purpose: the video and its files, the name of the output file, whether a stretch is asked for (a quick
    test is never left on), the mode for hiding elements by clicking (a tool, not a choice), and the name typed before
    Save. Each is one field to add if that is wanted. They are kept out of `Settings` and `Preferences`, so changing
    one never causes a write.
  - Review of this work: the switch of the stretch had been put in `Preferences` with `serde(skip)`, which made each
    toggle a useless write, and it moved to the application (`stretch_on`); the rule that a chosen saved sound must
    exist in the file moved from the constructor of the application to `preferences::load`, where it is tested without
    FFmpeg; a settings file that could not be written was only logged and is now said to the user once; closing the
    window wrote the file even when nothing had changed, and now shares `flush_settings` with the timer.
