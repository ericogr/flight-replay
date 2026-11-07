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
| `ffmpeg` | Finds `ffmpeg` and `ffprobe`, reads what `ffprobe` says about a video, lists the encoders and tries each with a one-frame encode, decodes raw RGBA frames over a pipe (as stored, or already shrunk, which is how the previews that play are decoded), and runs the encoder process. FFmpeg is only ever a separate program. |
| `osd` | The `.osd` file (header, screens, firmware), the options for drawing it (position, masks), and `join`: several files of one video put on one timeline, with the coverage check. |
| `font` | OSD font sheets: the layouts they come in, and the glyphs at the size of the video, or made smaller for a picture that is smaller than the video (the previews), with the cells kept exactly in proportion. An `OsdFont` is one sheet (a file) or several (a font that comes with the program, compiled in from `assets/osd-fonts/`), and the sheet nearest to the video is the one drawn. |
| `srt` | The SRT file the goggles record, and the data in each line. |
| `overlay` | Draws the OSD and the SRT data on a frame, and the chroma-key background. |
| `telemetry` | Reads the numbers out of the OSD screens. `decoder` and `inav` hold the rules and the glyph table; `table` and `series` put the readings on a timeline; `recording` ties them together; `probe` shows what the OSD says at a time; `profile` says what a firmware can give the sound. |
| `timeline` | The one mapping between video time and telemetry time (offset and speed), used by the overlay and by the sound so they agree. |
| `synth` | The sound: settings (with the kind of aircraft), the presets of each kind, signals taken from the telemetry, and the engine (oscillators, filters, noise, limiter). `rotors` gives each motor of a drone a tone of its own, at its own speed. It is deterministic: the same input gives the same bytes, whatever the block size. |
| `audio` | PCM and WAV writing. |
| `render` | `RenderPlan` (everything a render needs), `build_plan` (makes one from the files and the settings), `Availability` (what the files at hand allow, and why not), `plan_folder` (makes one for each video of a folder), `render` (does it, with progress and cancelling), and the check of the output. |
| `config` | The settings of a render, and how to read and write settings files. |
| `discover` | Finds the `.osd` and `.srt` next to a video, and the videos of a folder. Knows the names of the files Flight Replay writes. |
| `paths`, `display` | Small helpers for paths and for writing times in messages, so every place does it the same way. |

Errors in the core are types (`thiserror`), so a caller can tell them apart. Text for the user is written where an
error is shown.

### What the files allow

The OSD, the SRT data and the sound each need files: the OSD an `.osd` and a font, the SRT data an `.srt`, the sound an
`.osd` of a firmware whose telemetry is read, with the throttle in it. `Availability` says, for each, why it cannot be
made, if it cannot. What the user asked for and the files cannot give is left out of the render, with the reason; it
is never an error, and the user's choice is not changed (it comes back with the file). The application and
`plan_flight` both apply it before `build_plan`, which stays strict: it still lists everything wrong with what it is
given. A flight is refused only when nothing is left to add.

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
├── preferences.rs  what the window remembers (colour scheme, last folder, the options of the previews, the timing of
│                   the OSD, the output options that are not in the core, the sound that was chosen), kept in the same
│                   file as the settings
├── wizard.rs       the seven steps, the rules for moving between them, and which layers the preview of each step
│                   has, with no iced in it
├── tasks.rs        work that must not block the window (opening files, probing, rendering): runs on a thread and
│                   answers with a message
├── preview.rs      what a preview picture is made from (`Scene`) and the layers drawn on it (`Layers`), which the
│                   still picture and the one that plays share; makes the still picture; the geometry of the OSD
│                   grid under the pointer
├── playback.rs     the picture of a preview that plays: one FFmpeg process decodes the stretch already shrunk, and a
│                   thread draws on each frame and sends it when the clock gets to it
├── audio.rs        plays the sound of the sound preview
├── sound_params.rs the table of sound parameters the sound step draws its sliders from (with a range for a drone where it differs)
├── app/            the state of the application and how it reacts
│   ├── mod.rs        the struct, and `update`: one flat table from message to handler
│   ├── files.rs      the video and the files that go with it
│   ├── options.rs    what to draw, the timing, the sound and the output
│   ├── persist.rs    writing the settings file whenever a choice changes
│   ├── picture.rs    the preview picture
│   ├── play.rs       a preview that plays (the sound step with its sound, the OSD and SRT steps without), and the
│   │                 picture of it, a stream paced by the clock of the sound or of the computer
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
  without a window. The facts about what is added are the effective ones (asked for, and allowed by the files), so the
  steps that choose what to add never hold the user back.
- The settings of a render live in the core (`Settings`). The window keeps its own things in `Preferences` and
  writes both to one `settings.toml`: the preferences at the top, then the settings. The core can still read that
  file by itself.
