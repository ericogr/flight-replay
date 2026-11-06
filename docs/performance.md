# Performance

Measured on 2026-11-06, on an Intel Core i9-13900K (32 threads), with FFmpeg 8.0.1 from Ubuntu and software
encoders. The input is a raw goggle recording: 1280x720, 60 fps, H.264.

Each run renders 30 seconds (1800 frames) with the OSD, the SRT data and the sound, as `make`-built release code:

```sh
cargo run --release -p flight-replay-core --example render_sample -- \
    --video a.mp4 --osd a.osd --srt a.srt --font WS_INAV_Europa_24.png \
    --start 120 --duration 30 --encoder libx264 --out out.mp4
```

| Encoder | Time | Frames per second | Speed |
|---------|------|-------------------|-------|
| `libx264` (default preset, 40 Mbps) | 8.5 s | 211 | 3.5x real time |
| `libx265` (default preset, 40 Mbps) | 31.8 s | 57 | 0.9x real time |

What the numbers say:

- The pipeline (decode to RGBA, draw, pipe to the encoder, make the sound) goes above 200 frames per second. The
  encoder sets the speed, not the drawing.
- A 14-minute flight takes about 4 minutes with `libx264`, and about 15 minutes with `libx265`, on this machine.
- Hardware encoders (NVENC, QSV, AMF, VideoToolbox, VAAPI) were not measured. They are meant for the cases where
  software H.265 is too slow.
- Every frame goes through a pipe as RGBA (3.7 MB at 720p, 8.3 MB at 1080p). This was not the limit at 720p. At 1440p
  or 4K, compositing in YUV 4:2:0 would cut the data by 2.7 times, if the pipe turns out to be the limit.

The sound alone (video copied) is much faster: ten seconds of audio take a few milliseconds to make, and the video is
not touched.
