// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The synthetic motor and propeller sound, driven by telemetry.

mod dsp;
mod engine;
mod presets;
mod rotors;
mod settings;
mod signals;

pub use engine::{Controls, Engine, EngineError, RenderStats, frames_for};
pub use presets::{MOTOR_PRESETS, MotorPreset, motor_preset_in_use};
pub use settings::{
    AircraftKind, AudioSettings, ColumnSettings, GForceSettings, LoudnessSettings, MAX_ROTORS,
    MotorSettings, PropNoiseSettings, RotorSettings, SmoothingSettings, SoundSettings, Stopped,
    ToneSettings, WindSettings,
};
pub use signals::{MIN_COVERAGE_PCT, MOTOR_HOLD_S, Role, Signal, SignalError, SignalSet};

use crate::timeline::Timeline;

/// Renders a stretch of the sound, for listening before the whole render: `duration_s` seconds that start at video
/// time `start_s`. Returns interleaved 16-bit samples.
pub fn render_preview(
    settings: &SoundSettings,
    signals: &SignalSet,
    timeline: Timeline,
    start_s: f64,
    duration_s: f64,
) -> Result<Vec<i16>, EngineError> {
    let mut engine = Engine::new(settings.clone(), signals.controls(), timeline, start_s)?;
    Ok(engine.render_s16(frames_for(duration_s, settings.audio.sample_rate)))
}
