// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/synth/engine.go (Apache-2.0, Copyright 2026 EricoGR).

//! Renders the electric motor and propeller sound from telemetry.
//!
//! The model: the pitch follows the throttle (smoothed, mapped linearly to `f_idle_hz..f_max_hz`, slightly
//! corrected by the battery voltage). The timbre is a fundamental plus decreasing harmonics, and a non-harmonic motor
//! whine; the upper harmonics grow with the load. The volume follows the current above its idle baseline, gated by
//! "motor running" (throttle above the off threshold), so gliding is silent except for the wind. The propeller noise
//! is band-pass noise that follows the load, modulated at the blade-pass frequency. The wind is low-pass noise that
//! follows the air speed. It is the only sound while gliding. An aircraft with several motors, a drone, has a tone
//! for each (see [`super::rotors`]).
//!
//! Rendering is block based and fully deterministic: the control values advance on a fixed grid of absolute sample
//! indices, so the output does not depend on how the caller splits it into blocks.

use std::f64::consts::TAU;

use super::dsp::{Rng, Smoother, Svf, SvfCoef, clamp, clamp01, lerp, soft_clip, stream_seed};
use super::rotors::RotorBank;
use super::settings::{MAX_ROTORS, SoundSettings, Stopped};
use crate::telemetry::{Cursor, Series, at_or};
use crate::timeline::Timeline;

/// The control rate, in samples (1.5 kHz at 48 kHz). The smoothing is evaluated here and the parameters ramp
/// linearly in between.
const CONTROL_INTERVAL: i64 = 32;

// Indices of the parameters that ramp.
const P_INC: usize = 0; // fundamental phase increment (radians per sample)
const P_WHINE_INC: usize = 1;
const P_MOTOR: usize = 2; // tone amplitude
const P_TILT: usize = 3; // harmonic tilt base
const P_PROP: usize = 4;
const P_WIND: usize = 5;
const P_BAL_L: usize = 6;
const P_BAL_R: usize = 7;
const P_RATIO: usize = 8; // speed of each rotor against the common speed, one parameter per rotor
const N_PARAMS: usize = P_RATIO + MAX_ROTORS;

/// The streams of noise of the propeller and the wind: the first of each, and the next one for the second channel
/// (see [`stream_seed`]).
const PROP_NOISE_STREAM: u64 = 0;
const WIND_NOISE_STREAM: u64 = 4;

/// The time constant of the smoothing of the roll angle, which pans the wind, in seconds.
const ROLL_SMOOTHING_S: f64 = 0.25;

/// The telemetry series that drive the sound. The throttle is required.
#[derive(Debug, Clone, Copy)]
pub struct Controls<'a> {
    /// Drives the pitch.
    pub throttle: &'a Series,
    /// Drives the volume. The throttle is used when there is none.
    pub current: Option<&'a Series>,
    /// Drives the wind.
    pub air_speed: Option<&'a Series>,
    /// Drives the wind when there is no air speed.
    pub gps_speed: Option<&'a Series>,
    /// Optional battery RPM correction.
    pub cell_voltage: Option<&'a Series>,
    /// Optional secondary effect.
    pub gforce: Option<&'a Series>,
    /// Optional stereo balance of the wind.
    pub roll: Option<&'a Series>,
    /// Optional load variation.
    pub pitch: Option<&'a Series>,
    /// Optional: 1 where INAV shows `IDLE` in place of the throttle. Read when the motor keeps spinning at idle.
    pub motor_idle: Option<&'a Series>,
}

/// What has been rendered so far.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RenderStats {
    /// Frames rendered.
    pub frames: i64,
    /// Peak magnitude after the limiter, from 0 to 1.
    pub peak: f64,
    /// RMS over all the channels.
    pub rms: f64,
    /// Samples whose magnitude before the limiter exceeded 1.
    pub limited: i64,
}

