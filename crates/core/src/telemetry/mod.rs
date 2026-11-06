// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Flight telemetry, read from the OSD screens.
//!
//! The decoder recognizes each field of the INAV OSD by its icon and unit ([`decoder`]). The readings are laid out
//! as gap-free, interpolable time series ([`Table`], [`Series`]). The glyph codes of the font are in [`inav`].

pub mod catalog;
pub mod decoder;
pub mod inav;
mod probe;
mod profile;
mod recording;
mod series;
mod table;

pub use probe::{
    CARRY_MAX_AGE_S, DEFAULT_PROBE_COLUMNS, ProbePosition, ProbeReport, ProbeValue, probe_at,
};
pub use profile::{Profile, TelemetrySupport};
pub use recording::{LoadOptions, Recording, RecordingError, Segment};
pub use series::{Cursor, Series, SeriesError, Stats, at_or, fill_forward};
pub use table::{BuildInfo, Row, Table, TableError, format_value};
