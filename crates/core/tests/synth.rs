// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Properties of the synthesizer, from its public interface.

mod common;

use std::f64::consts::TAU;

use common::straight;
use flight_replay_core::synth::{AircraftKind, Controls, Engine, SoundSettings, Stopped};
use flight_replay_core::telemetry::Series;

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
        motor_idle: None,
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

// ---- a drone: several motors that never stop while it is armed

/// The sound of a five-inch drone, with the settings of a drone.
fn drone() -> SoundSettings {
    let mut settings = SoundSettings::default();
    AircraftKind::Multirotor.apply(&mut settings);
    settings
}

fn render(
    settings: SoundSettings,
    controls: Controls<'_>,
    seconds: usize,
    block: usize,
) -> Vec<f64> {
    let channels = usize::from(settings.audio.channels);
    let rate = settings.audio.sample_rate as usize;
    let mut engine = Engine::new(settings, controls, straight(), 2.0).unwrap();
    let mut out = vec![0.0; seconds * rate * channels];
    for chunk in out.chunks_mut(block * channels) {
        engine.render(chunk);
    }
    out
}

fn peak(samples: &[f64]) -> f64 {
    samples.iter().fold(0.0, |m, s| f64::max(m, s.abs()))
}

#[test]
fn a_drone_idles_where_inav_shows_idle_and_is_silent_where_it_shows_stop_or_disarmed() {
    let throttle = constant("thr", 0.0);
    let idle = constant("idle", 1.0);
    let stopped = constant("idle", 0.0);

    let spinning = render(
        drone(),
        Controls {
            motor_idle: Some(&idle),
            ..throttle_only(&throttle)
        },
        1,
        4096,
    );
    assert!(
        peak(&spinning) > 0.05,
        "the motors idle: {}",
        peak(&spinning)
    );

    let silent = render(
        drone(),
        Controls {
            motor_idle: Some(&stopped),
            ..throttle_only(&throttle)
        },
        1,
        4096,
    );
    assert!(peak(&silent) < 1e-9, "STOP and DARM are silent");

    // a fixed wing does not idle: for it IDLE is as silent as STOP
    let plane = render(
        SoundSettings::default(),
        Controls {
            motor_idle: Some(&idle),
            ..throttle_only(&throttle)
        },
        1,
        4096,
    );
    assert!(peak(&plane) < 1e-9);
}

#[test]
fn a_drone_sounds_at_a_throttle_a_fixed_wing_would_call_stopped() {
    let throttle = constant("thr", 1.0);
    let plane = render(SoundSettings::default(), throttle_only(&throttle), 1, 4096);
    assert!(peak(&plane) < 1e-9, "a fixed wing stops below 2 %");
    let drone = render(drone(), throttle_only(&throttle), 1, 4096);
    assert!(peak(&drone) > 0.05);
}

/// How much the level changes from one 10 ms window to the next, as a fraction of the mean level.
fn level_variation(samples: &[f64], channels: usize, rate: usize) -> f64 {
    let window = rate / 100 * channels;
    let levels: Vec<f64> = samples
        .chunks_exact(window)
        .skip(20) // the first fifth of a second is the motor settling
        .map(|w| (w.iter().map(|s| s * s).sum::<f64>() / w.len() as f64).sqrt())
        .collect();
    let mean = levels.iter().sum::<f64>() / levels.len() as f64;
    let variance = levels.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / levels.len() as f64;
    variance.sqrt() / mean
}

#[test]
fn the_motors_of_a_drone_beat_against_one_another_and_a_single_motor_does_not() {
    let throttle = constant("thr", 50.0);
    // only the tone: no propeller noise and no wind, which are random
    let tone_only = |mut settings: SoundSettings| {
        settings.prop_noise.gain = 0.0;
        settings.wind.gain = 0.0;
        settings
    };
    let variation = |settings: SoundSettings| {
        let rate = settings.audio.sample_rate as usize;
        let channels = usize::from(settings.audio.channels);
        level_variation(
            &render(tone_only(settings), throttle_only(&throttle), 3, 4096),
            channels,
            rate,
        )
    };

    let beating = variation(drone());
    let mut same_speed = drone();
    same_speed.rotors.detune = 0.0;
    same_speed.rotors.wander = 0.0;
    let steady = variation(same_speed);
    let mut single = drone();
    single.rotors.count = 1;
    let one = variation(single);

    assert!(beating > 0.05, "the motors should beat: {beating:.3}");
    assert!(
        beating > 4.0 * steady,
        "beating {beating:.3} against motors at the same speed {steady:.3}"
    );
    assert!(
        beating > 4.0 * one,
        "beating {beating:.3} against one motor {one:.3}"
    );
}

