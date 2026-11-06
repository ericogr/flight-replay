# `.osd` files

Walksnail Avatar goggles record an `.osd` file next to each video. It holds the OSD screen that the pilot saw,
frame by frame, as a grid of glyph codes. It does not hold numbers.

## Layout (little-endian)

| Offset | Size | Content |
|--------|------|---------|
| `0x00` | 4 bytes | Firmware code: `INAV`, `BTFL` (Betaflight), `ARDU`, `KISS`, `ULTR` |
| `0x24` | `u16` | Columns of the grid |
| `0x26` | `u16` | Rows of the grid |
| `0x28` | 40 bytes in total | End of the header |

After the header, the file is a sequence of frames of the same size:

| Offset in frame | Size | Content |
|-----------------|------|---------|
| `0` | `u32` | Time of the screen, in milliseconds from the start of the recording |
| `4` | `u16` × columns × rows | Glyph code of each cell, row by row. `0` is an empty cell; `0x20` is a space |

A 53x20 grid gives frames of 2124 bytes. Any trailing bytes that do not make a whole frame are ignored.

## Timing

The screens are recorded at about 6 to 7 per second, at irregular intervals (the mean interval in the sample is
157 ms). Screen `n` applies from its time until the next screen's time.

The timestamps follow real time on the raw goggle recording checked in phase P2 (AvatarG0153: the last screen is
at 845.9 s, the video lasts 846.1 s). The README of fpv-motorsound describes its sample flights as stamped as if
the video ran at 60 fps. The scale that reconciles the two is in `crates/core/src/timeline.rs`.

## Reading the values

The OSD does not store text. A value is read from the glyphs that draw it, by its icon and unit. Rules for each
INAV element are in `crates/core/src/telemetry/`. Blinking or hidden elements are missing from some screens.

## Several files for one video

The goggles write one `.osd` file for each recording. When the pilot joins several recordings into one video, the
video has one timeline, so the `.osd` files have to follow one another on it, in the order they were recorded
(`crates/core/src/osd/join.rs`):

- The first file keeps its times. Each following file starts where the one before it ends. A file ends at its last
  screen plus the average interval between its screens, so the length of the joined recording is the sum of the
  lengths of the files.
- The files must have the same grid and the same firmware. Otherwise they do not belong to one video, and they are
  refused.
- The overlay, the sound and the telemetry loader all use these shifts, so they agree about where each file starts.
- The coverage check compares the joined length with the length of the video. It reports a difference larger than
  2 seconds or 1.5 % of the video: a file may be missing, or a file may belong to another video. Goggles and video
  clocks drift a little, so a smaller difference is normal. The automatic speed of the OSD (see
  [the timeline](../audio-model.md)) hides a missing file, which is why the check does not use it.
