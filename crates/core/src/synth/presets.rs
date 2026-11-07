// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Ready-made sounds for different aircraft, electric motors and propellers.
//!
//! There are two layers. The **kind of aircraft** ([`AircraftKind`]) sets what belongs to the aircraft: whether the
//! motor can stop in flight, how many motors there are, and the current that gives full volume. Choosing the kind
//! also chooses the first preset of the kind. A **preset** changes the *character* of the sound within a kind: the
//! pitch range, how quickly the motor follows the throttle, the harmonics, the whine, the propeller noise and, on a
//! drone, how much the motors differ from one another. It leaves alone everything that belongs to the user's
//! recording: the audio format and the master gain, the telemetry columns, the battery correction, the wind and the
//! G-force effect. So a preset can be chosen at any moment, and every value it sets can still be changed afterwards.
//!
//! A preset is a function that builds a whole [`SoundSettings`] on top of the base of its kind. Only the fields
//! listed in [`copy_character`] are taken from it. To add a preset, write one function and add a row to
//! [`MOTOR_PRESETS`]; the tests check that it is valid, that it is not too loud or too quiet, and that it differs
//! from the others.

use super::settings::{
    AircraftKind, LoudnessSettings, MotorSettings, PropNoiseSettings, RotorSettings,
    SmoothingSettings, SoundSettings, Stopped, ToneSettings,
};

/// A sound for one kind of motor and propeller.
#[derive(Clone, Copy)]
pub struct MotorPreset {
    /// The name shown in the interface.
    pub name: &'static str,
    /// A line about the aircraft and the sound.
    pub description: &'static str,
    /// The kind of aircraft the sound is for. The interface offers the presets of the kind that is chosen.
    pub kind: AircraftKind,
    build: fn() -> SoundSettings,
}

impl MotorPreset {
    /// The whole settings of the preset: the defaults with the character of the preset on top.
    #[must_use]
    pub fn settings(&self) -> SoundSettings {
        (self.build)()
    }

    /// Gives `target` the character of the preset and leaves the rest of it as it is.
    pub fn apply(&self, target: &mut SoundSettings) {
        copy_character(&self.settings(), target);
    }

    /// Whether the character of `settings` is exactly that of the preset.
    #[must_use]
    pub fn is_applied_to(&self, settings: &SoundSettings) -> bool {
        let mut expected = settings.clone();
        self.apply(&mut expected);
        expected == *settings
    }
}

impl std::fmt::Debug for MotorPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MotorPreset")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// The index in [`MOTOR_PRESETS`] of the preset whose character `settings` has, if it has exactly that of one of its
/// kind of aircraft. A sound that was changed by hand after choosing a preset has none.
#[must_use]
pub fn motor_preset_in_use(settings: &SoundSettings) -> Option<usize> {
    MOTOR_PRESETS
        .iter()
        .position(|preset| preset.kind == settings.aircraft && preset.is_applied_to(settings))
}

impl AircraftKind {
    /// The presets of the kind, with their place in [`MOTOR_PRESETS`], in the order they are offered. The first is
    /// the sound the kind starts with.
    pub fn presets(self) -> impl Iterator<Item = (usize, &'static MotorPreset)> {
        MOTOR_PRESETS
            .iter()
            .enumerate()
            .filter(move |(_, preset)| preset.kind == self)
    }

    /// The settings with what belongs to the kind of aircraft and the default character.
    #[must_use]
    pub fn base(self) -> SoundSettings {
        match self {
            Self::FixedWing => SoundSettings::default(),
            Self::Multirotor => multirotor_base(),
        }
    }

    /// Makes `target` the sound of this kind of aircraft: what belongs to the kind, and the character of its first
    /// preset. The recording's own settings (the audio format and the master gain, the columns, the battery
    /// correction, the wind and the G-force effect) stay as they are.
    pub fn apply(self, target: &mut SoundSettings) {
        copy_aircraft(&self.base(), target);
        if let Some((_, first)) = self.presets().next() {
            first.apply(target);
        }
    }
}

/// What belongs to the kind of aircraft, and not to the sound of a motor or to the recording.
fn copy_aircraft(from: &SoundSettings, to: &mut SoundSettings) {
    to.aircraft = from.aircraft;
    to.rotors.count = from.rotors.count;
    to.motor.off_threshold_pct = from.motor.off_threshold_pct;
    to.motor.stopped = from.motor.stopped;
    to.loudness = from.loudness.clone();
}

