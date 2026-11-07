// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The motors of an aircraft, as the engine hears them.
//!
//! A fixed wing has one motor, and its tone is one harmonic series. A drone has several (usually four), and they never
//! turn at exactly the same speed: the controller corrects each one all the time. The rough, buzzing sound of a drone
//! is the beating of those series against one another, so each motor gets a series of its own, at its own speed.
//!
//! The speed of motor `r` is the common speed (the one the throttle gives) times a ratio, kept near 1:
//!
//! `1 + spread[r] * detune + wander[r] + attitude * (roll_sign[r] * roll_rate + pitch_sign[r] * pitch_rate)`
//!
//! With one motor the ratio is exactly 1 and the tone is what it was before there was more than one: the arithmetic
//! of that path is unchanged, so the sound of a fixed wing is the same, sample by sample.

use std::f64::consts::{PI, SQRT_2, TAU};

use super::dsp::{Rng, Smoother, clamp, stream_seed, wrap_phase};
use super::settings::{ROTOR_RATIO_SPAN, SoundSettings};

/// The conjugate of the golden ratio, as the Go program has it (truncated: the phases of the only motor must stay
/// what they were). Multiples of it, taken modulo 1, are spread evenly without a pattern, so the harmonics, the
/// motors and their speeds are told apart without a random generator.
const GOLDEN_RATIO_CONJUGATE: f64 = 0.618_033_988_7;

/// How long the random drift of a motor's speed takes to forget, in seconds.
const WANDER_TIME_S: f64 = 0.4;

/// The standard deviation of the noise the drift is made of (uniform in -1..1): the square root of 1/3.
const UNIFORM_NOISE_DEVIATION: f64 = 0.577_350_269_189_626;

/// The time constant of the roll and pitch rates, in seconds, and the largest rate that counts, in degrees per
/// second.
const RATE_SMOOTHING_S: f64 = 0.1;
const RATE_LIMIT_DPS: f64 = 720.0;

/// The first of the streams of noise of the drifts, one for each motor. It is far from the streams of the propeller
/// and the wind (`seed * 8` and the five after it), so no two of them are the same noise.
const WANDER_STREAM: u64 = 1 << 20;

/// What the motors sound like at one sample.
#[derive(Debug, Clone, Copy)]
pub(super) struct Voice {
    /// The tone, for each channel (the second one is 0 in mono).
    pub(super) tone: [f64; 2],
    /// The whine of the motors, the same in every channel.
    pub(super) whine: f64,
    /// The mean over the motors of the sine of the blade-pass phase, which modulates the propeller noise.
    pub(super) blade: f64,
}

/// The motors of the aircraft: their tone, and what makes their speeds differ.
pub(super) struct RotorBank {
    count: usize,
    stereo: bool,
    /// Seconds between two calls of [`RotorBank::advance`].
    dt: f64,
    /// The amplitude of the fundamental and of each harmonic.
    harmonics: Vec<f64>,
    detune: f64,
    attitude: f64,
    /// Each motor's share of the fixed difference of speed, from -1 to 1, with a mean of 0.
    spread: Vec<f64>,
    /// How much a roll to the right slows the motor: -1 on the right, 1 on the left.
    roll_sign: Vec<f64>,
    /// How much a pitch up speeds the motor: 1 at the front, -1 at the rear.
    pitch_sign: Vec<f64>,
    /// `cos_offset[motor][harmonic]`: the fixed phase of each harmonic, as a cosine and a sine.
    cos_offset: Vec<Vec<f64>>,
    sin_offset: Vec<Vec<f64>>,
    /// `pan[motor][channel][harmonic]`
    pan: Vec<[Vec<f64>; 2]>,
    /// Added up, the motors are as loud as one: their sounds do not add in phase.
    norm: f64,

    phases: Vec<f64>,
    whine_phases: Vec<f64>,
    wander: Vec<f64>,
    wander_noise: Vec<Rng>,
    wander_alpha: f64,
    wander_gain: f64,
    roll_rate: Smoother,
    pitch_rate: Smoother,
    prev_roll: f64,
    prev_pitch: f64,
}

