// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Properties of the synthesizer, from its public interface.

use std::f64::consts::TAU;

use flight_replay_core::synth::{Controls, Engine, SoundSettings, Stopped};
use flight_replay_core::telemetry::Series;
use flight_replay_core::timeline::{ScaleMode, SyncOptions, Timeline};

fn straight() -> Timeline {
    Timeline::new(
        SyncOptions {
            scale: ScaleMode::Off,
            offset_s: 0.0,
        },
        60.0,
        60.0,
    )
}

/// A series that holds one value for 60 seconds.
fn constant(name: &str, value: f64) -> Series {
    let times: Vec<f64> = (0..=60).map(f64::from).collect();
    Series::new(name, &times, &vec![value; times.len()]).unwrap()
}

fn throttle_only(series: &Series) -> Controls<'_> {
    Controls {
        throttle: series,
        current: None,
        air_speed: None,
        gps_speed: None,
        cell_voltage: None,
        gforce: None,
        roll: None,
        pitch: None,
    }
}

/// The power of the signal at `hz`, by the Goertzel algorithm, for the left channel of an interleaved buffer.
fn tone_power(samples: &[f64], channels: usize, hz: f64, sample_rate: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    let mut n = 0.0;
    for frame in samples.chunks(channels) {
        let phase = TAU * hz * n / sample_rate;
        re += frame[0] * phase.cos();
        im += frame[0] * phase.sin();
        n += 1.0;
    }
    (re * re + im * im) / (n * n)
}

#[test]
fn the_output_is_deterministic_and_does_not_depend_on_the_block_size() {
    let throttle = constant("thr", 50.0);
    let current = constant("cur", 8.0);
    let air = constant("air", 40.0);
    let controls = Controls {
        current: Some(&current),
        air_speed: Some(&air),
        ..throttle_only(&throttle)
    };
    let render = |block: usize| {
        let mut engine = Engine::new(SoundSettings::default(), controls, straight(), 5.0).unwrap();
        let mut out = Vec::new();
        let mut left = 30_000;
        while left > 0 {
            let n = block.min(left);
            out.extend(engine.render_s16(n));
            left -= n;
        }
        out
    };
    let whole = render(30_000);
    assert_eq!(whole, render(30_000), "the same input gives the same bytes");
    assert_eq!(whole, render(1), "one frame at a time");
    assert_eq!(whole, render(8192), "blocks of 8192 frames");
    assert_eq!(whole, render(777), "an odd block size");
    assert!(whole.iter().any(|&s| s != 0));
}

#[test]
fn the_pitch_follows_the_throttle() {
    let sr = 48_000.0;
    // 50 % throttle is halfway between the idle and the maximum frequency: 110 + 0.5 * (520 - 110) = 315 Hz.
    let throttle = constant("thr", 50.0);
    let mut settings = SoundSettings::default();
    settings.audio.channels = 1;
    let mut engine = Engine::new(settings, throttle_only(&throttle), straight(), 0.0).unwrap();
    let mut samples = vec![0.0; 48_000];
    engine.render(&mut samples);

    let at = |hz| tone_power(&samples, 1, hz, sr);
    let peak = at(315.0);
    assert!(peak > 20.0 * at(250.0), "the 315 Hz tone is missing");
    assert!(peak > 20.0 * at(380.0));
    // the second harmonic
    assert!(at(630.0) > 20.0 * at(500.0));
}

/// The measured frequency is within 0.1 % of the one the throttle gives, at several throttle values.
#[test]
fn the_frequency_is_within_a_tenth_of_a_percent_of_the_throttle_mapping() {
    let sr = 48_000.0;
    for (percent, expected) in [
        (0.0, 110.0),
        (25.0, 212.5),
        (50.0, 315.0),
        (80.0, 438.0),
        (100.0, 520.0),
    ] {
        // a throttle of 0 would be a stopped motor, so the check starts above the off threshold
        let throttle = constant("thr", f64::max(percent, 2.5));
        let expected = if percent == 0.0 {
            110.0 + 0.025 * 410.0
        } else {
            expected
        };
        let mut settings = SoundSettings::default();
        settings.audio.channels = 1;
        let mut engine = Engine::new(settings, throttle_only(&throttle), straight(), 0.0).unwrap();
        let mut samples = vec![0.0; 4 * 48_000];
        engine.render(&mut samples);
        // look for the strongest frequency within 1 % of the expected one, in steps of 0.05 Hz
        let (mut best, mut best_power) = (0.0, 0.0);
        let mut hz = expected * 0.99;
        while hz < expected * 1.01 {
            let power = tone_power(&samples, 1, hz, sr);
            if power > best_power {
                (best, best_power) = (hz, power);
            }
            hz += 0.05;
        }
        let error = (best - expected).abs() / expected;
        assert!(
            error < 0.001,
            "throttle {percent} %: measured {best:.2} Hz, expected {expected:.2} Hz"
        );
    }
}