/// What makes the character of a sound. The rest of the settings stays as the user has it.
fn copy_character(from: &SoundSettings, to: &mut SoundSettings) {
    to.motor.f_idle_hz = from.motor.f_idle_hz;
    to.motor.f_max_hz = from.motor.f_max_hz;
    to.motor.throttle_curve = from.motor.throttle_curve;
    to.smoothing.throttle_attack_s = from.smoothing.throttle_attack_s;
    to.smoothing.throttle_release_s = from.smoothing.throttle_release_s;
    to.smoothing.current_attack_s = from.smoothing.current_attack_s;
    to.smoothing.current_release_s = from.smoothing.current_release_s;
    to.tone = from.tone.clone();
    to.prop_noise = from.prop_noise.clone();
    // how many motors there are belongs to the kind of aircraft; how much they differ is character
    to.rotors.detune = from.rotors.detune;
    to.rotors.wander = from.rotors.wander;
    to.rotors.attitude = from.rotors.attitude;
}

/// The presets, in the order they are offered. The first of each kind is the sound the kind starts with, and the
/// first of all is the sound the program starts with.
pub const MOTOR_PRESETS: &[MotorPreset] = &[
    MotorPreset {
        name: "Default",
        description: "The sound the program starts with: a two-blade propeller, with a warm mid-range tone.",
        kind: AircraftKind::FixedWing,
        build: SoundSettings::default,
    },
    MotorPreset {
        name: "Small park flyer",
        description: "A small two-blade propeller turning fast: a high, thin tone with some whine, and a quick response.",
        kind: AircraftKind::FixedWing,
        build: small_park_flyer,
    },
    MotorPreset {
        name: "Small high-KV motor",
        description: "A tiny, high-KV motor spinning a small propeller very fast: a piercing, thin tone with a loud whine, and almost no lag.",
        kind: AircraftKind::FixedWing,
        build: small_high_kv,
    },
    MotorPreset {
        name: "Sport plane, three blades",
        description: "A three-blade propeller: a higher blade pass and a smoother, rounder sound with broader air noise.",
        kind: AircraftKind::FixedWing,
        build: three_blade_sport,
    },
    MotorPreset {
        name: "Aerobatic power",
        description: "A big motor on a short propeller: a hard, bright and buzzy sound that answers the throttle at once.",
        kind: AircraftKind::FixedWing,
        build: aerobatic_power,
    },
    MotorPreset {
        name: "Large slow propeller",
        description: "A long-range plane or motor glider: a deep, rich tone that chops the air, and a motor that is slow to follow.",
        kind: AircraftKind::FixedWing,
        build: large_slow_propeller,
    },
    MotorPreset {
        name: "Pusher behind the wing",
        description: "A pusher propeller behind the wing or the fuselage: the same motor, but dull and muffled.",
        kind: AircraftKind::FixedWing,
        build: pusher_muffled,
    },
    MotorPreset {
        name: "Ducted fan",
        description: "An electric ducted fan: a high whine on top of a hiss, with almost no beat of the blades.",
        kind: AircraftKind::FixedWing,
        build: ducted_fan,
    },
    MotorPreset {
        name: "5-inch freestyle",
        description: "A five-inch drone with three-blade propellers: four motors that buzz against one another, from a rough idle to a high scream, and answer every stick movement.",
        kind: AircraftKind::Multirotor,
        build: freestyle_5_inch,
    },
    MotorPreset {
        name: "7-inch long range",
        description: "A seven-inch drone with big propellers turning slower: lower and rounder than a five-inch, with a smoother, slower response.",
        kind: AircraftKind::Multirotor,
        build: long_range_7_inch,
    },
    MotorPreset {
        name: "Cinewhoop, ducted",
        description: "A small drone with ducted propellers: the same buzz, but muffled by the ducts, with more air noise and less whine.",
        kind: AircraftKind::Multirotor,
        build: cinewhoop_ducted,
    },
];

/// What a drone has that a fixed wing does not: several motors that never stop while it is armed, so the motor is
/// silent only where INAV shows `STOP` or `DARM`; a current that is larger than a plane's (a five-inch draws 15 to
/// 25 A hovering and 80 to 100 A at full throttle); and a pitch angle that tells nothing about the load, because it
/// is the lean of the drone and not its climb.
fn multirotor_base() -> SoundSettings {
    SoundSettings {
        aircraft: AircraftKind::Multirotor,
        motor: MotorSettings {
            off_threshold_pct: 0.0,
            stopped: Stopped::Spinning,
            ..MotorSettings::default()
        },
        rotors: RotorSettings {
            count: 4,
            ..RotorSettings::default()
        },
        loudness: LoudnessSettings {
            current_base_a: 3.0,
            current_full_a: 60.0,
            pitch_gain: 0.0,
            ..LoudnessSettings::default()
        },
        ..SoundSettings::default()
    }
}