impl RotorBank {
    /// The motors of `settings`. `dt` is the time, in seconds, between two calls of [`RotorBank::advance`].
    pub(super) fn new(settings: &SoundSettings, dt: f64) -> Self {
        let count = usize::from(settings.rotors.count);
        let harmonics = settings.tone.harmonics.clone();
        let channels = usize::from(settings.audio.channels);
        let positions = positions(count);
        let (cos_offset, sin_offset) = harmonic_offsets(count, harmonics.len());
        let pan = pan_table(
            &positions,
            harmonics.len(),
            channels,
            settings.tone.stereo_width,
        );
        // The first motor starts at phase 0, as the only motor always did; the others start apart, so that they do
        // not begin in step.
        let phases: Vec<f64> = (0..count)
            .map(|r| TAU * ((r as f64 * (1.0 - GOLDEN_RATIO_CONJUGATE)) % 1.0))
            .collect();
        let (wander_alpha, wander_gain) = wander_coefficients(settings.rotors.wander, dt);
        let seed = settings.audio.seed;

        Self {
            count,
            stereo: channels == 2,
            dt,
            harmonics,
            detune: settings.rotors.detune,
            attitude: settings.rotors.attitude,
            spread: spread(count),
            roll_sign: positions.iter().map(|&(x, _)| -x).collect(),
            pitch_sign: positions.iter().map(|&(_, y)| y).collect(),
            cos_offset,
            sin_offset,
            pan,
            norm: 1.0 / (count as f64).sqrt(),
            whine_phases: phases.clone(),
            phases,
            wander: vec![0.0; count],
            wander_noise: (0..count)
                .map(|r| Rng::new(stream_seed(seed, WANDER_STREAM + r as u64)))
                .collect(),
            wander_alpha,
            wander_gain,
            roll_rate: Smoother::new(RATE_SMOOTHING_S, RATE_SMOOTHING_S, dt),
            pitch_rate: Smoother::new(RATE_SMOOTHING_S, RATE_SMOOTHING_S, dt),
            prev_roll: 0.0,
            prev_pitch: 0.0,
        }
    }

    /// The number of motors.
    pub(super) fn count(&self) -> usize {
        self.count
    }

    /// Starts from the attitude the aircraft has at the start, so the first rate is not a jump from zero.
    pub(super) fn start(&mut self, roll: f64, pitch: f64) {
        self.prev_roll = roll;
        self.prev_pitch = pitch;
    }

    /// Moves what makes the speeds differ, by one step of `dt`: how fast the aircraft rolls and pitches (in degrees,
    /// as the OSD shows them), and the drift of each motor. A single motor has nothing to move.
    pub(super) fn advance(&mut self, roll: f64, pitch: f64) {
        if self.count == 1 {
            return;
        }
        self.roll_rate
            .step(angle_difference(roll, self.prev_roll) / self.dt);
        self.pitch_rate
            .step(angle_difference(pitch, self.prev_pitch) / self.dt);
        self.prev_roll = roll;
        self.prev_pitch = pitch;
        for (wander, noise) in self.wander.iter_mut().zip(&mut self.wander_noise) {
            *wander += self.wander_alpha * (self.wander_gain * noise.next() - *wander);
        }
    }

    /// Writes into `out`, one for each motor, the speed of the motor over the common speed.
    pub(super) fn ratios(&self, out: &mut [f64]) {
        debug_assert_eq!(out.len(), self.count);
        if self.count == 1 {
            out[0] = 1.0;
            return;
        }
        let roll = clamp(self.roll_rate.y, -RATE_LIMIT_DPS, RATE_LIMIT_DPS);
        let pitch = clamp(self.pitch_rate.y, -RATE_LIMIT_DPS, RATE_LIMIT_DPS);
        for (r, ratio) in out.iter_mut().enumerate() {
            let follow = self.attitude * (self.roll_sign[r] * roll + self.pitch_sign[r] * pitch);
            let ratio_now = 1.0 + self.spread[r] * self.detune + self.wander[r] + follow;
            *ratio = clamp(ratio_now, 1.0 - ROTOR_RATIO_SPAN, 1.0 + ROTOR_RATIO_SPAN);
        }
    }

