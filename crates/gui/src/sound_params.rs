// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The numeric parameters of the sound, as a table: each has a group, a label, a range and a step. The sound step
//! builds its sliders from this table, so adding a parameter needs no new interface code.

use flight_replay_core::synth::{AircraftKind, MAX_ROTORS, SoundSettings};

/// A numeric parameter of the sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Param {
    MasterGainDb,
    FIdleHz,
    FMaxHz,
    Rotors,
    RotorDetune,
    RotorWander,
    RotorAttitude,
    ThrottleCurve,
    OffThresholdPct,
    BatteryGain,
    ThrottleAttack,
    ThrottleRelease,
    CurrentAttack,
    CurrentRelease,
    SpeedAttack,
    SpeedRelease,
    ToneGain,
    Brightness,
    WhineGain,
    WhineRatio,
    StereoWidth,
    CurrentBase,
    CurrentFull,
    Floor,
    PropGain,
    PropLow,
    PropHigh,
    PropQ,
    BladeDepth,
    WindGain,
    WindRef,
    RollBalance,
    GForceGain,
}

/// The sections of the sound step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Level,
    Motor,
    Rotors,
    Inertia,
    Tone,
    Loudness,
    Propeller,
    Wind,
}

impl Group {
    pub const ALL: [Group; 8] = [
        Self::Level,
        Self::Motor,
        Self::Rotors,
        Self::Inertia,
        Self::Tone,
        Self::Loudness,
        Self::Propeller,
        Self::Wind,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Level => "Level",
            Self::Motor => "Motor",
            Self::Rotors => "Several motors",
            Self::Inertia => "Inertia (seconds)",
            Self::Tone => "Tone",
            Self::Loudness => "Volume from the current",
            Self::Propeller => "Propeller noise",
            Self::Wind => "Wind",
        }
    }
}

impl Group {
    /// Whether the section is shown for this kind of aircraft: a fixed wing has one motor.
    pub fn applies_to(self, kind: AircraftKind) -> bool {
        self != Self::Rotors || kind == AircraftKind::Multirotor
    }
}

/// What the interface needs to know about a parameter.
pub struct Spec {
    pub param: Param,
    pub group: Group,
    pub label: &'static str,
    pub unit: &'static str,
    pub min: f64,
    pub max: f64,
    /// The range of the slider on a drone, where it is not the same: a drone turns its propellers much faster and
    /// draws much more current than a plane.
    pub drone_range: Option<(f64, f64)>,
    pub step: f64,
    pub help: &'static str,
}

impl Spec {
    /// The range of the slider for this kind of aircraft.
    pub fn range(&self, kind: AircraftKind) -> (f64, f64) {
        match (kind, self.drone_range) {
            (AircraftKind::Multirotor, Some(range)) => range,
            _ => (self.min, self.max),
        }
    }

    /// The same parameter with another range of the slider on a drone.
    const fn on_a_drone(mut self, min: f64, max: f64) -> Self {
        self.drone_range = Some((min, max));
        self
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "a row of the table of parameters: every column is one argument"
)]
const fn spec(
    param: Param,
    group: Group,
    label: &'static str,
    unit: &'static str,
    min: f64,
    max: f64,
    step: f64,
    help: &'static str,
) -> Spec {
    Spec {
        param,
        group,
        label,
        unit,
        min,
        max,
        drone_range: None,
        step,
        help,
    }
}