fn small_park_flyer() -> SoundSettings {
    SoundSettings {
        motor: MotorSettings {
            f_idle_hz: 170.0,
            f_max_hz: 760.0,
            ..MotorSettings::default()
        },
        smoothing: SmoothingSettings {
            throttle_attack_s: 0.08,
            throttle_release_s: 0.22,
            current_attack_s: 0.07,
            current_release_s: 0.20,
            ..SmoothingSettings::default()
        },
        tone: ToneSettings {
            gain: 0.56,
            harmonics: vec![1.0, 0.45, 0.22, 0.10, 0.05],
            brightness: 0.60,
            whine_ratio: 4.5,
            whine_gain: 0.10,
            stereo_width: 0.40,
        },
        prop_noise: PropNoiseSettings {
            gain: 0.80,
            center_low_hz: 1500.0,
            center_high_hz: 4200.0,
            q: 1.2,
            blade_mod_depth: 0.25,
        },
        ..SoundSettings::default()
    }
}

/// A small brushless motor of high KV on a small propeller, as on a foam or micro plane: it turns at 20,000 rpm and
/// more, so the blade pass is the highest of the presets, the rotor is so light that it follows the throttle almost
/// at once, and the whine of the motor and the hiss of the small propeller sit high in the spectrum.
fn small_high_kv() -> SoundSettings {
    SoundSettings {
        motor: MotorSettings {
            f_idle_hz: 260.0,
            f_max_hz: 950.0,
            ..MotorSettings::default()
        },
        smoothing: SmoothingSettings {
            throttle_attack_s: 0.05,
            throttle_release_s: 0.14,
            current_attack_s: 0.05,
            current_release_s: 0.14,
            ..SmoothingSettings::default()
        },
        tone: ToneSettings {
            gain: 0.57,
            harmonics: vec![1.0, 0.35, 0.14, 0.06, 0.03],
            brightness: 0.75,
            whine_ratio: 6.0,
            whine_gain: 0.18,
            stereo_width: 0.35,
        },
        prop_noise: PropNoiseSettings {
            gain: 0.95,
            center_low_hz: 2400.0,
            center_high_hz: 6000.0,
            q: 1.4,
            blade_mod_depth: 0.12,
        },
        ..SoundSettings::default()
    }
}

fn three_blade_sport() -> SoundSettings {
    SoundSettings {
        motor: MotorSettings {
            f_idle_hz: 150.0,
            f_max_hz: 680.0,
            ..MotorSettings::default()
        },
        smoothing: SmoothingSettings {
            throttle_attack_s: 0.17,
            throttle_release_s: 0.45,
            ..SmoothingSettings::default()
        },
        tone: ToneSettings {
            gain: 0.55,
            harmonics: vec![1.0, 0.22, 0.30, 0.07, 0.05, 0.02],
            brightness: 0.25,
            whine_ratio: 3.5,
            whine_gain: 0.04,
            stereo_width: 0.45,
        },
        prop_noise: PropNoiseSettings {
            gain: 0.95,
            center_low_hz: 1000.0,
            center_high_hz: 3000.0,
            q: 0.8,
            blade_mod_depth: 0.15,
        },
        ..SoundSettings::default()
    }
}

fn aerobatic_power() -> SoundSettings {
    SoundSettings {
        motor: MotorSettings {
            f_idle_hz: 130.0,
            f_max_hz: 640.0,
            throttle_curve: 1.2,
            ..MotorSettings::default()
        },
        smoothing: SmoothingSettings {
            throttle_attack_s: 0.06,
            throttle_release_s: 0.18,
            current_attack_s: 0.05,
            current_release_s: 0.20,
            ..SmoothingSettings::default()
        },
        tone: ToneSettings {
            gain: 0.42,
            harmonics: vec![1.0, 0.88, 0.74, 0.62, 0.50, 0.38, 0.27, 0.18],
            brightness: 0.90,
            whine_ratio: 3.5,
            whine_gain: 0.14,
            stereo_width: 0.55,
        },
        prop_noise: PropNoiseSettings {
            gain: 1.10,
            center_low_hz: 1400.0,
            center_high_hz: 4200.0,
            q: 1.1,
            blade_mod_depth: 0.35,
        },
        ..SoundSettings::default()
    }
}