    /// The next sample of the motors. `increment` and `whine_increment` are the phase increments, in radians per
    /// sample, of the common speed, `tilt` is the base of the tilt of the harmonics, and `ratios` has the speed of
    /// each motor over the common one.
    pub(super) fn voice(
        &mut self,
        increment: f64,
        whine_increment: f64,
        tilt: f64,
        ratios: &[f64],
    ) -> Voice {
        let mut tone = [0.0f64; 2];
        let mut whine = 0.0;
        let mut blade = 0.0;
        for (r, &ratio) in ratios.iter().enumerate() {
            self.phases[r] = wrap_phase(self.phases[r] + increment * ratio);
            self.whine_phases[r] = wrap_phase(self.whine_phases[r] + whine_increment * ratio);
            let (s1, c1) = self.phases[r].sin_cos();
            blade += s1;
            // The harmonic series by the angle-addition recurrence: one sin_cos per sample and motor instead of one
            // sin per harmonic.
            let (mut sk, mut ck) = (s1, c1);
            let mut tilt_power = 1.0;
            for (k, amplitude) in self.harmonics.iter().enumerate() {
                let v = sk * self.cos_offset[r][k] + ck * self.sin_offset[r][k];
                let w = amplitude * tilt_power * v;
                tone[0] += w * self.pan[r][0][k];
                if self.stereo {
                    tone[1] += w * self.pan[r][1][k];
                }
                tilt_power *= tilt;
                (sk, ck) = (sk * c1 + ck * s1, ck * c1 - sk * s1);
            }
            whine += self.whine_phases[r].sin();
        }
        Voice {
            tone: [tone[0] * self.norm, tone[1] * self.norm],
            whine: whine * self.norm,
            blade: blade / self.count as f64,
        }
    }
}

/// The one-pole coefficient of the drift of a motor's speed and the gain of the noise that drives it, so that the
/// drift has a standard deviation of `wander` (as a fraction of the speed).
fn wander_coefficients(wander: f64, dt: f64) -> (f64, f64) {
    let alpha = 1.0 - (-dt / WANDER_TIME_S).exp();
    (
        alpha,
        wander / (UNIFORM_NOISE_DEVIATION * (alpha / (2.0 - alpha)).sqrt()),
    )
}

/// The fixed phase of the harmonics, as `(cos, sin)` for each motor and harmonic. They decorrelate the harmonics so
/// the summed waveform has a low crest factor (no "pulse train" peaks). Each motor has its own, so that when their
/// speeds meet, the peaks of their waveforms do not add up. The first motor has the phases of the only motor.
fn harmonic_offsets(count: usize, harmonics: usize) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    (0..count)
        .map(|r| {
            (0..harmonics)
                .map(|k| {
                    let d = TAU * (((k as f64 + r as f64 * 0.5) * GOLDEN_RATIO_CONJUGATE) % 1.0);
                    (d.cos(), d.sin())
                })
                .unzip()
        })
        .unzip()
}

/// The constant-power pan of every harmonic of every motor, as `pan[motor][channel][harmonic]`. The fundamental of
/// the only motor stays centred; the higher harmonics alternate sides, scaled by the stereo `width`. A motor on the
/// left or the right of the aircraft sounds on that side, by the same width.
fn pan_table(
    positions: &[(f64, f64)],
    harmonics: usize,
    channels: usize,
    width: f64,
) -> Vec<[Vec<f64>; 2]> {
    positions
        .iter()
        .map(|&(x, _)| {
            let mut pan: [Vec<f64>; 2] =
                [Vec::with_capacity(harmonics), Vec::with_capacity(harmonics)];
            for k in 0..harmonics {
                let mut offset = 0.0;
                if channels == 2 {
                    if k > 0 {
                        let side = if k % 2 == 1 { -1.0 } else { 1.0 };
                        offset = side * width * (k as f64 / 3.0).min(1.0);
                    }
                    offset = clamp(offset + x * width, -1.0, 1.0);
                }
                let theta = (offset + 1.0) * PI / 4.0;
                pan[0].push(SQRT_2 * theta.cos());
                pan[1].push(SQRT_2 * theta.sin());
            }
            pan
        })
        .collect()
}

/// Where the motors are, seen from above: `(x to the right, y to the front)`, both scaled so that the outermost
/// motor is at 1. One motor is in the middle. Several are spread on a circle, the first at the front right, so four
/// are at the corners of an X.
fn positions(count: usize) -> Vec<(f64, f64)> {
    if count <= 1 {
        return vec![(0.0, 0.0)];
    }
    let circle: Vec<(f64, f64)> = (0..count)
        .map(|r| {
            let angle = TAU * (r as f64 + 0.5) / count as f64;
            (angle.sin(), angle.cos())
        })
        .collect();
    let widest =
        |axis: fn(&(f64, f64)) -> f64| circle.iter().map(|p| axis(p).abs()).fold(1e-9, f64::max);
    let (x_max, y_max) = (widest(|p| p.0), widest(|p| p.1));
    circle
        .iter()
        .map(|&(x, y)| (x / x_max, y / y_max))
        .collect()
}

