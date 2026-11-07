// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/config/config.go, validate.go and default.yaml (Apache-2.0, Copyright 2026
// EricoGR). The synchronisation and output settings of that file are elsewhere in Flight Replay (`timeline`, the
// render options).

//! Every tunable of the sound model: audio format, which telemetry columns drive the sound, and the parameters of
//! the motor, the propeller and the wind. The defaults are those of fpv-motorsound.

use serde::{Deserialize, Serialize};

/// What kind of aircraft the sound is for. It decides what belongs to the aircraft and not to the motor: whether the
/// motor can stop in flight, how many motors there are, and the current that gives full volume. What each kind sets,
/// and the sounds of each kind, are in the `presets` module (`AircraftKind::apply`, `AircraftKind::presets`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AircraftKind {
    /// One motor and propeller that can stop in flight (gliding). The sound the program started with.
    #[default]
    FixedWing,
    /// A drone: several motors that keep spinning while it is armed.
    Multirotor,
}

impl AircraftKind {
    /// Every kind, in the order they are offered.
    pub const ALL: [Self; 2] = [Self::FixedWing, Self::Multirotor];

    /// The name shown in the interface.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::FixedWing => "Fixed wing",
            Self::Multirotor => "Drone",
        }
    }

    /// A line about what the choice sets.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::FixedWing => {
                "One motor that stops when the throttle is closed, so gliding is silent but for the wind."
            }
            Self::Multirotor => {
                "Several motors that never stop while the drone is armed: they idle at zero throttle and beat against one another."
            }
        }
    }
}

impl std::fmt::Display for AircraftKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// What the motor does when the throttle is below the off threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stopped {
    /// No motor sound.
    Silence,
    /// A low hum while the aircraft is on the ground.
    Idle,
    /// The motor keeps spinning at its idle speed while INAV shows `IDLE` in place of the throttle (armed, motor
    /// idling), as on a drone. `STOP` and `DARM` are silent.
    Spinning,
}

/// Format of the audio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// Samples per second (8000 to 192000; 44100 or 48000 are usual).
    pub sample_rate: u32,
    /// 1 (mono) or 2 (stereo).
    pub channels: u16,
    /// Final gain before the soft limiter, in dB.
    pub master_gain_db: f64,
    /// Noise seed: the same seed gives the same audio.
    pub seed: u64,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 2,
            master_gain_db: -8.0,
            seed: 1,
        }
    }
}

/// The telemetry columns the sound reads. An empty name turns the feature off. Only the throttle is required.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnSettings {
    /// Drives the pitch (RPM).
    pub throttle: String,
    /// Drives the volume and the propeller noise.
    pub current: String,
    /// Drives the wind.
    pub air_speed: String,
    /// The wind follows this when there is no air speed.
    pub gps_speed: String,
    /// Corrects the RPM by the battery voltage.
    pub cell_voltage: String,
    /// Optional secondary effect.
    pub gforce: String,
    /// Stereo balance of the wind.
    pub roll: String,
    /// Slight variation of the load.
    pub pitch: String,
    /// 1 where INAV shows `IDLE` in place of the throttle. Read only when the motor `stopped` setting is `spinning`.
    pub motor_idle: String,
}

impl Default for ColumnSettings {
    fn default() -> Self {
        Self {
            throttle: "osd_throttle_pct".into(),
            current: "osd_current_a".into(),
            air_speed: "osd_air_speed_kmh".into(),
            gps_speed: "osd_gps_speed_kmh".into(),
            cell_voltage: "osd_batt_cell_v".into(),
            gforce: "osd_gforce".into(),
            roll: "osd_roll_deg".into(),
            pitch: "osd_pitch_deg".into(),
            motor_idle: "osd_motor_idle".into(),
        }
    }
}

/// The motor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MotorSettings {
    /// Fundamental frequency (blade pass) at 0 % throttle, in Hz.
    pub f_idle_hz: f64,
    /// Fundamental frequency at 100 % throttle, in Hz.
    pub f_max_hz: f64,
    /// Throttle exponent (1 is linear, below 1 raises the low end faster).
    pub throttle_curve: f64,
    /// A throttle at or below this means the motor is stopped (gliding), in percent.
    pub off_threshold_pct: f64,
    /// What a stopped motor sounds like.
    pub stopped: Stopped,
    /// Hum volume when `stopped` is `idle`.
    pub idle_gain: f64,
    /// Below this speed, with the motor stopped, the aircraft counts as on the ground, in km/h.
    pub stop_speed_kmh: f64,
    /// How much the cell voltage lowers the RPM (0 is off).
    pub battery_gain: f64,
    /// Per-cell voltage that counts as full, in volts.
    pub battery_ref_v: f64,
    /// Compensates the voltage drop under load: V + I x R, in milliohms per cell.
    pub battery_sag_mohm: f64,
}

