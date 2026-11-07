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

## Aircraft and motor presets

The sound step starts with the **aircraft**: a fixed wing or a drone. The choice sets what belongs to the aircraft
(`AircraftKind::apply`, in `crates/core/src/synth/presets.rs`), and then the first sound of that kind. Under it, the
list of **motor and propeller** offers the sounds of that kind only (`MOTOR_PRESETS`, each with its `kind`). Choosing
one is a starting point: every value it sets can still be changed.

| Setting | Fixed wing (what the program started with) | Drone |
|---------|--------------------------------------------|-------|
| `aircraft` | `fixed_wing` | `multirotor` |
| `rotors.count` | 1 | 4 |
| `motor.off_threshold_pct` | 2 % | 0 %: any throttle above zero sounds |
| `motor.stopped` | `silence` | `spinning`: where INAV shows `IDLE` the motors idle at `f_idle_hz`; `STOP` and `DARM` are silent |
| `loudness.current_base_a`, `current_full_a` | 2.6 A, 12 A | 3 A, 60 A (a five-inch draws 15 to 25 A hovering and 80 to 100 A at full throttle) |
| `loudness.pitch_gain` | 0.10 | 0: on a drone the pitch angle is the lean, not the climb |

Choosing the kind again does nothing, so values changed by hand are not thrown away. Choosing the other kind brings
its settings and its first sound back, and keeps what belongs to the recording: the audio format and the master gain,
the columns, the battery correction, the wind and the G-force effect. An old `settings.toml` has none of these keys
and reads as a fixed wing, so nothing changes for it.

### Fixed wing

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

### Drone

| Preset | What it models |
|--------|----------------|
| 5-inch freestyle | Three-blade propellers from about 5,000 rpm at idle to 38,000: a blade pass of 250 to 1,900 Hz, a rich, buzzy tone, quick response |
| 7-inch long range | Bigger propellers turning slower (160 to 1,000 Hz): lower and rounder, a smoother response |
| Cinewhoop, ducted | Small propellers inside ducts: the same buzz, muffled, with more air noise and less whine |

The whine of a brushless motor is `pole pairs / blades` times the blade pass: 7 / 2 = 3.5 for a 14-pole motor and
two blades (the default), and 7 / 3, about 2.3, with three blades (the drones).

A preset sets the **character** of the sound: the pitch range (`motor.f_idle_hz`, `f_max_hz`, `throttle_curve`), the
inertia of the pitch and of the volume, the whole `tone` and the whole `prop_noise`, and, for a drone, how much the
motors differ (`rotors.detune`, `wander`, `attitude`). It leaves alone what belongs to the aircraft and the
recording. After a preset the sound list says "Custom" as soon as one of those values is changed by hand. The
harmonics list is the one part of a preset that has no slider; it is in `settings.toml` (`sound.tone.harmonics`).

To add a preset, write a function that builds a `SoundSettings` (on `multirotor_base()` for a drone) and add a row to
`MOTOR_PRESETS` with its kind. The tests (`crates/core/tests/presets.rs`) check that it is valid at 44.1 and 48 kHz,
that its level on a throttle sweep of a flight of its own kind is within 1.5 dB of the default and does not reach the
limiter, that it differs by at least 3 dB from every other preset of its kind in some octave band, and (in the
window) that every value fits its slider. They cannot tell whether it sounds right: that is for the ear.

## Several motors

A drone has four motors, and they never turn at exactly the same speed: the controller corrects each one all the time.
That is where the rough, buzzing sound of a drone comes from, and one oscillator cannot make it. With
`rotors.count` above 1 the engine renders a harmonic series for each motor (`RotorBank`, in
`crates/core/src/synth/rotors.rs`), and the speed of motor `r` is the common speed times a ratio, kept between 0.8
and 1.2:

`ratio = 1 + spread[r] * detune + wander[r] + attitude * (roll_sign[r] * roll_rate + pitch_sign[r] * pitch_rate)`

- `spread` is fixed, from -1 to 1 with a mean of 0, so the pitch stays where the throttle puts it. `detune` is the
  fraction of speed it stands for (1.2 % in the five-inch).
- `wander` is a slow random drift of each motor, a one-pole low-pass of seeded noise (0.4 s), with a standard
  deviation of `rotors.wander`. It is deterministic like the rest: the same seed gives the same bytes.
- The last term follows the rolling and pitching of the drone. The motors sit on a circle, the first at the front
  right; rolling right slows the motors on the right and speeds up the ones on the left, and pitching up speeds up
  the ones at the front. The rates come from the roll and pitch of the OSD (degrees per second, smoothed over 0.1 s).
  Without those readings the term is 0 and the warnings say so.
- Each motor has its own phases of the harmonics and its own place in the stereo picture (a motor on the left of the
  drone sounds on the left, scaled by `tone.stereo_width`), and the motors are added as if they did not sound in
  phase (divided by the square root of their number), so four motors are as loud as one.

With `rotors.count = 1` none of this runs and the output is, sample by sample, what it was before the drone existed;
the settings `detune`, `wander` and `attitude` do nothing then.

## Properties

- **On a fixed wing the motor is off a good part of the time**, mostly in flight (gliding). At a throttle at or below
  `off_threshold_pct` (2 %) the motor is stopped: the tone spins down and only the wind sounds. The current never
  becomes volume by itself, because it does not fall to zero (the system draws about 2.5 A). A drone has no such
  state: its motors idle while it is armed, so its threshold is 0, and the only silences are `STOP` and `DARM`.
- A reading that is not legible in a frame (blinking, another screen) is repeated from the previous frame, and
  back-filled at the start. The throttle is the exception at the start: before its first reading the motor is off
  (the splash screen, the time before the element is drawn), so the throttle is 0 there and is not copied back from
  the first reading.
- Where INAV shows `STOP`, `IDLE` or `DARM` in place of the throttle percentage, the throttle is 0. It is a reading,
  not a gap, so it is not carried over from the frame before. `osd_motor_idle` is 1 only for `IDLE`: a fixed wing is
  silent there as for the others, and a drone idles (`motor.stopped = "spinning"`). A negative percentage (the stick just under the end of
  its range) is read as shown and counts as 0 for the sound.
- After the last throttle reading the throttle is held for 2 s, then the motor stops: once the OSD no longer shows
  the throttle, the motor is off (after disarming, at the end of a recording).
- The oscillator uses a **phase accumulator**, never `sin(2 pi f t)`, so there are no clicks when the frequency
  changes.
- Stereo: the fundamental is centred, the upper harmonics alternate sides (`tone.stereo_width`), and the propeller and
  wind noise is independent in each channel.
- Rendering runs in blocks and is **deterministic**: the control values advance on a grid of absolute sample indices,
  so the output does not depend on the block size. The same seed gives the same bytes. The seed (`audio.seed`) can be
  any 64-bit number: the streams of noise are `seed * 8 + n`, with wrapping arithmetic.

## What it is checked against

- `crates/core/tests/synth.rs`: determinism, independence of the block size, the pitch within 0.1 % of the throttle
  mapping at five throttle values, silence with a stopped motor, the limiter, the seed.
- `crates/core/tests/real_samples.rs`: ten seconds of `AvatarG0153` rendered by the Go program and by this engine on the
  same telemetry and settings are identical, sample by sample (0 of 960000 samples differ).
