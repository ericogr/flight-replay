# Architecture

Flight Replay is a Cargo workspace of three crates.

| Crate | What it is |
|---|---|
| `crates/core` (`flight-replay-core`) | A library with no window and no GUI type in it. Everything that reads, decodes, synthesises, draws and encodes is here, so a command-line tool can be added later by calling it. |
| `crates/gui` (`flight-replay`) | The application: an [iced](https://iced.rs) wizard on top of the core. It has the binary. |
| `xtask` | Developer automation (`cargo xtask`): cutting the test clip, packaging. It is not a command line for users. |

The rule that keeps this true: the core never depends on the GUI, and the GUI never reimplements what the core does.
When the GUI needs logic that is not about a window (planning a folder, joining `.osd` files, finding the frame to
draw), it goes in the core, where a command line can use it too.

## The core

| Module | What it does |
|---|---|
| `ffmpeg` | Finds `ffmpeg` and `ffprobe`, reads what `ffprobe` says about a video, lists the encoders and tries each with a one-frame encode, decodes raw RGBA frames over a pipe, and runs the encoder process. FFmpeg is only ever a separate program. |
| `osd` | The `.osd` file (header, screens, firmware), the options for drawing it (position, masks), and `join`: several files of one video put on one timeline, with the coverage check. |
| `font` | OSD font sheets: the layouts they come in, and the glyphs at the size of the video. |
| `srt` | The SRT file the goggles record, and the data in each line. |
| `overlay` | Draws the OSD and the SRT data on a frame, and the chroma-key background. |
| `telemetry` | Reads the numbers out of the OSD screens. `decoder` and `inav` hold the rules and the glyph table; `table` and `series` put the readings on a timeline; `recording` ties them together; `probe` shows what the OSD says at a time; `profile` says what a firmware can give the sound. |
| `timeline` | The one mapping between video time and telemetry time (offset and speed), used by the overlay and by the sound so they agree. |
| `synth` | The sound: settings, signals taken from the telemetry, and the engine (oscillators, filters, noise, limiter). It is deterministic: the same input gives the same bytes, whatever the block size. |
| `audio` | PCM and WAV writing. |
| `render` | `RenderPlan` (everything a render needs), `build_plan` (makes one from the files and the settings), `plan_folder` (makes one for each video of a folder), `render` (does it, with progress and cancelling), and the check of the output. |
| `config` | The settings of a render, and how to read and write settings files. |
| `discover` | Finds the `.osd` and `.srt` next to a video, and the videos of a folder. Knows the names of the files Flight Replay writes. |
| `paths`, `display` | Small helpers for paths and for writing times in messages, so every place does it the same way. |

Errors in the core are types (`thiserror`), so a caller can tell them apart. Text for the user is written where an
error is shown.

### A render

1. `build_plan` turns the files and the settings into a `RenderPlan`, or lists everything that is wrong.
2. `render` checks the plan, then `Timeline` maps video time to telemetry time, and a warning is given if the
   telemetry does not span the video.
3. With sound, `telemetry::Recording` decodes the `.osd`, `synth::SignalSet` takes the signals the sound follows, and
   the engine makes the sound a block at a time.
4. With the OSD or the SRT data, the video is decoded to raw frames, each is drawn on, and the frames go to the
   encoder; the sound is made first into a temporary WAV file. With only sound, the video stream is copied and the
   sound goes to FFmpeg as it is made.
5. The output is written under a `.partial` name and renamed when it is complete, so a cancelled render leaves
   nothing behind. Then it is opened to compare the length of the picture and of the sound.

The numbers behind the timeline are in `timeline.rs`; the formats are in [formats](formats); the sound is in
[audio-model.md](audio-model.md).

## The application

The GUI follows the Elm shape that iced is made for: a state, a `Message` for everything that can happen, one
`update` that changes the state, and `view` functions that draw it.

```text
crates/gui/src
├── main.rs         starts iced
├── message.rs      every message, grouped by what it is about
├── state.rs        what is in memory: loaded files, the preview, the render, the queue, and the text the user typed
├── preferences.rs  what the window remembers (colour scheme, last folder), kept in the same file as the settings
├── wizard.rs       the seven steps and the rules for moving between them, with no iced in it
├── tasks.rs        work that must not block the window (opening files, probing, rendering): runs on a thread and
│                   answers with a message
├── preview.rs      makes the preview picture, and the geometry of the OSD grid under the pointer
├── audio.rs        plays the sound preview
├── sound_params.rs the table of sound parameters the sound step draws its sliders from
├── app/            the state of the application and how it reacts
│   ├── mod.rs        the struct, and `update`: one flat table from message to handler
│   ├── files.rs      the video and the files that go with it
│   ├── options.rs    what to draw, the timing, the sound and the output
│   ├── picture.rs    the preview picture
│   ├── listen.rs     the sound preview; the picture follows the sound
│   ├── rendering.rs  the plan and running a render
│   └── queue.rs      several videos, one after the other
└── view/           one file for each step, and the pieces they share
```

Things worth knowing:

- `update` is one `match` with an arm for every message and no wildcard, so adding a message that nothing handles
  does not compile.
- A handler does not do slow work. It starts a task (`tasks.rs`) and returns; the task answers with a message
  (`VideoLoaded`, `PreviewReady`, `Render(..)`), and another handler takes the result.
- What the user types is kept as text (`SyncInput`, `StretchInput`) and read when it is needed, so there is one copy
  of each choice and nothing to keep in step.
- The wizard rules (`wizard.rs`) take a plain `Context` of facts, so the rules about what may come next are tested
  without a window.
- The settings of a render live in the core (`Settings`). The window keeps its own things in `Preferences` and
  writes both to one `settings.toml`: the preferences at the top, then the settings. The core can still read that
  file by itself.

### Adding a feature to the window

1. Add the message to `message.rs`, and the state it needs to `state.rs` or the `FlightReplay` struct.
2. Add its arm to `update` in `app/mod.rs` and write the handler in the module of `app/` it belongs to. The compiler
   points at every `match` that is now missing a case.
3. Draw it in the `view` of its step.
4. If it is logic that does not need a window, write it in the core instead and call it from the handler.

## Tests

- **Unit tests** are next to the code. They are for logic: parsers, the timeline, the join, the decoder rules, the
  sound engine, the wizard rules, the geometry of the preview, how typed text is read.
- **Integration tests** (`crates/core/tests`) use the real FFmpeg and the fixture clip in `tests/fixtures`. They
  render, cancel, join, and plan folders. They print a message and pass when FFmpeg is not installed.
- **Golden images** (`tests/golden`) pin the output of the overlay. `make golden` rewrites them; look at the pictures
  before committing.
- **Reference checks** compare the telemetry and the sound with the Go programs they were ported from, on real
  recordings that are not in the repository. They run when the variables in [build.md](build.md#tests) are set.
- **The wizard tests** (`crates/gui/src/tests.rs`) run the real `update` and `view` with iced's simulator and the
  software renderer, so they need no desktop and no GPU. They click through every step, render the fixture clip, join
  `.osd` files and run a queue. They are slow on purpose: they are the check that the pieces work together.

Not tested on purpose: the layout of each view (it is read in the pictures the wizard tests can save), code that a
tool writes (the notices, the Windows resource), and the sound device.