impl Default for MotorSettings {
    fn default() -> Self {
        Self {
            f_idle_hz: 110.0,
            f_max_hz: 520.0,
            throttle_curve: 1.0,
            off_threshold_pct: 2.0,
            stopped: Stopped::Silence,
            idle_gain: 0.12,
            stop_speed_kmh: 3.0,
            battery_gain: 0.5,
            battery_ref_v: 4.0,
            battery_sag_mohm: 0.0,
        }
    }
}

/// The most motors an aircraft can have.
pub const MAX_ROTORS: usize = 8;

/// The largest fixed difference of speed between the motors, and the largest drift, as a fraction of the speed.
const MAX_ROTOR_DIFFERENCE: f64 = 0.1;

/// The largest part of the speed that follows the roll or pitch rate, as a fraction for each degree per second.
const MAX_ROTOR_ATTITUDE: f64 = 0.01;

/// How far a rotor's speed may stray from the common speed, as a ratio: it is kept between `1 - x` and `1 + x`.
pub const ROTOR_RATIO_SPAN: f64 = 0.2;

/// Several motors: the speed of each one is a little different from the others, which makes the sound rough.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RotorSettings {
    /// Number of motors (1 to 8). With 1 the sound is that of one motor, as on a fixed wing.
    pub count: u8,
    /// A fixed difference of speed between the motors, as a fraction of the speed.
    pub detune: f64,
    /// A slow, random drift of the speed of each motor (the controller correcting all the time), as a fraction.
    pub wander: f64,
    /// How much a roll or pitch rate speeds up the motors on one side and slows the others, as a fraction of the
    /// speed for each degree per second.
    pub attitude: f64,
}

impl Default for RotorSettings {
    fn default() -> Self {
        Self {
            count: 1,
            detune: 0.0,
            wander: 0.0,
            attitude: 0.0,
        }
    }
}

impl RotorSettings {
    /// The largest ratio between the speed of a motor and the common speed.
    #[must_use]
    pub fn top_ratio(&self) -> f64 {
        if self.count > 1 {
            1.0 + ROTOR_RATIO_SPAN
        } else {
            1.0
        }
    }
}

/// Motor and propeller inertia: time constants, in seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SmoothingSettings {
    /// RPM rising.
    pub throttle_attack_s: f64,
    /// RPM falling.
    pub throttle_release_s: f64,
    /// Current rising.
    pub current_attack_s: f64,
    /// Current falling.
    pub current_release_s: f64,
    /// Wind speed rising.
    pub speed_attack_s: f64,
    /// Wind speed falling.
    pub speed_release_s: f64,
    /// Motor starting.
    pub gate_attack_s: f64,
    /// Motor spinning down to a stop.
    pub gate_release_s: f64,
    /// The battery voltage changes slowly.
    pub battery_s: f64,
}

impl Default for SmoothingSettings {
    fn default() -> Self {
        Self {
            throttle_attack_s: 0.15,
            throttle_release_s: 0.40,
            current_attack_s: 0.10,
            current_release_s: 0.35,
            speed_attack_s: 0.80,
            speed_release_s: 1.20,
            gate_attack_s: 0.10,
            gate_release_s: 0.30,
            battery_s: 3.00,
        }
    }
}

/// The tonal part: propeller and motor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToneSettings {
    /// Volume of the tone.
    pub gain: f64,
    /// Amplitude of the fundamental and of each harmonic (1 to 16 values).
    pub harmonics: Vec<f64>,
    /// 0 to 1: the upper harmonics grow with the load.
    pub brightness: f64,
    /// The motor whine is this ratio times the fundamental (it is not a harmonic).
    pub whine_ratio: f64,
    /// Volume of the whine.
    pub whine_gain: f64,
    /// 0 puts everything in the centre; 1 spreads the harmonics wide.
    pub stereo_width: f64,
}

impl Default for ToneSettings {
    fn default() -> Self {
        Self {
            gain: 0.55,
            harmonics: vec![1.0, 0.60, 0.38, 0.24, 0.15, 0.09],
            brightness: 0.55,
            whine_ratio: 3.5,
            whine_gain: 0.06,
            stereo_width: 0.45,
        }
    }
}

