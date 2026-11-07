// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The motor presets, heard through the engine: none is much louder or quieter than the others, none overloads the
//! limiter, and each one sounds different from every other. Choosing a preset must not blast the user's ears, and a
//! preset that sounds like another is of no use. Each is heard on a flight of its own kind of aircraft: a fixed wing
//! draws up to 12 A, and a drone five or six times that.

mod common;

use std::f64::consts::TAU;

use common::straight;
use flight_replay_core::synth::{AircraftKind, Controls, Engine, MOTOR_PRESETS, MotorPreset};
use flight_replay_core::telemetry::Series;

const RATE: usize = 48_000;

/// How far the level of a preset may be from that of the default, in dB.
const LEVEL_TOLERANCE_DB: f64 = 1.5;

/// How much two presets must differ in at least one octave band to count as different sounds, in dB.
const MIN_DIFFERENCE_DB: f64 = 3.0;

fn series(name: &str, value: impl Fn(f64) -> f64) -> Series {
    let times: Vec<f64> = (0..=600).map(|i| f64::from(i) * 0.1).collect();
    let values: Vec<f64> = times.iter().map(|&t| value(t)).collect();
    Series::new(name, &times, &values).unwrap()
}

/// What a flight gives the engine: the throttle, the current that goes with it, and the speed.
struct Flight {
    throttle: Series,
    current: Series,
    speed: Series,
    /// A drone has no pitot tube: its speed is the ground speed.
    ground_speed: bool,
}

impl Flight {
    /// The throttle goes up and down once in twenty seconds.
    fn sweep(kind: AircraftKind) -> Self {
        Self::new(kind, |t| 8.0 + 84.0 * (0.5 - 0.5 * (t * TAU / 20.0).cos()))
    }

    /// The throttle stays at 60 %.
    fn steady(kind: AircraftKind) -> Self {
        Self::new(kind, |_| 60.0)
    }

    fn new(kind: AircraftKind, throttle: impl Fn(f64) -> f64 + Copy) -> Self {
        let (current, ground_speed): (Box<dyn Fn(f64) -> f64>, bool) = match kind {
            // 2.6 A with the motor off, 12 A at full throttle
            AircraftKind::FixedWing => (Box::new(move |t| 2.6 + throttle(t) / 100.0 * 9.4), false),
            // 3 A at idle, a five-inch drone draws about 20 A hovering at 35 % and about 90 A at full throttle
            AircraftKind::Multirotor => (
                Box::new(move |t| 3.0 + 90.0 * (throttle(t) / 100.0).powf(2.2)),
                true,
            ),
        };
        Self {
            throttle: series("thr", throttle),
            current: series("cur", current),
            speed: series("speed", |_| 45.0),
            ground_speed,
        }
    }

    fn controls(&self) -> Controls<'_> {
        Controls {
            throttle: &self.throttle,
            current: Some(&self.current),
            air_speed: (!self.ground_speed).then_some(&self.speed),
            gps_speed: self.ground_speed.then_some(&self.speed),
            cell_voltage: None,
            gforce: None,
            roll: None,
            pitch: None,
            motor_idle: None,
        }
    }
}

/// Renders `seconds` of the flight with a preset, in stereo at 48 kHz. Gives the samples and the level in dBFS.
fn render(preset: &MotorPreset, flight: &Flight, seconds: usize) -> (Vec<f64>, f64, i64) {
    let mut engine = Engine::new(preset.settings(), flight.controls(), straight(), 0.0).unwrap();
    let mut samples = vec![0.0; RATE * seconds * 2];
    engine.render(&mut samples);
    let stats = engine.stats();
    (samples, 20.0 * stats.rms.max(1e-9).log10(), stats.limited)
}

/// An in-place radix-2 FFT.
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -TAU / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((angle * k as f64).cos(), (angle * k as f64).sin());
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// The share of the energy in each octave band from 62.5 Hz up, in dB, for the left channel of two seconds of the
/// steady flight taken after the first two.
fn octave_bands(preset: &MotorPreset) -> Vec<f64> {
    const N: usize = 32_768;
    let (samples, _, _) = render(preset, &Flight::steady(preset.kind), 4);
    let mut re: Vec<f64> = samples[2 * RATE..]
        .chunks(2)
        .take(N)
        .enumerate()
        .map(|(i, frame)| frame[0] * (0.5 - 0.5 * (TAU * i as f64 / N as f64).cos()))
        .collect();
    let mut im = vec![0.0; N];
    fft(&mut re, &mut im);

    let mut bands = [0.0; 9];
    for k in 1..N / 2 {
        let hz = k as f64 * RATE as f64 / N as f64;
        if hz >= 62.5 {
            let band = ((hz / 62.5).log2().floor() as usize).min(bands.len() - 1);
            bands[band] += re[k] * re[k] + im[k] * im[k];
        }
    }
    let total: f64 = bands.iter().sum();
    bands
        .iter()
        .map(|b| 10.0 * (b / total + 1e-12).log10())
        .collect()
}

#[test]
fn no_preset_is_much_louder_or_quieter_than_the_default() {
    let (_, reference, _) = render(&MOTOR_PRESETS[0], &Flight::sweep(MOTOR_PRESETS[0].kind), 20);
    for preset in MOTOR_PRESETS {
        let (_, level, _) = render(preset, &Flight::sweep(preset.kind), 20);
        assert!(
            (level - reference).abs() <= LEVEL_TOLERANCE_DB,
            "{}: {level:.1} dBFS against {reference:.1} dBFS of the default",
            preset.name
        );
    }
}

#[test]
fn no_preset_overloads_the_limiter() {
    for preset in MOTOR_PRESETS {
        let (samples, _, limited) = render(preset, &Flight::sweep(preset.kind), 20);
        assert_eq!(limited, 0, "{} went over full scale", preset.name);
        assert!(
            samples.iter().all(|s| s.abs() < 0.95),
            "{} comes near full scale",
            preset.name
        );
    }
}

#[test]
fn every_preset_sounds_different_from_every_other_of_its_kind() {
    // the list offers the presets of one kind of aircraft at a time, so those are the ones that must not be alike
    let bands: Vec<Vec<f64>> = MOTOR_PRESETS.iter().map(octave_bands).collect();
    for (i, a) in MOTOR_PRESETS.iter().enumerate() {
        for (j, b) in MOTOR_PRESETS.iter().enumerate().skip(i + 1) {
            if a.kind != b.kind {
                continue;
            }
            let difference = bands[i]
                .iter()
                .zip(&bands[j])
                .map(|(x, y)| (x - y).abs())
                .fold(0.0, f64::max);
            assert!(
                difference >= MIN_DIFFERENCE_DB,
                "{} and {} differ by only {difference:.1} dB in any octave band",
                a.name,
                b.name
            );
        }
    }
}