/// The number of whole frames that cover `duration_s` seconds at `sample_rate`.
#[must_use]
pub fn frames_for(duration_s: f64, sample_rate: u32) -> usize {
    (duration_s * f64::from(sample_rate) - 1e-6).ceil().max(0.0) as usize
}

/// Errors when creating an engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The settings are not valid.
    #[error("invalid sound settings:\n  - {}", .0.join("\n  - "))]
    Settings(Vec<String>),
}

/// The synthesizer. One engine per output; it is not shared between threads while rendering.
pub struct Engine<'a> {
    settings: SoundSettings,
    sample_rate: f64,
    channels: usize,
    timeline: Timeline,
    start: f64,
    position: i64,

    throttle: Cursor<'a>,
    current: Option<Cursor<'a>>,
    air_speed: Option<Cursor<'a>>,
    gps_speed: Option<Cursor<'a>>,
    cell_voltage: Option<Cursor<'a>>,
    gforce: Option<Cursor<'a>>,
    roll: Option<Cursor<'a>>,
    pitch: Option<Cursor<'a>>,
    motor_idle: Option<Cursor<'a>>,
    has_current: bool,

    // control state
    started: bool,
    throttle_smoother: Smoother,
    current_smoother: Smoother,
    speed_smoother: Smoother,
    gate_smoother: Smoother,
    cell_smoother: Smoother,
    gforce_smoother: Smoother,
    roll_smoother: Smoother,
    current_params: [f64; N_PARAMS],
    step: [f64; N_PARAMS],
    target: [f64; N_PARAMS],
    prop_coef: SvfCoef,
    wind_coef: SvfCoef,

    // audio state
    rotors: RotorBank,
    prop_noise: [Rng; 2],
    wind_noise: [Rng; 2],
    prop_filter: [Svf; 2],
    wind_filter: [Svf; 2],
    wind_high_pass: [f64; 2],
    high_pass_coef: f64,
    master_gain: f64,

    // statistics
    peak: f64,
    sum_squares: f64,
    limited: i64,
}

/// What the telemetry says at one moment, before smoothing.
#[derive(Debug, Clone, Copy)]
struct Raw {
    throttle: f64,
    current: f64,
    speed: f64,
    cell: f64,
    gforce: f64,
    roll: f64,
    pitch: f64,
    motor_idle: f64,
}

