# Contributing to Flight Replay

## Language

All code, comments, documentation and commit messages are written in English.

## Commits

- Use [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`, `refactor:`,
  `test:`, `chore:`.
- Run `make check` before each commit. It runs formatting, lints, tests and the dependency policy.
- Do not add AI co-author trailers to commits or pull requests.

## Source file headers

Every source file starts with the SPDX identifier:

```rust
// SPDX-License-Identifier: GPL-3.0-only
```

A file ported or adapted from another project also names its origin, its license and the change:

```rust
// SPDX-License-Identifier: GPL-3.0-only
// Adapted from walksnail-osd-tool, backend/src/osd/osd_file.rs (GPL-3.0-only),
// Copyright (C) Alexander van Saase.
// Modified for Flight Replay: <short description of the change>.
```

Add the new work to `NOTICE` when a file is ported. Dependencies are added to `deny.toml` only if their license
is not already allowed.

## Where code goes

Read [docs/architecture.md](docs/architecture.md) first. The short version: logic that does not need a window goes in
`crates/core`, so a command line can use it later; the window only asks for it. A new message of the window needs an arm
in `update` (`crates/gui/src/app/mod.rs`) and a handler in the module of `app/` it belongs to. Before you write a helper,
look for one: paths are in `paths`, times in `display`.

Write a test when there is logic to check: a parser, a rule, a mapping, a way of reading what the user typed. Do not write
one for code that a tool generated, for the layout of a view, or for a function that only calls another.

## Dependencies

Dependencies must be compatible with GPL-3.0-only. `make deny` checks the licenses, advisories and sources. Take the
smallest set of features that does the job: a crate that is only read from does not need the feature that writes.

## Before you call a change done

Every change, small or large, ends with these steps. They are part of the work, not a favour to the reviewer.

1. **Review what you wrote, and what it touches.**
   - *Dependencies between modules point one way.* The core never depends on the GUI. Inside the core, a low-level
     module (`ffmpeg`, `paths`) does not import a high-level one (`render`). A type that two modules share goes in a
     small module of its own (`cancel`).
   - *No duplicated behaviour.* Look for a helper before writing one (`paths`, `display`, `tasks::stream_from_thread`),
     and look again when you are done for a second copy of the same idea: a margin, a way of writing a size, a loop that
     only differs in what it reads.
   - *Clean code.* Small functions. A named constant for a number that means something. A type, not a string built by
     hand (and no `""` fallback that hides a value that is missing). No `Option` for what is always there. No dead code.
     `#[allow(...)]` always with a `reason`.
   - *Fix what is wrong next to your change* when the change makes it reachable, even if you did not write it. Say so
     in the changelog.
2. **Test what has logic, and only that.** The rule is in "Where code goes". In addition: a bug fix has a test that fails
   without the fix (undo the fix once and watch it fail); a test never uses the network or depends on the speed of the
   machine; a check that needs the real network, programs or hardware is `#[ignore]`d and run by hand, with the
   command in a comment above it; a test that only repeats the code it tests (for instance a `cfg!` written twice) is
   deleted.
3. **Run the checks.**
   - `make check` (formatting, clippy with `-D warnings`, tests, dependency policy).
   - `make cross-lint` when you touched code or tests gated by `cfg(windows)`, `cfg(unix)` or `cfg(target_os = ...)`:
     `make check` only lints the system you are on, and the CI of another one fails on what it does not see.
   - For a change to a screen, look at the pictures: `FLIGHT_REPLAY_SNAPSHOTS=<folder> cargo test -p flight-replay`
     writes one for each step the tests go through.
   - For a change that touches the real world (the network, another program, the disk), run the real thing at least
     once, not only the tests with stand-ins.
4. **Update the documentation that the change makes wrong or incomplete:** `README.md` (what a user sees),
   `docs/build.md`, `docs/architecture.md` (modules and files), `CHANGELOG.md` under `[Unreleased]`, `PLAN.md`
   (decisions, risks, the task checklist, and a revision entry), and `make notices` when dependencies changed. Tick a
   box of the checklist only for what was verified.
5. **Say what was verified and what was not.** A check that could not be done on this machine (a system that is not
   yours, hardware you do not have) is written down as not done, not left out.