fn large_slow_propeller() -> SoundSettings {
    SoundSettings {
        motor: MotorSettings {
            f_idle_hz: 55.0,
            f_max_hz: 260.0,
            ..MotorSettings::default()
        },
        smoothing: SmoothingSettings {
            throttle_attack_s: 0.35,
            throttle_release_s: 0.90,
            current_attack_s: 0.25,
            current_release_s: 0.70,
            ..SmoothingSettings::default()
        },
        tone: ToneSettings {
            gain: 0.50,
            harmonics: vec![1.0, 0.75, 0.50, 0.32, 0.20, 0.12, 0.07, 0.04],
            brightness: 0.35,
            whine_ratio: 6.0,
            whine_gain: 0.02,
            stereo_width: 0.50,
        },
        prop_noise: PropNoiseSettings {
            gain: 0.90,
            center_low_hz: 300.0,
            center_high_hz: 1500.0,
            q: 0.8,
            blade_mod_depth: 0.45,
        },
        ..SoundSettings::default()
    }
}

fn pusher_muffled() -> SoundSettings {
    SoundSettings {
        motor: MotorSettings {
            f_idle_hz: 100.0,
            f_max_hz: 460.0,
            ..MotorSettings::default()
        },
        smoothing: SmoothingSettings {
            throttle_attack_s: 0.18,
            throttle_release_s: 0.45,
            ..SmoothingSettings::default()
        },
        tone: ToneSettings {
            gain: 0.60,
            harmonics: vec![1.0, 0.50, 0.22, 0.08, 0.03],
            brightness: 0.20,
            whine_ratio: 3.5,
            whine_gain: 0.02,
            stereo_width: 0.25,
        },
        prop_noise: PropNoiseSettings {
            gain: 0.55,
            center_low_hz: 600.0,
            center_high_hz: 1600.0,
            q: 0.7,
            blade_mod_depth: 0.20,
        },
        ..SoundSettings::default()
    }
}

fn ducted_fan() -> SoundSettings {
    SoundSettings {
        motor: MotorSettings {
            f_idle_hz: 280.0,
            f_max_hz: 900.0,
            throttle_curve: 0.8,
            ..MotorSettings::default()
        },
        smoothing: SmoothingSettings {
            throttle_attack_s: 0.12,
            throttle_release_s: 0.50,
            ..SmoothingSettings::default()
        },
        tone: ToneSettings {
            gain: 0.40,
            harmonics: vec![1.0, 0.30, 0.12, 0.05],
            brightness: 0.70,
            whine_ratio: 5.0,
            whine_gain: 0.22,
            stereo_width: 0.30,
        },
        prop_noise: PropNoiseSettings {
            gain: 1.30,
            center_low_hz: 2200.0,
            center_high_hz: 6500.0,
            q: 0.9,
            blade_mod_depth: 0.08,
        },
        ..SoundSettings::default()
    }
}

/// A five-inch drone, three-blade propellers at 5,000 rpm at idle and 38,000 at full throttle: the blade pass is 250
/// to 1,900 Hz. The whine of a 14-pole motor is seven times its turning speed, which is 7 / 3 of the blade pass.
/// Light propellers and a controller that never stops correcting make the response quick.
fn freestyle_5_inch() -> SoundSettings {
    let mut s = multirotor_base();
    s.motor.f_idle_hz = 250.0;
    s.motor.f_max_hz = 1900.0;
    s.smoothing = SmoothingSettings {
        throttle_attack_s: 0.05,
        throttle_release_s: 0.10,
        current_attack_s: 0.06,
        current_release_s: 0.15,
        ..SmoothingSettings::default()
    };
    s.tone = ToneSettings {
        gain: 0.44,
        harmonics: vec![1.0, 0.60, 0.42, 0.30, 0.20, 0.13, 0.08],
        brightness: 0.55,
        whine_ratio: 2.3,
        whine_gain: 0.14,
        stereo_width: 0.55,
    };
    s.prop_noise = PropNoiseSettings {
        gain: 1.00,
        center_low_hz: 1800.0,
        center_high_hz: 6000.0,
        q: 0.9,
        blade_mod_depth: 0.15,
    };
    s.rotors.detune = 0.012;
    s.rotors.wander = 0.008;
    s.rotors.attitude = 0.000_15;
    s
}

