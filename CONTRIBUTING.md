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

Dependencies must be compatible with GPL-3.0-only. `make deny` checks the licenses, advisories and sources.