- Every choice the user makes on a screen lives in `Settings` or in `Preferences`, once, and nowhere else (the screens
  read it from there), so there is nothing to keep in step and nothing to forget to save. `app/persist.rs` compares
  both with what the file holds after every message and writes the difference: at once for the first change, and a
  burst of changes (a slider) in one write 300 ms later. Closing the window writes at once. If the file cannot be
  written the user is told once, in a notice. A new choice is remembered by adding it to one of the two structs; what
  is not remembered on purpose is listed in the README. What is read is made to agree with itself in
  `preferences::load` (a chosen saved sound that is no longer in the file is dropped), and what belongs to one run
  only (`stretch_on`, the mode for hiding elements, the name typed for a sound) is kept out of the two structs, so
  that changing it never causes a write.
- The still picture and the picture that plays are made from the same `Scene` and drawn with the same `Layers`, so
  what one shows the other shows. Which layers a step wants is a rule of `wizard.rs` (`Step::preview_layers`), and
  the application only applies it to what the files allow.
- Everything about a play is in one `Playing` (the clock, the sound if any, the stream of the picture), so there is
  no picture without a play, and a stopped play cannot leave a stream behind.

### The preview in motion

A play shows ten seconds of the video from the time on the slider, with the sound in the step of the sound and
without it in the steps of the OSD and of the SRT data.

- **One FFmpeg process, decoded small.** `playback::start` opens a single decoder at the start time, with
  `DecodeOptions::scale_to` and a frame rate that is the rate of the video or a whole fraction of it up to 30
  (`play_rate`). The OSD glyphs and offsets are shrunk by the same factor (`ScaledFont::resized`), so the OSD lands
  where it does on the video. Starting FFmpeg for every picture instead took 200 ms at 1080p and 550 ms at 4K.
- **A clock paces it.** A thread reads a frame, draws the layers on it, waits until the clock has got to its time and
  sends it. The clock is the moment the sound started, or the moment the play started when there is no sound. A
  frame more than one frame late is skipped so that the picture catches up, but one is always shown at least every
  200 ms (`should_skip`): a decoder slower than the clock would otherwise freeze the picture.
- **The window never waits.** Frames go through a short channel, and a frame that finds it full is dropped. The
  window ignores a frame from a stream that was stopped (each stream has a number).
- **Edits restart the stream, once.** A change made while it plays (a position, a field, the timing, the option of the
  layers) asks for the picture to start again from where the clock is. The request waits 150 ms and is dropped if
  another edit came meanwhile, because dragging a slider makes dozens of edits a second and each start runs FFmpeg.
  The ticks that look for the end of the play and these restarts carry the number of their play, so a stopped play
  leaves nothing behind.
- **It belongs to its step.** Moving to another step stops it, and a sound that is ready after the user left the step
  of the sound is not played.
- **The user is told.** The picture is a preview at reduced quality (smaller, at most 30 frames a second, frames
  skipped if the computer is slow). Each step says so next to the picture, and the render is not affected.

### Adding a feature to the window

1. Add the message to `message.rs`, and the state it needs to `state.rs` or the `FlightReplay` struct.
2. Add its arm to `update` in `app/mod.rs` and write the handler in the module of `app/` it belongs to. The compiler
   points at every `match` that is now missing a case.
3. Draw it in the `view` of its step.
4. If it is logic that does not need a window, write it in the core instead and call it from the handler.

## Tests

- **Unit tests** are next to the code. They are for logic: parsers, the timeline, the join, the decoder rules, the
  arguments given to FFmpeg, the sound engine, the wizard rules (including the layers each preview has), the geometry
  of the preview, how a preview follows its clock (the rate, which frames are skipped), how typed text is read.
- **Integration tests** (`crates/core/tests`) use the real FFmpeg and the fixture clip in `tests/fixtures`. They
  render, cancel, join, and plan folders. They print a message and pass when FFmpeg is not installed.
- **Golden images** (`tests/golden`) pin the output of the overlay. `make golden` rewrites them; look at the pictures
  before committing.
- **Reference checks** compare the telemetry and the sound with the Go programs they were ported from, on real
  recordings that are not in the repository. They run when the variables in [build.md](build.md#tests) are set.
- **The wizard tests** (`crates/gui/src/tests.rs`) run the real `update` and `view` with iced's simulator and the
  software renderer, so they need no desktop and no GPU. They click through every step, render the fixture clip, join
  `.osd` files, run a queue, and play a preview to its end or stop it by leaving the step. They are slow on purpose: they are the check that the pieces work together.

Not tested on purpose: the layout of each view (it is read in the pictures the wizard tests can save), code that a
tool writes (the notices, the Windows resource), the sound device, and the timing of a play against a real clock (the
tests check that it ends, stops and restores the picture, not how many frames it gave in a second).
