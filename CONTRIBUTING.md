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

## Dependencies

Dependencies must be compatible with GPL-3.0-only. `make deny` checks the licenses, advisories and sources.