impl<'a> Engine<'a> {
    /// Prepares an engine that renders audio starting at video time `start` (seconds). `timeline` maps video time to
    /// telemetry time.
    pub fn new(
        settings: SoundSettings,
        controls: Controls<'a>,
        timeline: Timeline,
        start: f64,
    ) -> Result<Self, EngineError> {
        settings.validate().map_err(EngineError::Settings)?;
        let sample_rate = f64::from(settings.audio.sample_rate);
        let channels = usize::from(settings.audio.channels);

        let dt = CONTROL_INTERVAL as f64 / sample_rate;
        let sm = &settings.smoothing;
        let throttle_smoother = Smoother::new(sm.throttle_attack_s, sm.throttle_release_s, dt);
        let current_smoother = Smoother::new(sm.current_attack_s, sm.current_release_s, dt);
        let speed_smoother = Smoother::new(sm.speed_attack_s, sm.speed_release_s, dt);
        let gate_smoother = Smoother::new(sm.gate_attack_s, sm.gate_release_s, dt);
        let cell_smoother = Smoother::new(sm.battery_s, sm.battery_s, dt);
        let gforce_smoother = Smoother::new(sm.current_attack_s, sm.current_release_s, dt);
        let roll_smoother = Smoother::new(ROLL_SMOOTHING_S, ROLL_SMOOTHING_S, dt);

        let rotors = RotorBank::new(&settings, dt);

        let seed = settings.audio.seed;
        let noise = |first: u64| {
            std::array::from_fn(|channel| Rng::new(stream_seed(seed, first + channel as u64)))
        };
        let (prop_noise, wind_noise) = (noise(PROP_NOISE_STREAM), noise(WIND_NOISE_STREAM));
        let high_pass_coef = 1.0 - (-TAU * 40.0 / sample_rate).exp(); // 40 Hz high-pass on the wind
        let master_gain = 10f64.powf(settings.audio.master_gain_db / 20.0);

        Ok(Self {
            sample_rate,
            channels,
            timeline,
            start,
            position: 0,
            throttle: controls.throttle.cursor(),
            current: controls.current.map(Series::cursor),
            air_speed: controls.air_speed.map(Series::cursor),
            gps_speed: controls.gps_speed.map(Series::cursor),
            cell_voltage: controls.cell_voltage.map(Series::cursor),
            gforce: controls.gforce.map(Series::cursor),
            roll: controls.roll.map(Series::cursor),
            pitch: controls.pitch.map(Series::cursor),
            motor_idle: controls.motor_idle.map(Series::cursor),
            has_current: controls.current.is_some(),
            started: false,
            throttle_smoother,
            current_smoother,
            speed_smoother,
            gate_smoother,
            cell_smoother,
            gforce_smoother,
            roll_smoother,
            current_params: [0.0; N_PARAMS],
            step: [0.0; N_PARAMS],
            target: [0.0; N_PARAMS],
            prop_coef: SvfCoef::default(),
            wind_coef: SvfCoef::default(),
            rotors,
            prop_noise,
            wind_noise,
            prop_filter: [Svf::default(); 2],
            wind_filter: [Svf::default(); 2],
            wind_high_pass: [0.0; 2],
            high_pass_coef,
            master_gain,
            peak: 0.0,
            sum_squares: 0.0,
            limited: 0,
            settings,
        })
    }

    /// The peak, RMS and limiter activity so far.
    #[must_use]
    pub fn stats(&self) -> RenderStats {
        RenderStats {
            frames: self.position,
            peak: self.peak,
            rms: if self.position > 0 {
                (self.sum_squares / (self.position * self.channels as i64) as f64).sqrt()
            } else {
                0.0
            },
            limited: self.limited,
        }
    }

    /// Number of channels of the audio.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Renders `frames` frames of interleaved 16-bit samples.
    pub fn render_s16(&mut self, frames: usize) -> Vec<i16> {
        let mut buffer = vec![0.0; frames * self.channels];
        self.render(&mut buffer);
        buffer
            .iter()
            .map(|&v| (clamp(v, -1.0, 1.0) * 32767.0).round() as i16)
            .collect()
    }

    /// Renders `dst.len() / channels` frames of interleaved samples in [-1, 1] into `dst`.
    pub fn render(&mut self, dst: &mut [f64]) {
        let channels = self.channels;
        let frames = dst.len() / channels;
        let blade_depth = clamp01(self.settings.prop_noise.blade_mod_depth);
        let whine_gain = self.settings.tone.whine_gain;
        let active = self.active_params();

        for i in 0..frames {
            if self.position % CONTROL_INTERVAL == 0 {
                self.tick();
            }
            for j in 0..active {
                self.current_params[j] += self.step[j];
            }

            let voice = self.rotors.voice(
                self.current_params[P_INC],
                self.current_params[P_WHINE_INC],
                self.current_params[P_TILT],
                &self.current_params[P_RATIO..active],
            );
            let whine = voice.whine * whine_gain;

            let motor = self.current_params[P_MOTOR];
            // the blades of all the motors beat the air: the noise follows their mean
            let prop_amp = self.current_params[P_PROP] * (1.0 + blade_depth * voice.blade);
            let wind_amp = self.current_params[P_WIND];

            for ch in 0..channels {
                let (_, band) =
                    self.prop_filter[ch].run(self.prop_coef, self.prop_noise[ch].next());
                let (low, _) = self.wind_filter[ch].run(self.wind_coef, self.wind_noise[ch].next());
                self.wind_high_pass[ch] += self.high_pass_coef * (low - self.wind_high_pass[ch]);
                let wind = low - self.wind_high_pass[ch];

                let balance = if ch == 1 {
                    self.current_params[P_BAL_R]
                } else {
                    self.current_params[P_BAL_L]
                };
                let mut y =
                    motor * (voice.tone[ch] + whine) + prop_amp * band + wind_amp * balance * wind;
                y *= self.master_gain;

                if y.abs() > 1.0 {
                    self.limited += 1;
                }
                y = soft_clip(y);
                if y.abs() > self.peak {
                    self.peak = y.abs();
                }
                self.sum_squares += y * y;
                dst[i * channels + ch] = y;
            }
            self.position += 1;
        }
    }

