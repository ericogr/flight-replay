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

## Properties

- **The motor is off a good part of the time**, mostly in flight (gliding). At a throttle at or below
  `off_threshold_pct` (2 %) the motor is stopped: the tone spins down and only the wind sounds. The current never
  becomes volume by itself, because it does not fall to zero (the system draws about 2.5 A).
- A reading that is not legible in a frame (blinking, another screen) is repeated from the previous frame, and
  back-filled at the start.
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