/// Volume as a function of the current.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoudnessSettings {
    /// Draw with the motor off (video link, servos...). It does not count as load, in amperes.
    pub current_base_a: f64,
    /// The current that gives full volume, in amperes.
    pub current_full_a: f64,
    /// Minimum audible volume while the motor is spinning.
    pub floor: f64,
    /// Load exponent.
    pub curve: f64,
    /// Climbing (positive pitch) raises the load a little.
    pub pitch_gain: f64,
}

impl Default for LoudnessSettings {
    fn default() -> Self {
        Self {
            current_base_a: 2.6,
            current_full_a: 12.0,
            floor: 0.30,
            curve: 0.8,
            pitch_gain: 0.10,
        }
    }
}

/// Propeller noise: a band-pass noise proportional to the load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PropNoiseSettings {
    /// Volume of the noise.
    pub gain: f64,
    /// Centre of the band at 0 % throttle, in Hz.
    pub center_low_hz: f64,
    /// Centre of the band at 100 % throttle, in Hz.
    pub center_high_hz: f64,
    /// Sharpness of the band.
    pub q: f64,
    /// Depth of the noise modulation at the blade-pass frequency.
    pub blade_mod_depth: f64,
}

impl Default for PropNoiseSettings {
    fn default() -> Self {
        Self {
            gain: 0.80,
            center_low_hz: 1100.0,
            center_high_hz: 3200.0,
            q: 1.1,
            blade_mod_depth: 0.30,
        }
    }
}

/// Wind: a low-pass noise proportional to the air speed. It is the only sound while gliding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindSettings {
    /// Volume of the wind.
    pub gain: f64,
    /// The speed that gives full wind, in km/h.
    pub speed_ref_kmh: f64,
    /// Speed exponent.
    pub curve: f64,
    /// Cutoff at speed 0, in Hz.
    pub cutoff_low_hz: f64,
    /// Cutoff at the reference speed, in Hz.
    pub cutoff_high_hz: f64,
    /// Banking (roll) pans the wind between left and right.
    pub roll_balance: f64,
}

impl Default for WindSettings {
    fn default() -> Self {
        Self {
            gain: 1.80,
            speed_ref_kmh: 60.0,
            curve: 1.6,
            cutoff_low_hz: 220.0,
            cutoff_high_hz: 1300.0,
            roll_balance: 0.20,
        }
    }
}

/// An optional secondary effect of the G-force.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GForceSettings {
    /// Above 0, `|g - 1|` raises the propeller noise and the wind. 0 is off.
    pub gain: f64,
}

impl Default for GForceSettings {
    fn default() -> Self {
        Self { gain: 0.0 }
    }
}

/// All the parameters of the sound.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundSettings {
    /// What kind of aircraft the sound is for. It comes first because a TOML file has its plain values before its
    /// tables.
    pub aircraft: AircraftKind,
    /// Audio format.
    pub audio: AudioSettings,
    /// Telemetry columns.
    pub columns: ColumnSettings,
    /// The motor.
    pub motor: MotorSettings,
    /// Several motors.
    pub rotors: RotorSettings,
    /// Inertia.
    pub smoothing: SmoothingSettings,
    /// The tonal part.
    pub tone: ToneSettings,
    /// Volume from the current.
    pub loudness: LoudnessSettings,
    /// Propeller noise.
    pub prop_noise: PropNoiseSettings,
    /// Wind.
    pub wind: WindSettings,
    /// G-force effect.
    pub gforce: GForceSettings,
}

/// How close to Nyquist a tone may get before it aliases badly.
const NYQUIST_MARGIN: f64 = 0.95;

