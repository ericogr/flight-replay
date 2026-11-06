// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Data files that are compiled into the library. Their licenses are in `assets/`.

/// Azeret Mono, the font of the SRT text (SIL Open Font License 1.1, see `assets/fonts/AzeretMono-LICENSE.txt`).
pub const SRT_FONT: &[u8] = include_bytes!("../../../assets/fonts/AzeretMono-Regular.ttf");