#[test]
fn the_preview_has_the_requested_length() {
    use flight_replay_core::synth::frames_for;
    assert_eq!(frames_for(10.0, 48_000), 480_000);
    assert_eq!(frames_for(0.5, 44_100), 22_050);
    assert_eq!(frames_for(0.000_01, 48_000), 1);
}

#[test]
fn a_stopped_motor_is_silent_and_the_wind_is_the_only_sound() {
    let throttle = constant("thr", 0.0);
    let mut engine = Engine::new(
        SoundSettings::default(),
        throttle_only(&throttle),
        straight(),
        0.0,
    )
    .unwrap();
    assert!(
        engine.render_s16(48_000).iter().all(|&s| s == 0),
        "no motor and no wind is silence"
    );

    let air = constant("air", 60.0);
    let controls = Controls {
        air_speed: Some(&air),
        ..throttle_only(&throttle)
    };
    let mut engine = Engine::new(SoundSettings::default(), controls, straight(), 0.0).unwrap();
    let samples = engine.render_s16(48_000);
    let rms =
        (samples.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>() / samples.len() as f64).sqrt();
    assert!(
        rms > 500.0,
        "the wind should be audible while gliding, rms {rms}"
    );
}

#[test]
fn an_idle_motor_hums_on_the_ground() {
    let throttle = constant("thr", 0.0);
    let mut settings = SoundSettings::default();
    settings.motor.stopped = Stopped::Idle;
    let mut engine = Engine::new(settings, throttle_only(&throttle), straight(), 0.0).unwrap();
    assert!(engine.render_s16(48_000).iter().any(|&s| s != 0));
}

#[test]
fn the_limiter_keeps_the_output_within_full_scale() {
    let throttle = constant("thr", 100.0);
    let current = constant("cur", 40.0);
    let air = constant("air", 120.0);
    let mut settings = SoundSettings::default();
    settings.audio.master_gain_db = 20.0;
    let controls = Controls {
        current: Some(&current),
        air_speed: Some(&air),
        ..throttle_only(&throttle)
    };
    let mut engine = Engine::new(settings, controls, straight(), 0.0).unwrap();
    let mut samples = vec![0.0; 2 * 48_000];
    engine.render(&mut samples);
    assert!(samples.iter().all(|s| s.abs() <= 1.0));
    let stats = engine.stats();
    assert!(stats.limited > 0 && stats.peak <= 1.0, "{stats:?}");
    assert_eq!(stats.frames, 48_000);
}

#[test]
fn the_seed_changes_the_noise() {
    let throttle = constant("thr", 60.0);
    let render = |seed| {
        let mut settings = SoundSettings::default();
        settings.audio.seed = seed;
        Engine::new(settings, throttle_only(&throttle), straight(), 0.0)
            .unwrap()
            .render_s16(4_800)
    };
    assert_eq!(render(1), render(1));
    assert_ne!(render(1), render(2));
}

#[test]
fn mono_renders_one_channel() {
    let throttle = constant("thr", 60.0);
    let mut settings = SoundSettings::default();
    settings.audio.channels = 1;
    let mut engine = Engine::new(settings, throttle_only(&throttle), straight(), 0.0).unwrap();
    assert_eq!(engine.channels(), 1);
    assert_eq!(engine.render_s16(100).len(), 100);
}

#[test]
fn invalid_settings_are_refused() {
    let throttle = constant("thr", 60.0);
    let mut settings = SoundSettings::default();
    settings.motor.f_max_hz = 20.0;
    assert!(Engine::new(settings, throttle_only(&throttle), straight(), 0.0).is_err());
}
