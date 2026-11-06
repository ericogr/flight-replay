// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/synth/dsp.go (Apache-2.0, Copyright 2026 EricoGR).

//! The building blocks of the synthesizer: smoothers, state-variable filters, a noise generator and a soft limiter.

use std::f64::consts::PI;

/// A one-pole low-pass with separate attack (rising) and release (falling) time constants: the inertia of the
/// motor and the propeller.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Smoother {
    pub(super) y: f64,
    attack: f64,
    release: f64,
}

impl Smoother {
    /// A smoother stepped every `dt` seconds. A time constant of 0 means no smoothing.
    pub(super) fn new(attack_s: f64, release_s: f64, dt: f64) -> Self {
        Self {
            y: 0.0,
            attack: coefficient(attack_s, dt),
            release: coefficient(release_s, dt),
        }
    }

    pub(super) fn step(&mut self, x: f64) -> f64 {
        let c = if x > self.y {
            self.attack
        } else {
            self.release
        };
        self.y += (x - self.y) * c;
        self.y
    }
}

fn coefficient(tau: f64, dt: f64) -> f64 {
    if tau <= 0.0 {
        1.0
    } else {
        1.0 - (-dt / tau).exp()
    }
}

/// The coefficients of a TPT state-variable filter (Simper). It stays stable and click-free while the cutoff is
/// modulated.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SvfCoef {
    a1: f64,
    a2: f64,
    a3: f64,
    k: f64,
}

impl SvfCoef {
    pub(super) fn new(cutoff: f64, q: f64, sample_rate: f64) -> Self {
        let g = (PI * cutoff.min(0.49 * sample_rate) / sample_rate).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        Self {
            a1,
            a2: g * a1,
            a3: g * g * a1,
            k,
        }
    }
}

/// The state of one state-variable filter.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Svf {
    ic1: f64,
    ic2: f64,
}

impl Svf {
    /// Returns the low-pass and the peak-normalised band-pass outputs.
    pub(super) fn run(&mut self, c: SvfCoef, x: f64) -> (f64, f64) {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, c.k * v1)
    }
}

/// A xorshift64* generator: deterministic, allocation-free, good enough for audio noise.
#[derive(Debug, Clone, Copy)]
pub(super) struct Rng {
    state: u64,
}

impl Rng {
    pub(super) fn new(seed: u64) -> Self {
        // splitmix64, so that nearby seeds give unrelated streams
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        if z == 0 {
            z = 0x9E37_79B9_7F4A_7C15;
        }
        Self { state: z }
    }

    /// White noise uniformly distributed in [-1, 1).
    pub(super) fn next(&mut self) -> f64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        let scrambled = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        ((scrambled as i64) >> 11) as f64 / (1u64 << 52) as f64
    }
}

/// A rational tanh approximation: unit slope at 0, saturates at plus or minus 1.
pub(super) fn soft_clip(x: f64) -> f64 {
    if x >= 3.0 {
        return 1.0;
    }
    if x <= -3.0 {
        return -1.0;
    }
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

pub(super) fn clamp01(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

pub(super) fn clamp(x: f64, low: f64, high: f64) -> f64 {
    low.max(high.min(x))
}

pub(super) fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Keeps an oscillator phase in [0, 2 pi). The phase (not the time) is what accumulates, so changing the frequency
/// never makes the waveform jump, unlike `sin(2 pi f t)`. The increment per sample is always below 2 pi.
pub(super) fn wrap_phase(phase: f64) -> f64 {
    if phase >= 2.0 * PI {
        phase - 2.0 * PI
    } else {
        phase
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_smoother_rises_and_falls_at_its_own_speed() {
        let mut s = Smoother::new(0.1, 0.4, 0.01);
        let mut rise = 0;
        while s.step(1.0) < 0.63 {
            rise += 1;
        }
        // one time constant is 10 steps of 10 ms
        assert!((8..=12).contains(&rise), "{rise}");
        let mut fall = 0;
        while s.step(0.0) > 0.37 {
            fall += 1;
        }
        // from 63 % down to 37 % takes about half a release time constant (40 steps): longer than the attack
        assert!(fall > rise * 2, "release {fall} steps, attack {rise}");
        assert_eq!(Smoother::new(0.0, 0.0, 0.01).step(5.0), 5.0, "no smoothing");
    }

    #[test]
    fn the_noise_is_deterministic_and_centred() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(1);
        let mut other = Rng::new(2);
        let first: Vec<f64> = (0..1000).map(|_| a.next()).collect();
        assert!(
            first
                .iter()
                .zip((0..1000).map(|_| b.next()))
                .all(|(x, y)| x.to_bits() == y.to_bits())
        );
        assert!(
            first
                .iter()
                .zip((0..1000).map(|_| other.next()))
                .any(|(x, y)| x.to_bits() != y.to_bits())
        );
        assert!(first.iter().all(|v| (-1.0..1.0).contains(v)));
        let mean: f64 = first.iter().sum::<f64>() / 1000.0;
        assert!(mean.abs() < 0.1, "mean {mean}");
    }

    #[test]
    fn the_soft_clip_saturates_and_has_unit_slope_at_zero() {
        assert_eq!(soft_clip(10.0), 1.0);
        assert_eq!(soft_clip(-10.0), -1.0);
        assert!((soft_clip(0.001) - 0.001).abs() < 1e-6);
        assert!(soft_clip(2.0) < 1.0 && soft_clip(2.0) > 0.9);
    }

    #[test]
    fn the_low_pass_passes_low_and_blocks_high_frequencies() {
        let sr = 48_000.0;
        let coef = SvfCoef::new(500.0, 0.7, sr);
        let power = |hz: f64| {
            let mut filter = Svf::default();
            let mut sum = 0.0;
            for n in 0..48_000 {
                let (low, _) = filter.run(coef, (2.0 * PI * hz * f64::from(n) / sr).sin());
                if n > 4_800 {
                    sum += low * low;
                }
            }
            sum
        };
        assert!(power(100.0) > 20.0 * power(5_000.0));
    }
}