/// A different number for each motor, from -1 to 1, with a mean of 0 (so the pitch stays where the throttle puts
/// it), spread without a pattern.
fn spread(count: usize) -> Vec<f64> {
    if count <= 1 {
        return vec![0.0];
    }
    let raw: Vec<f64> = (1..=count)
        .map(|r| (r as f64 * GOLDEN_RATIO_CONJUGATE) % 1.0)
        .collect();
    let mean = raw.iter().sum::<f64>() / count as f64;
    let widest = raw.iter().map(|v| (v - mean).abs()).fold(1e-9, f64::max);
    raw.iter().map(|v| (v - mean) / widest).collect()
}

/// The shortest signed difference between two angles in degrees, from -180 to 180: a roll that goes round past 180
/// degrees is a small step, not a jump of 360.
fn angle_difference(now: f64, before: f64) -> f64 {
    let d = (now - before) % 360.0;
    if d > 180.0 {
        d - 360.0
    } else if d < -180.0 {
        d + 360.0
    } else {
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_motors_are_at_the_corners_of_an_x_and_one_is_in_the_middle() {
        assert_eq!(positions(1), [(0.0, 0.0)]);
        // front right, rear right, rear left, front left
        let four = positions(4);
        for (&(x, y), (ex, ey)) in
            four.iter()
                .zip([(1.0, 1.0), (1.0, -1.0), (-1.0, -1.0), (-1.0, 1.0)])
        {
            assert!((x - ex).abs() < 1e-9 && (y - ey).abs() < 1e-9, "{four:?}");
        }
    }

    #[test]
    fn the_motors_of_any_number_stay_within_the_unit_square() {
        for count in 2..=8 {
            let all = positions(count);
            assert!(
                all.iter()
                    .all(|&(x, y)| x.abs() <= 1.0 + 1e-9 && y.abs() <= 1.0 + 1e-9),
                "{count}: {all:?}"
            );
            // the outermost motor is at 1 (the front-rear axis is flat for two motors side by side, so it is not
            // checked)
            let widest = all.iter().fold(0.0f64, |m, &(x, _)| m.max(x.abs()));
            assert!((widest - 1.0).abs() < 1e-9, "{count}: {all:?}");
        }
    }

    #[test]
    fn the_fixed_differences_of_speed_have_a_mean_of_zero_and_reach_one() {
        assert_eq!(spread(1), [0.0]);
        for count in 2..=8 {
            let all = spread(count);
            let mean = all.iter().sum::<f64>() / count as f64;
            let widest = all.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            assert!(mean.abs() < 1e-9, "{count}: mean {mean}");
            assert!((widest - 1.0).abs() < 1e-9, "{count}: {all:?}");
            assert!(
                all.iter()
                    .enumerate()
                    .all(|(i, a)| all[i + 1..].iter().all(|b| (a - b).abs() > 1e-6)),
                "{count}: two motors are at the same speed: {all:?}"
            );
        }
    }

    #[test]
    fn a_roll_past_180_degrees_is_a_small_step() {
        assert!((angle_difference(-179.0, 179.0) - 2.0).abs() < 1e-9);
        assert!((angle_difference(179.0, -179.0) + 2.0).abs() < 1e-9);
        assert!((angle_difference(10.0, 4.0) - 6.0).abs() < 1e-9);
        assert!((angle_difference(4.0, 10.0) + 6.0).abs() < 1e-9);
    }

    #[test]
    fn the_speed_of_a_motor_never_strays_far_from_the_common_speed() {
        let mut settings = SoundSettings::default();
        settings.rotors.count = 4;
        settings.rotors.detune = 0.1;
        settings.rotors.attitude = 0.01;
        let mut bank = RotorBank::new(&settings, 0.001);
        // a violent flip: the rates are limited and so are the ratios
        for step in 0..2000 {
            bank.advance(
                f64::from(step) * 30.0 % 360.0 - 180.0,
                f64::from(step) * 17.0 % 360.0 - 180.0,
            );
        }
        let mut ratios = [0.0; 4];
        bank.ratios(&mut ratios);
        for ratio in ratios {
            assert!(
                (1.0 - ROTOR_RATIO_SPAN..=1.0 + ROTOR_RATIO_SPAN).contains(&ratio),
                "{ratios:?}"
            );
        }
    }

    #[test]
    fn a_single_motor_runs_at_the_common_speed() {
        let bank = RotorBank::new(&SoundSettings::default(), 0.001);
        let mut ratios = [0.0];
        bank.ratios(&mut ratios);
        assert_eq!(ratios, [1.0]);
    }
}
