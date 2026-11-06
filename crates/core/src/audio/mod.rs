// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Audio files and sample formats.

mod wav;

pub use wav::{pcm16le, write_wav_header};