/// Every parameter, in the order it is shown.
pub const SPECS: &[Spec] = &[
    spec(
        Param::MasterGainDb,
        Group::Level,
        "Master gain",
        "dB",
        -30.0,
        12.0,
        0.5,
        "Final gain before the soft limiter.",
    ),
    spec(
        Param::FIdleHz,
        Group::Motor,
        "Pitch at 0 % throttle",
        "Hz",
        40.0,
        400.0,
        1.0,
        "Fundamental frequency (blade pass) with the throttle at zero.",
    )
    .on_a_drone(100.0, 800.0),
    spec(
        Param::FMaxHz,
        Group::Motor,
        "Pitch at 100 % throttle",
        "Hz",
        200.0,
        1000.0,
        1.0,
        "Fundamental frequency with the throttle at the maximum.",
    )
    .on_a_drone(400.0, 3000.0),
    spec(
        Param::Rotors,
        Group::Rotors,
        "Number of motors",
        "",
        1.0,
        MAX_ROTORS as f64,
        1.0,
        "How many motors the drone has. Each one sounds on its own, at a slightly different speed.",
    ),
    spec(
        Param::RotorDetune,
        Group::Rotors,
        "Speed difference",
        "%",
        0.0,
        10.0,
        0.1,
        "A fixed difference of speed between the motors. The larger it is, the faster they beat against one another.",
    ),
    spec(
        Param::RotorWander,
        Group::Rotors,
        "Speed drift",
        "%",
        0.0,
        10.0,
        0.1,
        "How far the speed of each motor drifts, slowly and at random, as the controller corrects the drone.",
    ),
    spec(
        Param::RotorAttitude,
        Group::Rotors,
        "Follows roll and pitch",
        "",
        0.0,
        5.0,
        0.05,
        "How much the motors on one side speed up and the others slow down when the drone rolls or pitches, in percent of their speed for each 100 degrees per second.",
    ),
    spec(
        Param::ThrottleCurve,
        Group::Motor,
        "Throttle curve",
        "",
        0.3,
        2.0,
        0.05,
        "1 is linear. Below 1 raises the low end faster.",
    ),
    spec(
        Param::OffThresholdPct,
        Group::Motor,
        "Motor off below",
        "%",
        0.0,
        20.0,
        0.5,
        "A throttle at or below this means the motor is stopped (gliding).",
    ),
    spec(
        Param::BatteryGain,
        Group::Motor,
        "Battery effect on pitch",
        "",
        0.0,
        2.0,
        0.05,
        "How much a low cell voltage lowers the pitch. 0 is off.",
    ),
    spec(
        Param::ThrottleAttack,
        Group::Inertia,
        "Pitch rising",
        "s",
        0.0,
        1.0,
        0.01,
        "How fast the pitch follows the throttle going up.",
    ),
    spec(
        Param::ThrottleRelease,
        Group::Inertia,
        "Pitch falling",
        "s",
        0.0,
        2.0,
        0.01,
        "How fast the pitch follows the throttle going down.",
    ),
    spec(
        Param::CurrentAttack,
        Group::Inertia,
        "Volume rising",
        "s",
        0.0,
        1.0,
        0.01,
        "How fast the volume follows the current going up.",
    ),
    spec(
        Param::CurrentRelease,
        Group::Inertia,
        "Volume falling",
        "s",
        0.0,
        2.0,
        0.01,
        "How fast the volume follows the current going down.",
    ),
    spec(
        Param::SpeedAttack,
        Group::Inertia,
        "Wind rising",
        "s",
        0.0,
        3.0,
        0.05,
        "How fast the wind follows the speed going up.",
    ),
    spec(
        Param::SpeedRelease,
        Group::Inertia,
        "Wind falling",
        "s",
        0.0,
        4.0,
        0.05,
        "How fast the wind follows the speed going down.",
    ),
    spec(
        Param::ToneGain,
        Group::Tone,
        "Tone volume",
        "",
        0.0,
        1.5,
        0.01,
        "Volume of the tonal part (motor and propeller).",
    ),
    spec(
        Param::Brightness,
        Group::Tone,
        "Brightness",
        "",
        0.0,
        1.0,
        0.01,
        "0 to 1: the upper harmonics grow with the load.",
    ),
    spec(
        Param::WhineGain,
        Group::Tone,
        "Motor whine",
        "",
        0.0,
        0.3,
        0.005,
        "Volume of the high whine that is not a harmonic.",
    ),
    spec(
        Param::WhineRatio,
        Group::Tone,
        "Whine pitch",
        "×",
        1.5,
        8.0,
        0.1,
        "The pitch of the whine, as a multiple of the pitch of the propeller. It is not a harmonic.",
    ),
    spec(
        Param::StereoWidth,
        Group::Tone,
        "Stereo width",
        "",
        0.0,
        1.0,
        0.01,
        "0 puts everything in the centre. 1 spreads the harmonics wide.",
    ),
    spec(
        Param::CurrentBase,
        Group::Loudness,
        "Base draw",
        "A",
        0.0,
        10.0,
        0.1,
        "Current with the motor off (video link, servos). It does not count as load.",
    )
    .on_a_drone(0.0, 20.0),
    spec(
        Param::CurrentFull,
        Group::Loudness,
        "Current for full volume",
        "A",
        4.0,
        80.0,
        0.5,
        "The current that gives the full volume.",
    )
    .on_a_drone(10.0, 250.0),
    spec(
        Param::Floor,
        Group::Loudness,
        "Minimum volume",
        "",
        0.0,
        1.0,
        0.01,
        "The quietest the motor gets while it spins.",
    ),
    spec(
        Param::PropGain,
        Group::Propeller,
        "Noise volume",
        "",
        0.0,
        2.0,
        0.01,
        "Volume of the propeller noise.",
    ),
    spec(
        Param::PropLow,
        Group::Propeller,
        "Band at 0 % throttle",
        "Hz",
        300.0,
        3000.0,
        10.0,
        "Centre of the noise band with the throttle at zero.",
    )
    .on_a_drone(300.0, 6000.0),
    spec(
        Param::PropHigh,
        Group::Propeller,
        "Band at 100 % throttle",
        "Hz",
        600.0,
        8000.0,
        10.0,
        "Centre of the noise band with the throttle at the maximum.",
    )
    .on_a_drone(600.0, 12000.0),
    spec(
        Param::PropQ,
        Group::Propeller,
        "Band sharpness",
        "",
        0.3,
        4.0,
        0.05,
        "How narrow the noise band is. A higher value is a thinner, more whistling noise.",
    ),
    spec(
        Param::BladeDepth,
        Group::Propeller,
        "Blade modulation",
        "",
        0.0,
        1.0,
        0.01,
        "Depth of the noise modulation at the blade-pass frequency.",
    ),
    spec(
        Param::WindGain,
        Group::Wind,
        "Wind volume",
        "",
        0.0,
        4.0,
        0.05,
        "Volume of the wind.",
    ),
    spec(
        Param::WindRef,
        Group::Wind,
        "Speed for full wind",
        "km/h",
        10.0,
        200.0,
        1.0,
        "The speed that gives the full wind.",
    ),
    spec(
        Param::RollBalance,
        Group::Wind,
        "Pan with roll",
        "",
        0.0,
        1.0,
        0.01,
        "Banking pans the wind between left and right.",
    ),
    spec(
        Param::GForceGain,
        Group::Wind,
        "G-force effect",
        "",
        0.0,
        2.0,
        0.05,
        "Above 0, G-force raises the noise and the wind (needs the G-force on the OSD).",
    ),
];