    /// Runs at the control rate: advances the smoothers using the telemetry at the end of the coming interval and
    /// computes the ramp towards the new parameters.
    fn tick(&mut self) {
        if !self.started {
            self.init_state();
            self.started = true;
        }
        self.current_params = self.target; // ramp from the previous target exactly

        let t = self.telemetry_at(
            self.start + (self.position + CONTROL_INTERVAL) as f64 / self.sample_rate,
        );
        let raw = self.sample(t);
        self.advance(raw);
        self.compute_targets(raw);
        for j in 0..self.active_params() {
            self.step[j] = (self.target[j] - self.current_params[j]) / CONTROL_INTERVAL as f64;
        }
    }

    /// How many of the ramping parameters are in use: the fixed ones, and a speed for each motor.
    fn active_params(&self) -> usize {
        P_RATIO + self.rotors.count()
    }

    fn telemetry_at(&self, video_time: f64) -> f64 {
        self.timeline.telemetry_time(video_time)
    }

    /// Seeds every smoother with the telemetry value at the start, so a preview that begins mid-flight does not fade
    /// in from silence.
    fn init_state(&mut self) {
        let t = self.telemetry_at(self.start);
        let raw = self.sample(t);
        self.throttle_smoother.y = raw.throttle;
        self.current_smoother.y = raw.current;
        self.speed_smoother.y = raw.speed;
        self.cell_smoother.y = raw.cell;
        self.gforce_smoother.y = raw.gforce;
        self.roll_smoother.y = raw.roll;
        self.rotors.start(raw.roll, raw.pitch);
        self.gate_smoother.y = self.gate_target(raw);
        self.compute_targets(raw);
        self.current_params = self.target;
    }

    fn sample(&mut self, t: f64) -> Raw {
        let gps = at_or(self.gps_speed.as_mut(), t, 0.0);
        Raw {
            // a negative throttle counts as zero
            throttle: self.throttle.at(t).max(0.0),
            current: at_or(self.current.as_mut(), t, 0.0),
            // the wind follows the air speed, else the ground speed
            speed: at_or(self.air_speed.as_mut(), t, gps),
            cell: at_or(self.cell_voltage.as_mut(), t, 0.0),
            gforce: at_or(self.gforce.as_mut(), t, 1.0), // 1 g is level flight
            roll: at_or(self.roll.as_mut(), t, 0.0),
            pitch: at_or(self.pitch.as_mut(), t, 0.0),
            motor_idle: at_or(self.motor_idle.as_mut(), t, 0.0),
        }
    }

    fn advance(&mut self, raw: Raw) {
        self.throttle_smoother.step(raw.throttle);
        self.current_smoother.step(raw.current);
        self.speed_smoother.step(raw.speed);
        self.cell_smoother.step(raw.cell);
        self.gforce_smoother.step(raw.gforce);
        self.roll_smoother.step(raw.roll);
        let gate = self.gate_target(raw);
        self.gate_smoother.step(gate);
        self.rotors.advance(raw.roll, raw.pitch);
    }

