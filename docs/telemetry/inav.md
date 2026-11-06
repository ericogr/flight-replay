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

## Signals used by the sound

| Role | Column |
|------|--------|
| throttle (required) | `osd_throttle_pct` |
| current | `osd_current_a` |
| air speed | `osd_air_speed_kmh` |
| ground speed | `osd_gps_speed_kmh` |
| cell voltage | `osd_batt_cell_v` |
| roll | `osd_roll_deg` |
| pitch | `osd_pitch_deg` |
| G-force | `osd_gforce` |

## What the port is checked against

- `crates/core/tests/telemetry_reference.rs`: every `osd_*` column of every frame of `AvatarG0031.osd` equals the CSV
  that fpv-osd-decoder wrote from it (6606 frames, 176717 readings).
- `crates/core/tests/real_samples.rs`: the signal statistics and the probe of `AvatarG0153` equal what
  `motorsound --dry-run -v` and `motorsound probe` print.

## Differences from the Go code

- `\b` and `\B` are written `(?-u:\b)` and `(?-u:\B)`. Go's are ASCII-only. Rust's are Unicode by default, and the
  accented-letter codes of the font would count as word characters.
- The glyph prefilter of the Go decoder (which skips a rule when the row has none of its glyphs) is not ported. The
  Rust `regex` crate is fast enough: a whole flight decodes in about half a second.

## Betaflight

Betaflight OSD layouts need rules of their own, which are not written yet. See `PLAN.md`, phase P4b.
