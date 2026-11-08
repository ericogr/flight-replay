# Instructions for coding agents

Flight Replay is a Rust workspace (an iced application on top of a core library) that burns a flight controller OSD,
SRT data and a synthesized motor sound onto a Walksnail Avatar recording, using FFmpeg as a separate program.

Read first:

- [CONTRIBUTING.md](CONTRIBUTING.md): the rules for this repository (language, commits, file headers, where code goes,
  when to write a test, dependencies). They apply to you as much as to a person.
- [docs/architecture.md](docs/architecture.md): the modules, the files, and why they are shaped as they are.
- [PLAN.md](PLAN.md): the decisions (`D1`, `D2`, ...), the phases, the risks and the task checklist.
- [docs/build.md](docs/build.md): building, the `make` targets, how FFmpeg is found and installed.

## How to finish a task

When an implementation is done, **do the review in "Before you call a change done" in `CONTRIBUTING.md` without being
asked**, fix what it finds (also in code you did not write, when your change makes it reachable), and only then report.
Doing it is the normal end of a task. It is not an extra for the user to request.

## Commands

- `make help` lists the targets. `make check` is the gate: formatting, clippy with `-D warnings`, tests, `cargo deny`.
- `make cross-lint` lints the Windows and macOS targets from any system. Run it after touching `cfg`-gated code.
- The tests that need FFmpeg print a message and pass when it is not installed. Look at the output to see whether they
  ran.
- The GUI is tested without a window (`crates/gui/src/tests.rs`, iced's simulator). `FLIGHT_REPLAY_SNAPSHOTS=<folder>`
  saves a picture of each step.

## Rules that are easy to miss

- Everything written in the repository (code, comments, docs, commits) is in English, whatever language the user speaks.
- Commits follow Conventional Commits and carry **no AI co-author trailer**. **Do not commit or push unless asked.**
- A new dependency must have a license that `deny.toml` allows, and the smallest feature set that works.
- `#[allow(...)]` needs a `reason`. The core has `#![forbid(unsafe_code)]`, prints nothing, and its errors are `thiserror`
  types.
- The program does nothing on its own over the network. The only use is the install of FFmpeg that the user asks for
  (`D13` in `PLAN.md`).
- Do not tick a box in the `PLAN.md` checklist for something that was not verified. A thing that could not be run here
  (Windows, macOS, hardware encoders) stays open, with the reason.
- In the GUI tests, `Harness::snapshot` deletes the files whose name starts with `<name>-`: give each snapshot a name
  that is not the start of another. A click in the harness runs its task to the end, threads included, so a test must
  not click a button that starts a real download or render.