/// A seven-inch drone: larger propellers turn slower for the same thrust (about 3,000 rpm at idle and 20,000 at full
/// throttle), so the blade pass is lower, and the response is a little slower.
fn long_range_7_inch() -> SoundSettings {
    let mut s = multirotor_base();
    s.motor.f_idle_hz = 160.0;
    s.motor.f_max_hz = 1000.0;
    s.smoothing = SmoothingSettings {
        throttle_attack_s: 0.09,
        throttle_release_s: 0.20,
        current_attack_s: 0.09,
        current_release_s: 0.22,
        ..SmoothingSettings::default()
    };
    s.tone = ToneSettings {
        gain: 0.46,
        harmonics: vec![1.0, 0.65, 0.42, 0.26, 0.15, 0.08],
        brightness: 0.45,
        whine_ratio: 2.3,
        whine_gain: 0.06,
        stereo_width: 0.50,
    };
    s.prop_noise = PropNoiseSettings {
        gain: 0.90,
        center_low_hz: 900.0,
        center_high_hz: 3500.0,
        q: 0.9,
        blade_mod_depth: 0.20,
    };
    s.rotors.detune = 0.010;
    s.rotors.wander = 0.010;
    s.rotors.attitude = 0.000_10;
    s
}

/// A three-inch drone with ducts around the propellers: small propellers turn fast, and the ducts take the edge off
/// the upper harmonics and the whine while the air noise stays.
fn cinewhoop_ducted() -> SoundSettings {
    let mut s = multirotor_base();
    s.motor.f_idle_hz = 380.0;
    s.motor.f_max_hz = 1700.0;
    s.smoothing = SmoothingSettings {
        throttle_attack_s: 0.06,
        throttle_release_s: 0.12,
        current_attack_s: 0.07,
        current_release_s: 0.18,
        ..SmoothingSettings::default()
    };
    s.tone = ToneSettings {
        gain: 0.56,
        harmonics: vec![1.0, 0.25, 0.08, 0.03],
        brightness: 0.30,
        whine_ratio: 2.3,
        whine_gain: 0.03,
        stereo_width: 0.45,
    };
    s.prop_noise = PropNoiseSettings {
        gain: 1.10,
        center_low_hz: 900.0,
        center_high_hz: 2600.0,
        q: 0.8,
        blade_mod_depth: 0.10,
    };
    s.rotors.detune = 0.010;
    s.rotors.wander = 0.008;
    s.rotors.attitude = 0.000_12;
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_preset_is_the_default_sound() {
        assert_eq!(MOTOR_PRESETS[0].settings(), SoundSettings::default());
        assert_eq!(motor_preset_in_use(&SoundSettings::default()), Some(0));
    }

    #[test]
    fn the_names_are_not_repeated() {
        for (i, a) in MOTOR_PRESETS.iter().enumerate() {
            assert!(
                MOTOR_PRESETS.iter().skip(i + 1).all(|b| b.name != a.name),
                "{} twice",
                a.name
            );
            assert!(!a.description.is_empty(), "{} has no description", a.name);
        }
    }

    #[test]
    fn every_preset_is_valid_at_both_sample_rates_of_the_interface() {
        for preset in MOTOR_PRESETS {
            for rate in [44_100, 48_000] {
                let mut settings = preset.settings();
                settings.audio.sample_rate = rate;
                assert_eq!(settings.validate(), Ok(()), "{} at {rate}", preset.name);
            }
        }
    }

    #[test]
    fn applying_a_preset_keeps_what_belongs_to_the_user() {
        let mut settings = SoundSettings::default();
        settings.audio.sample_rate = 44_100;
        settings.audio.channels = 1;
        settings.audio.master_gain_db = -3.0;
        settings.columns.current = "my_current".into();
        settings.loudness.current_full_a = 40.0;
        settings.motor.battery_gain = 1.5;
        settings.wind.gain = 0.7;
        let before = settings.clone();

        for preset in MOTOR_PRESETS {
            let mut applied = before.clone();
            preset.apply(&mut applied);
            assert_eq!(applied.audio, before.audio, "{}", preset.name);
            assert_eq!(applied.columns, before.columns, "{}", preset.name);
            assert_eq!(applied.loudness, before.loudness, "{}", preset.name);
            assert_eq!(applied.wind, before.wind, "{}", preset.name);
            assert_eq!(applied.gforce, before.gforce, "{}", preset.name);
            assert_eq!(applied.motor.battery_gain, 1.5, "{}", preset.name);
            assert_eq!(
                applied.motor.off_threshold_pct, before.motor.off_threshold_pct,
                "{}",
                preset.name
            );
            assert_eq!(applied.motor.stopped, before.motor.stopped);
        }
    }

    #[test]
    fn a_preset_is_recognised_until_it_is_changed_by_hand() {
        for (index, preset) in MOTOR_PRESETS.iter().enumerate() {
            let mut settings = SoundSettings::default();
            settings.audio.sample_rate = 44_100;
            preset.kind.apply(&mut settings);
            preset.apply(&mut settings);
            assert_eq!(
                motor_preset_in_use(&settings),
                Some(index),
                "{}",
                preset.name
            );

            settings.motor.f_max_hz += 1.0;
            assert_eq!(
                motor_preset_in_use(&settings),
                None,
                "{} after a change",
                preset.name
            );
        }
    }

    #[test]
    fn the_presets_differ_from_one_another() {
        for (i, a) in MOTOR_PRESETS.iter().enumerate() {
            for b in &MOTOR_PRESETS[i + 1..] {
                assert_ne!(a.settings(), b.settings(), "{} and {}", a.name, b.name);
            }
        }
    }
    #[test]
    fn every_kind_has_presets_and_every_preset_belongs_to_its_kind() {
        for kind in AircraftKind::ALL {
            assert!(kind.presets().next().is_some(), "{kind} has no preset");
            for (index, preset) in kind.presets() {
                assert_eq!(MOTOR_PRESETS[index].name, preset.name);
                assert_eq!(preset.settings().aircraft, kind, "{}", preset.name);
                assert_eq!(
                    preset.settings().rotors.count,
                    kind.base().rotors.count,
                    "{}",
                    preset.name
                );
            }
        }
        // the program starts with the first fixed-wing sound, which is the default
        assert_eq!(MOTOR_PRESETS[0].kind, AircraftKind::FixedWing);
    }

    #[test]
    fn choosing_a_drone_sets_what_belongs_to_a_drone_and_its_first_preset() {
        let mut settings = SoundSettings::default();
        AircraftKind::Multirotor.apply(&mut settings);
        assert_eq!(settings.aircraft, AircraftKind::Multirotor);
        assert_eq!(settings.rotors.count, 4);
        assert_eq!(settings.motor.stopped, Stopped::Spinning);
        assert_eq!(settings.motor.off_threshold_pct, 0.0);
        assert!(settings.loudness.current_full_a > 40.0);
        let (first, _) = AircraftKind::Multirotor.presets().next().unwrap();
        assert_eq!(motor_preset_in_use(&settings), Some(first));
        assert_eq!(settings.validate(), Ok(()));
    }

    #[test]
    fn going_back_to_a_fixed_wing_gives_the_default_sound() {
        let mut settings = SoundSettings::default();
        AircraftKind::Multirotor.apply(&mut settings);
        AircraftKind::FixedWing.apply(&mut settings);
        assert_eq!(settings, SoundSettings::default());
    }

    #[test]
    fn choosing_a_kind_keeps_what_belongs_to_the_recording() {
        let mut settings = SoundSettings::default();
        settings.audio.sample_rate = 44_100;
        settings.audio.channels = 1;
        settings.audio.master_gain_db = -3.0;
        settings.columns.current = "my_current".into();
        settings.motor.battery_gain = 1.5;
        settings.wind.gain = 0.7;
        settings.gforce.gain = 0.4;
        let before = settings.clone();
        for kind in AircraftKind::ALL {
            let mut applied = before.clone();
            kind.apply(&mut applied);
            assert_eq!(applied.audio, before.audio, "{kind}");
            assert_eq!(applied.columns, before.columns, "{kind}");
            assert_eq!(applied.wind, before.wind, "{kind}");
            assert_eq!(applied.gforce, before.gforce, "{kind}");
            assert_eq!(applied.motor.battery_gain, 1.5, "{kind}");
        }
    }

    #[test]
    fn a_fixed_wing_preset_is_not_in_use_on_a_drone() {
        let mut settings = SoundSettings::default();
        AircraftKind::Multirotor.apply(&mut settings);
        // the character of the default sound on a drone is nobody's preset
        MOTOR_PRESETS[0].apply(&mut settings);
        assert_eq!(motor_preset_in_use(&settings), None);
    }
}
