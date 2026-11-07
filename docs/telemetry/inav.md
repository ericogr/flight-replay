# INAV telemetry rules

The decoder in `crates/core/src/telemetry/decoder/` reads numbers and texts from the glyph grid of an INAV OSD.
It is a port of the decoder of fpv-motorsound (Go), which comes from fpv-osd-decoder (Go).

## How a frame is read

1. Each row of the grid becomes a line of text with one character per cell, the character being the glyph code.
2. About 70 rules run on the line in a fixed order. A rule is a regular expression over the glyph codes (an icon,
   blanks, a number, a unit glyph) and a function that stores the value.
3. A recognized span is erased from the line before the next rule runs. This is why the order matters: the more
   specific rules (the maximum airspeed) come before the generic ones (the speed without an icon).
4. If a rule finds a column the sample already has, it leaves the match alone: the first reading in screen order
   wins. INAV draws some elements in exactly the same way (the capacity used and the capacity left), so glyphs
   alone cannot tell them apart.
5. On the pilot's own screen, a generic pass then takes any number that sits next to a glyph no rule knows, and
   names its column after those glyphs (`osd_raw_...`).
6. The flight mode and the status messages have no anchor glyph. Their position is found in the recording by
   `detect_flight_modes` and `detect_message_row`.

The statistics screen shown after disarming is not read at all. The splash screen shown while arming is read by the
rules, but not by the generic pass. A field that is not readable in a frame (blinking, no GPS) is absent from the
sample. It is never zero.

The throttle is the one field that has a reading where it shows no number. After its icon INAV writes the percentage,
or one of the words `STOP` (armed, and the throttle it shows is at or below zero), `IDLE` (the same, at exactly zero
with the motor-stop feature off, so the motor idles) or `DARM` (disarmed); see `osdFormatThrottlePosition` in the
INAV source. The decoder reads each of these words as a throttle of 0, since that is what they say. The word also
replaces the auto-throttle glyphs, so who was in control cannot be told and `osd_throttle_auto` is absent for those
frames. A negative percentage (` -1`) is read as shown.

## Signals used by the sound

| Role | Column | OSD element to turn on (INAV Configurator, OSD tab) |
|------|--------|-----------------------------------------------------|
| throttle (required) | `osd_throttle_pct` | Throttle Position |
| current | `osd_current_a` | Current Draw |
| air speed | `osd_air_speed_kmh` | Air Speed (needs a pitot tube) |
| ground speed | `osd_gps_speed_kmh` | Ground Speed |
| cell voltage | `osd_batt_cell_v` | Battery Cell Voltage |
| roll | `osd_roll_deg` | Roll Angle |
| pitch | `osd_pitch_deg` | Pitch Angle |
| G-force | `osd_gforce` | g force |

The last column is what the sound step tells the user when the recording lacks a signal (`Role::osd_element`, in
`synth/signals.rs`). Without the throttle there is no sound. Without any of the others the sound is made with less to
follow, and the warnings say what is lost. The ground speed stands in for the air speed, so the air speed is only
asked for when there is no ground speed either.

## What the port is checked against

- `crates/core/tests/telemetry_reference.rs`: every `osd_*` column of every frame of `AvatarG0031.osd` equals the CSV
  that fpv-osd-decoder wrote from it (6606 frames, 176717 readings).
- `crates/core/tests/real_samples.rs`: the signal statistics and the probe of `AvatarG0153` equal what
  `motorsound --dry-run -v` and `motorsound probe` print.

## Differences from the Go code

- The throttle reads `STOP`, `IDLE` and `DARM` as 0, and a negative percentage as shown. The Go decoder reads
  nothing for those frames, so the sound held the last throttle through them (and, before the first reading, the
  first one). On a recording that starts with `STOP` on the screen the motor sounded at the first percentage seen,
  while it was stopped. `crates/core/tests/telemetry_reference.rs` allows this one difference, and only where Go reads
  nothing.
- `\b` and `\B` are written `(?-u:\b)` and `(?-u:\B)`. Go's are ASCII-only. Rust's are Unicode by default, and the
  accented-letter codes of the font would count as word characters.
- The glyph prefilter of the Go decoder (which skips a rule when the row has none of its glyphs) is not ported. The
  Rust `regex` crate is fast enough: a whole flight decodes in about half a second.

## Betaflight

Betaflight OSD layouts need rules of their own, which are not written yet. See `PLAN.md`, phase P4b.