/// The settings of the motors are fractions, and the sliders show them as percentages: of the speed, and of the speed
/// for each 100 degrees per second of roll or pitch.
const PERCENT: f64 = 100.0;
const PERCENT_PER_100_DEGREES_PER_SECOND: f64 = PERCENT * 100.0;

/// The value of a parameter.
pub fn get(settings: &SoundSettings, param: Param) -> f64 {
    match param {
        Param::MasterGainDb => settings.audio.master_gain_db,
        Param::FIdleHz => settings.motor.f_idle_hz,
        Param::FMaxHz => settings.motor.f_max_hz,
        Param::Rotors => f64::from(settings.rotors.count),
        Param::RotorDetune => settings.rotors.detune * PERCENT,
        Param::RotorWander => settings.rotors.wander * PERCENT,
        Param::RotorAttitude => settings.rotors.attitude * PERCENT_PER_100_DEGREES_PER_SECOND,
        Param::ThrottleCurve => settings.motor.throttle_curve,
        Param::OffThresholdPct => settings.motor.off_threshold_pct,
        Param::BatteryGain => settings.motor.battery_gain,
        Param::ThrottleAttack => settings.smoothing.throttle_attack_s,
        Param::ThrottleRelease => settings.smoothing.throttle_release_s,
        Param::CurrentAttack => settings.smoothing.current_attack_s,
        Param::CurrentRelease => settings.smoothing.current_release_s,
        Param::SpeedAttack => settings.smoothing.speed_attack_s,
        Param::SpeedRelease => settings.smoothing.speed_release_s,
        Param::ToneGain => settings.tone.gain,
        Param::Brightness => settings.tone.brightness,
        Param::WhineGain => settings.tone.whine_gain,
        Param::WhineRatio => settings.tone.whine_ratio,
        Param::StereoWidth => settings.tone.stereo_width,
        Param::CurrentBase => settings.loudness.current_base_a,
        Param::CurrentFull => settings.loudness.current_full_a,
        Param::Floor => settings.loudness.floor,
        Param::PropGain => settings.prop_noise.gain,
        Param::PropLow => settings.prop_noise.center_low_hz,
        Param::PropHigh => settings.prop_noise.center_high_hz,
        Param::PropQ => settings.prop_noise.q,
        Param::BladeDepth => settings.prop_noise.blade_mod_depth,
        Param::WindGain => settings.wind.gain,
        Param::WindRef => settings.wind.speed_ref_kmh,
        Param::RollBalance => settings.wind.roll_balance,
        Param::GForceGain => settings.gforce.gain,
    }
}