#[test]
fn with_one_motor_the_settings_of_several_change_nothing() {
    let throttle = constant("thr", 50.0);
    let roll = constant("roll", 10.0);
    let controls = Controls {
        roll: Some(&roll),
        ..throttle_only(&throttle)
    };
    let plain = render(SoundSettings::default(), controls, 1, 4096);
    let mut settings = SoundSettings::default();
    settings.rotors.detune = 0.05;
    settings.rotors.wander = 0.05;
    settings.rotors.attitude = 0.005;
    assert_eq!(plain, render(settings, controls, 1, 4096));
}

#[test]
fn a_drone_renders_the_same_whatever_the_block_size() {
    let times: Vec<f64> = (0..=60).map(f64::from).collect();
    let wave = |name: &str, f: fn(f64) -> f64| {
        let values: Vec<f64> = times.iter().map(|&t| f(t)).collect();
        Series::new(name, &times, &values).unwrap()
    };
    let throttle = wave("thr", |t| 30.0 + 25.0 * (t * 0.9).sin());
    let current = wave("cur", |t| 25.0 + 20.0 * (t * 0.9).sin());
    let roll = wave("roll", |t| 70.0 * (t * 1.3).sin());
    let pitch = wave("pitch", |t| 40.0 * (t * 0.8).sin());
    let idle = constant("idle", 0.0);
    let controls = Controls {
        current: Some(&current),
        roll: Some(&roll),
        pitch: Some(&pitch),
        motor_idle: Some(&idle),
        ..throttle_only(&throttle)
    };
    let whole = render(drone(), controls, 3, 144_000);
    assert!(peak(&whole) > 0.05);
    for block in [1, 512, 777, 8192] {
        assert_eq!(
            whole,
            render(drone(), controls, 3, block),
            "blocks of {block}"
        );
    }
    // and the same seed gives the same sound
    assert_eq!(whole, render(drone(), controls, 3, 4096));
    let mut other_seed = drone();
    other_seed.audio.seed = 2;
    assert_ne!(whole, render(other_seed, controls, 3, 4096));
}

#[test]
fn the_roll_rate_changes_the_speed_of_the_motors_only_when_they_follow_it() {
    let throttle = constant("thr", 50.0);
    let times: Vec<f64> = (0..=60).map(f64::from).collect();
    let values: Vec<f64> = times.iter().map(|&t| 80.0 * (t * 2.0).sin()).collect();
    let rolling = Series::new("roll", &times, &values).unwrap();
    let level = constant("roll", 0.0);
    let controls = |roll| Controls {
        roll: Some(roll),
        ..throttle_only(&throttle)
    };
    // the roll also pans the wind; with no wind, only the motors can tell the two apart
    let quiet_wind = |attitude| {
        let mut settings = drone();
        settings.wind.gain = 0.0;
        settings.rotors.attitude = attitude;
        settings
    };
    assert_eq!(
        render(quiet_wind(0.0), controls(&rolling), 2, 4096),
        render(quiet_wind(0.0), controls(&level), 2, 4096),
        "motors that do not follow the roll sound the same rolling or level"
    );
    assert_ne!(
        render(quiet_wind(0.0005), controls(&rolling), 2, 4096),
        render(quiet_wind(0.0005), controls(&level), 2, 4096),
    );
}

/// The streams of noise are `seed * 8 + n`, which overflowed for a seed above 2^61: a panic in a debug build, and a
/// silent wrap in a release one. The seed is any number a settings file can hold.
#[test]
fn any_seed_is_accepted() {
    let throttle = constant("thr", 50.0);
    for seed in [0, u64::MAX / 8, u64::MAX / 4, u64::MAX] {
        for mut settings in [SoundSettings::default(), drone()] {
            settings.audio.seed = seed;
            let sound = render(settings, throttle_only(&throttle), 1, 4096);
            assert!(peak(&sound) > 0.05, "seed {seed}");
        }
    }
}
