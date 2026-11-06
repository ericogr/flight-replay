# Test fixtures

A short clip used by the tests, cut from a raw Walksnail Avatar recording that is not part of this repository. Full
recordings never go into the repository.

| File | Content |
|------|---------|
| `clip-3s.mp4` | 3 seconds of video, from second 120 of the recording (a flight in progress). 1280x720, H.264, 60 fps, 180 frames, no sound. Stream copy: the video is not re-encoded. No OSD is burned into the picture |
| `clip-3s.osd` | The OSD of the same 3 seconds: INAV, 53x20 grid, 22 frames, times counted from the start of the clip |
| `clip-3s.srt` | The SRT data of the same 3 seconds: 21 cues |

## What was changed

- **GPS positions.** The OSD shows the position of the aircraft. In `clip-3s.osd` every latitude and longitude was
  moved by one constant offset to a **fake point in the South Atlantic** (about 35.2 S, 31.4 W). The track keeps its
  shape, speed and distances, so the telemetry stays consistent. The offset is the difference between the first
  real position and the fake target. It is computed while the command runs and is never stored. Only the digit
  glyphs of those cells changed. Every other reading was checked to be identical to the original.
- **Times.** The OSD and SRT times are counted from the start of the clip.
- **The picture is not changed.** It is an aerial view of a city. The owner of the recording accepts that.

The test `the_positions_in_the_clip_are_the_fake_ones_in_the_south_atlantic` fails if a position outside the fake
area gets into the fixture.

## How it is made

```sh
make fixtures SAMPLES=/folder/of/the/recording RECORDING=AvatarG0153
```

This runs `cargo xtask fixtures`. The command also checks that the moved positions equal the original ones plus the
offset, and that no OSD frame shows a plus code (which would give the position away).

There is no font in the fixtures. The goggles' fonts are files of the manufacturer, so the tests draw with a font
sheet made by the tests themselves (`crates/core/tests/common/mod.rs`).