/// Sets a parameter.
pub fn set(settings: &mut SoundSettings, param: Param, value: f64) {
    match param {
        Param::MasterGainDb => settings.audio.master_gain_db = value,
        Param::FIdleHz => settings.motor.f_idle_hz = value,
        Param::FMaxHz => settings.motor.f_max_hz = value,
        Param::Rotors => {
            settings.rotors.count = value.round().clamp(1.0, MAX_ROTORS as f64) as u8;
        }
        Param::RotorDetune => settings.rotors.detune = value / PERCENT,
        Param::RotorWander => settings.rotors.wander = value / PERCENT,
        Param::RotorAttitude => {
            settings.rotors.attitude = value / PERCENT_PER_100_DEGREES_PER_SECOND;
        }
        Param::ThrottleCurve => settings.motor.throttle_curve = value,
        Param::OffThresholdPct => settings.motor.off_threshold_pct = value,
        Param::BatteryGain => settings.motor.battery_gain = value,
        Param::ThrottleAttack => settings.smoothing.throttle_attack_s = value,
        Param::ThrottleRelease => settings.smoothing.throttle_release_s = value,
        Param::CurrentAttack => settings.smoothing.current_attack_s = value,
        Param::CurrentRelease => settings.smoothing.current_release_s = value,
        Param::SpeedAttack => settings.smoothing.speed_attack_s = value,
        Param::SpeedRelease => settings.smoothing.speed_release_s = value,
        Param::ToneGain => settings.tone.gain = value,
        Param::Brightness => settings.tone.brightness = value,
        Param::WhineGain => settings.tone.whine_gain = value,
        Param::WhineRatio => settings.tone.whine_ratio = value,
        Param::StereoWidth => settings.tone.stereo_width = value,
        Param::CurrentBase => settings.loudness.current_base_a = value,
        Param::CurrentFull => settings.loudness.current_full_a = value,
        Param::Floor => settings.loudness.floor = value,
        Param::PropGain => settings.prop_noise.gain = value,
        Param::PropLow => settings.prop_noise.center_low_hz = value,
        Param::PropHigh => settings.prop_noise.center_high_hz = value,
        Param::PropQ => settings.prop_noise.q = value,
        Param::BladeDepth => settings.prop_noise.blade_mod_depth = value,
        Param::WindGain => settings.wind.gain = value,
        Param::WindRef => settings.wind.speed_ref_kmh = value,
        Param::RollBalance => settings.wind.roll_balance = value,
        Param::GForceGain => settings.gforce.gain = value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_replay_core::synth::MOTOR_PRESETS;

    /// The sound a kind of aircraft starts with.
    fn starting_sound(kind: AircraftKind) -> SoundSettings {
        let (_, first) = kind.presets().next().unwrap();
        first.settings()
    }

    #[test]
    fn every_parameter_reads_back_what_was_set_and_the_starting_sounds_are_in_range() {
        for kind in AircraftKind::ALL {
            let start = starting_sound(kind);
            for spec in SPECS.iter().filter(|s| s.group.applies_to(kind)) {
                let (min, max) = spec.range(kind);
                let value = get(&start, spec.param);
                assert!(
                    (min..=max).contains(&value),
                    "{kind}, {}: the starting value {value} is outside {min}..{max}",
                    spec.label,
                );
                let mut changed = start.clone();
                let target = f64::midpoint(min, max).round();
                set(&mut changed, spec.param, target);
                assert!(
                    (get(&changed, spec.param) - target).abs() < 1e-9,
                    "{}",
                    spec.label
                );
            }
        }
    }

    #[test]
    fn every_parameter_is_in_the_table_once() {
        for (i, a) in SPECS.iter().enumerate() {
            assert!(
                SPECS.iter().skip(i + 1).all(|b| b.param != a.param),
                "{} twice",
                a.label
            );
        }
        assert_eq!(SPECS.len(), 33);
    }

    #[test]
    fn every_motor_preset_fits_in_the_sliders_of_its_kind() {
        for preset in MOTOR_PRESETS {
            let settings = preset.settings();
            for spec in SPECS.iter().filter(|s| s.group.applies_to(preset.kind)) {
                let (min, max) = spec.range(preset.kind);
                let value = get(&settings, spec.param);
                assert!(
                    (min..=max).contains(&value),
                    "{}: {} is {value}, outside {min}..{max}",
                    preset.name,
                    spec.label,
                );
            }
        }
    }

    #[test]
    fn every_group_has_parameters() {
        for group in Group::ALL {
            assert!(SPECS.iter().any(|s| s.group == group), "{group:?}");
        }
    }
}