impl SoundSettings {
    /// Checks ranges that would otherwise produce silent garbage. Returns every problem at once.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut problems = Vec::new();
        self.validate_audio(&mut problems);
        self.validate_motor(&mut problems);
        self.validate_rotors(&mut problems);
        self.validate_smoothing(&mut problems);
        self.validate_tone(&mut problems);
        self.validate_loudness(&mut problems);
        self.validate_prop_noise(&mut problems);
        self.validate_wind(&mut problems);
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }

    fn nyquist(&self) -> f64 {
        f64::from(self.audio.sample_rate) / 2.0
    }

    fn validate_audio(&self, problems: &mut Vec<String>) {
        let a = &self.audio;
        if !(8_000..=192_000).contains(&a.sample_rate) {
            problems.push(format!(
                "audio.sample_rate={} is outside 8000..192000",
                a.sample_rate
            ));
        }
        if a.channels != 1 && a.channels != 2 {
            problems.push(format!(
                "audio.channels={}: use 1 (mono) or 2 (stereo)",
                a.channels
            ));
        }
        if self.columns.throttle.is_empty() {
            problems.push("columns.throttle is required (it drives the motor pitch)".into());
        }
    }

    fn validate_motor(&self, problems: &mut Vec<String>) {
        let m = &self.motor;
        if m.f_idle_hz <= 0.0 || m.f_max_hz <= m.f_idle_hz {
            problems.push(format!(
                "motor: needs 0 < f_idle_hz ({}) < f_max_hz ({})",
                m.f_idle_hz, m.f_max_hz
            ));
        }
        if m.throttle_curve <= 0.0 {
            problems.push(format!(
                "motor.throttle_curve={} must be > 0",
                m.throttle_curve
            ));
        }
        if m.battery_ref_v <= 0.0 {
            problems.push(format!(
                "motor.battery_ref_v={} must be > 0",
                m.battery_ref_v
            ));
        }
    }

    fn validate_rotors(&self, problems: &mut Vec<String>) {
        let r = &self.rotors;
        if !(1..=MAX_ROTORS).contains(&usize::from(r.count)) {
            problems.push(format!(
                "rotors.count={} is outside 1..{MAX_ROTORS}",
                r.count
            ));
        }
        for (key, value, top) in [
            ("rotors.detune", r.detune, MAX_ROTOR_DIFFERENCE),
            ("rotors.wander", r.wander, MAX_ROTOR_DIFFERENCE),
            ("rotors.attitude", r.attitude, MAX_ROTOR_ATTITUDE),
        ] {
            if !(0.0..=top).contains(&value) {
                problems.push(format!("{key}={value} is outside 0..{top}"));
            }
        }
    }

    fn validate_smoothing(&self, problems: &mut Vec<String>) {
        let s = &self.smoothing;
        for (key, value) in [
            ("smoothing.throttle_attack_s", s.throttle_attack_s),
            ("smoothing.throttle_release_s", s.throttle_release_s),
            ("smoothing.current_attack_s", s.current_attack_s),
            ("smoothing.current_release_s", s.current_release_s),
            ("smoothing.speed_attack_s", s.speed_attack_s),
            ("smoothing.speed_release_s", s.speed_release_s),
            ("smoothing.gate_attack_s", s.gate_attack_s),
            ("smoothing.gate_release_s", s.gate_release_s),
            ("smoothing.battery_s", s.battery_s),
        ] {
            if value < 0.0 {
                problems.push(format!("{key}={value} cannot be negative"));
            }
        }
    }

    fn validate_tone(&self, problems: &mut Vec<String>) {
        // The fastest of several motors runs a little above the common speed.
        let f_max = self.motor.f_max_hz * self.rotors.top_ratio();
        let (t, nyquist) = (&self.tone, self.nyquist());
        if t.harmonics.is_empty() || t.harmonics.len() > 16 {
            problems.push("tone.harmonics: give 1 to 16 amplitudes".into());
        }
        for k in 0..t.harmonics.len() {
            if (k + 1) as f64 * f_max >= nyquist * NYQUIST_MARGIN {
                problems.push(format!(
                    "tone.harmonics: harmonic {} ({:.0} Hz) is above Nyquist ({nyquist:.0} Hz)",
                    k + 1,
                    (k + 1) as f64 * f_max
                ));
                break;
            }
        }
        if t.whine_gain > 0.0 && t.whine_ratio * f_max >= nyquist * NYQUIST_MARGIN {
            problems.push(format!(
                "tone.whine_ratio*motor.f_max_hz = {:.0} Hz is above Nyquist ({nyquist:.0} Hz)",
                t.whine_ratio * f_max
            ));
        }
        if !(0.0..=1.0).contains(&t.stereo_width) {
            problems.push(format!(
                "tone.stereo_width={} is outside 0..1",
                t.stereo_width
            ));
        }
        if !(0.0..=1.0).contains(&t.brightness) {
            problems.push(format!("tone.brightness={} is outside 0..1", t.brightness));
        }
    }

    fn validate_loudness(&self, problems: &mut Vec<String>) {
        let l = &self.loudness;
        if l.current_full_a <= l.current_base_a {
            problems.push(format!(
                "loudness: current_full_a ({}) must be > current_base_a ({})",
                l.current_full_a, l.current_base_a
            ));
        }
        if !(0.0..=1.0).contains(&l.floor) {
            problems.push(format!("loudness.floor={} is outside 0..1", l.floor));
        }
        if l.curve <= 0.0 {
            problems.push(format!("loudness.curve={} must be > 0", l.curve));
        }
    }

    fn validate_prop_noise(&self, problems: &mut Vec<String>) {
        let p = &self.prop_noise;
        if p.center_low_hz <= 0.0
            || p.center_high_hz < p.center_low_hz
            || p.center_high_hz >= self.nyquist() * 0.9
        {
            problems
                .push("prop_noise: needs 0 < center_low_hz <= center_high_hz < 0.9*Nyquist".into());
        }
        if p.q <= 0.0 {
            problems.push(format!("prop_noise.q={} must be > 0", p.q));
        }
    }

    fn validate_wind(&self, problems: &mut Vec<String>) {
        let w = &self.wind;
        if w.cutoff_low_hz <= 0.0
            || w.cutoff_high_hz < w.cutoff_low_hz
            || w.cutoff_high_hz >= self.nyquist() * 0.9
        {
            problems.push("wind: needs 0 < cutoff_low_hz <= cutoff_high_hz < 0.9*Nyquist".into());
        }
        if w.speed_ref_kmh <= 0.0 || w.curve <= 0.0 {
            problems.push("wind.speed_ref_kmh and wind.curve must be > 0".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_valid() {
        assert_eq!(SoundSettings::default().validate(), Ok(()));
    }

    #[test]
    fn reports_every_problem_at_once() {
        let mut settings = SoundSettings::default();
        settings.audio.channels = 3;
        settings.motor.f_max_hz = 50.0;
        settings.loudness.current_full_a = 1.0;
        settings.columns.throttle.clear();
        let problems = settings.validate().unwrap_err();
        assert!(problems.len() >= 4, "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("audio.channels")));
        assert!(problems.iter().any(|p| p.contains("current_full_a")));
    }

    #[test]
    fn a_tone_above_nyquist_is_refused() {
        let mut settings = SoundSettings::default();
        settings.audio.sample_rate = 8_000;
        settings.motor.f_max_hz = 700.0;
        let problems = settings.validate().unwrap_err();
        assert!(
            problems.iter().any(|p| p.contains("above Nyquist")),
            "{problems:?}"
        );
    }

    #[test]
    fn settings_round_trip_through_toml_and_missing_keys_use_defaults() {
        let text = "[motor]\nf_max_hz = 600.0\n\n[audio]\nchannels = 1\n";
        let settings: SoundSettings = toml::from_str(text).unwrap();
        assert!((settings.motor.f_max_hz - 600.0).abs() < 1e-12);
        assert_eq!(settings.audio.channels, 1);
        assert_eq!(settings.audio.sample_rate, 48_000);
        let again: SoundSettings = toml::from_str(&toml::to_string(&settings).unwrap()).unwrap();
        assert_eq!(again, settings);
    }

    #[test]
    fn a_fixed_wing_with_no_rotor_settings_is_what_files_from_before_read_as() {
        // a settings.toml from before the kind of aircraft existed
        let settings: SoundSettings = toml::from_str("[motor]\nf_max_hz = 600.0\n").unwrap();
        assert_eq!(settings.aircraft, AircraftKind::FixedWing);
        assert_eq!(settings.rotors, RotorSettings::default());
        assert_eq!(settings.motor.stopped, Stopped::Silence);
        assert_eq!(settings.columns.motor_idle, "osd_motor_idle");
    }

    #[test]
    fn the_rotors_are_checked() {
        let mut settings = SoundSettings::default();
        settings.rotors.count = 0;
        settings.rotors.detune = 0.5;
        let problems = settings.validate().unwrap_err();
        assert!(
            problems.iter().any(|p| p.contains("rotors.count")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("rotors.detune")),
            "{problems:?}"
        );
        settings.rotors.count = 9;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn the_fastest_of_several_motors_counts_for_nyquist() {
        let mut settings = SoundSettings::default();
        settings.audio.sample_rate = 44_100;
        settings.tone.harmonics = vec![1.0; 9];
        // 9 harmonics of 2,200 Hz is 19,800 Hz, under the limit of 20,947 Hz for a single motor...
        settings.motor.f_max_hz = 2_200.0;
        assert_eq!(settings.validate(), Ok(()));
        // ...but several motors run up to 20 % faster
        settings.rotors.count = 4;
        assert!(settings.validate().is_err());
    }
}