    /// Decides whether the motor is sounding: running above the off threshold; otherwise silent, a quiet hum on the
    /// ground in `idle` mode, or spinning at its idle speed where INAV says the motor idles in `spinning` mode.
    fn gate_target(&self, raw: Raw) -> f64 {
        let m = &self.settings.motor;
        if raw.throttle > m.off_threshold_pct {
            return 1.0;
        }
        match m.stopped {
            Stopped::Idle if raw.speed <= m.stop_speed_kmh => m.idle_gain,
            Stopped::Spinning if raw.motor_idle > 0.5 => 1.0,
            _ => 0.0,
        }
    }

    fn compute_targets(&mut self, raw: Raw) {
        let settings = &self.settings;
        let motor = &settings.motor;

        // pitch
        let throttle_norm = clamp01(self.throttle_smoother.y / 100.0).powf(motor.throttle_curve);
        let frequency =
            lerp(motor.f_idle_hz, motor.f_max_hz, throttle_norm) * self.battery_factor();
        self.target[P_INC] = TAU * frequency / self.sample_rate;
        self.target[P_WHINE_INC] = TAU * frequency * settings.tone.whine_ratio / self.sample_rate;

        // load from the current above its idle baseline (the throttle if there is no current)
        let loudness = &settings.loudness;
        let mut load = if self.has_current {
            clamp01(
                (self.current_smoother.y - loudness.current_base_a)
                    / (loudness.current_full_a - loudness.current_base_a),
            )
        } else {
            throttle_norm
        };
        load = load.powf(loudness.curve);
        if self.pitch.is_some() {
            load = clamp01(load * (1.0 + loudness.pitch_gain * clamp(raw.pitch / 30.0, -1.0, 1.0)));
        }
        let volume = loudness.floor + (1.0 - loudness.floor) * load;
        let gate = self.gate_smoother.y;

        self.target[P_MOTOR] = gate * volume * settings.tone.gain;
        self.target[P_TILT] = (1.0 - settings.tone.brightness) + settings.tone.brightness * load;

        let mut gforce_factor = 1.0;
        if settings.gforce.gain != 0.0 && self.gforce.is_some() {
            gforce_factor =
                (1.0 + settings.gforce.gain * (self.gforce_smoother.y - 1.0).abs()).max(0.0);
        }

        let prop = &settings.prop_noise;
        self.target[P_PROP] = prop.gain * gate * (0.15 + 0.85 * load) * gforce_factor;
        self.prop_coef = SvfCoef::new(
            lerp(prop.center_low_hz, prop.center_high_hz, throttle_norm),
            prop.q,
            self.sample_rate,
        );

        let wind = &settings.wind;
        let wind_norm = clamp(self.speed_smoother.y / wind.speed_ref_kmh, 0.0, 1.5);
        self.target[P_WIND] = wind.gain * wind_norm.powf(wind.curve) * gforce_factor;
        self.wind_coef = SvfCoef::new(
            lerp(wind.cutoff_low_hz, wind.cutoff_high_hz, wind_norm.min(1.0)),
            0.7,
            self.sample_rate,
        );

        let mut balance = 0.0;
        if self.channels == 2 && self.roll.is_some() {
            balance = wind.roll_balance * clamp(self.roll_smoother.y / 45.0, -1.0, 1.0);
        }
        self.target[P_BAL_L] = 1.0 - balance;
        self.target[P_BAL_R] = 1.0 + balance;

        let active = self.active_params();
        self.rotors.ratios(&mut self.target[P_RATIO..active]);
    }

    /// Lowers the RPM as the (sag-corrected) cell voltage drops.
    fn battery_factor(&self) -> f64 {
        let m = &self.settings.motor;
        if m.battery_gain == 0.0 || self.cell_voltage.is_none() {
            return 1.0;
        }
        let v = self.cell_smoother.y + self.current_smoother.y * m.battery_sag_mohm / 1000.0;
        clamp(
            1.0 - m.battery_gain * (1.0 - v / m.battery_ref_v),
            0.6,
            1.15,
        )
    }
}
