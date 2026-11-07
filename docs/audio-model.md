# Audio model

The sound is synthesized from the telemetry in the `.osd` file. It is a port of fpv-motorsound's model
(`crates/core/src/synth/`). The parameters and their defaults are in `SoundSettings`.

## What drives what

| Effect | Telemetry | How |
|--------|-----------|-----|
| Pitch (RPM) | throttle (`osd_throttle_pct`) | Smoothed (rise 0.15 s, fall 0.4 s), mapped linearly to `f_idle_hz..f_max_hz` (110 to 520 Hz), slightly corrected by the cell voltage |
| Volume and propeller noise | current (`osd_current_a`) | Above the base draw (about 2.6 A), times "motor on". An audible floor while the motor spins. The throttle is used when there is no current |
| Wind | air speed (`osd_air_speed_kmh`), else ground speed | Low-pass noise. Its brightness and volume grow with the speed |
| Stereo | roll (`osd_roll_deg`) | Pans the wind slightly between left and right |
| Load | pitch (`osd_pitch_deg`) | A small variation with the climb angle |
| Optional | G-force (`osd_gforce`) | Off by default (`gforce.gain`) |

Only the throttle is required. A signal the recording does not have is reported, with what it costs the sound
(`SignalSet::warnings`).

## Motor presets

The sound step offers ready-made sounds for different electric motors and propellers (`MOTOR_PRESETS`, in
`crates/core/src/synth/presets.rs`). Choosing one is a starting point: every value it sets can still be changed.

| Preset | What it models |
|--------|----------------|
| Default | The sound the program starts with: a two-blade propeller, a warm mid-range tone |
| Small park flyer | A small propeller turning fast: high and thin, some whine, quick response |
| Small high-KV motor | A tiny high-KV motor on a small propeller, 20,000 rpm and more: the highest pitch, a thin piercing tone, a loud whine, almost no lag |
| Sport plane, three blades | A higher blade pass, a rounder tone and broader air noise |
| Aerobatic power | A big motor on a short propeller: hard, bright and buzzy, answers the throttle at once |
| Large slow propeller | Long-range plane or motor glider: deep and rich, chops the air, slow to follow |
| Pusher behind the wing | The same motor, dull and muffled |
| Ducted fan | A high whine over a hiss, almost no beat of the blades |

A preset sets the **character** of the sound: the pitch range (`motor.f_idle_hz`, `f_max_hz`, `throttle_curve`), the
inertia of the pitch and of the volume, the whole `tone` and the whole `prop_noise`. It leaves alone what belongs to
the aircraft and the recording: the audio format and the master gain, the columns, the current that gives full
volume, the battery correction, the wind and the G-force effect. After a preset the sound list says "Custom" as soon
as one of those values is changed by hand. The harmonics list is the one part of a preset that has no slider; it is
in `settings.toml` (`sound.tone.harmonics`).

To add a preset, write a function that builds a `SoundSettings` and add a row to `MOTOR_PRESETS`. The tests
(`crates/core/tests/presets.rs`) check that it is valid at 44.1 and 48 kHz, that its level on a throttle sweep is
within 1.5 dB of the default and does not reach the limiter, that it differs by at least 3 dB from every other preset
in some octave band, and (in the window) that every value fits its slider. They cannot tell whether it sounds right:
that is for the ear.

## Properties

- **The motor is off a good part of the time**, mostly in flight (gliding). At a throttle at or below
  `off_threshold_pct` (2 %) the motor is stopped: the tone spins down and only the wind sounds. The current never
  becomes volume by itself, because it does not fall to zero (the system draws about 2.5 A).
- A reading that is not legible in a frame (blinking, another screen) is repeated from the previous frame, and
  back-filled at the start. The throttle is the exception at the start: before its first reading the motor is off
  (the splash screen, the time before the element is drawn), so the throttle is 0 there and is not copied back from
  the first reading.
- Where INAV shows `STOP`, `IDLE` or `DARM` in place of the throttle percentage, the throttle is 0. It is a reading,
  not a gap, so it is not carried over from the frame before. A negative percentage (the stick just under the end of
  its range) is read as shown and counts as 0 for the sound.
- After the last throttle reading the throttle is held for 2 s, then the motor stops: once the OSD no longer shows
  the throttle, the motor is off (after disarming, at the end of a recording).
- The oscillator uses a **phase accumulator**, never `sin(2 pi f t)`, so there are no clicks when the frequency
  changes.
- Stereo: the fundamental is centred, the upper harmonics alternate sides (`tone.stereo_width`), and the propeller and
  wind noise is independent in each channel.
- Rendering runs in blocks and is **deterministic**: the control values advance on a grid of absolute sample indices,
  so the output does not depend on the block size. The same seed gives the same bytes.

## What it is checked against

- `crates/core/tests/synth.rs`: determinism, independence of the block size, the pitch within 0.1 % of the throttle
  mapping at five throttle values, silence with a stopped motor, the limiter, the seed.
- `crates/core/tests/real_samples.rs`: ten seconds of `AvatarG0153` rendered by the Go program and by this engine on the
  same telemetry and settings are identical, sample by sample (0 of 960000 samples differ).
