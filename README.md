# Flight Replay

Flight Replay renders the flight controller OSD and the SRT data onto a Walksnail Avatar recording, and adds a
synthesized motor and propeller sound that follows the telemetry stored in the `.osd` file. It is a step-by-step
wizard for Linux, Windows and macOS.

![The icon of Flight Replay](assets/icon/flight-replay-128.png)

## What it does

- **The OSD on the picture.** The glyphs of the `.osd` file are drawn with a font that comes with the program (INAV,
  Betaflight) or with the font file of your goggles, at any position, with elements you can mask by clicking them.
  Fonts of 2 and 4 colours, 256 and 512 glyphs.
- **The SRT data on the picture.** Signal, latency, bitrate, distance, battery and time, where you want them.
- **A motor sound.** The telemetry is read from the OSD screens (throttle, current, battery voltage, and the speed,
  roll and pitch when the OSD shows them), and drives a synthesized motor, propeller and wind sound. Every parameter
  can be changed, listened to, and saved as a preset. You choose a fixed wing or a drone, and then one of the
  ready-made sounds for that aircraft (eight for planes, three for drones with four motors that beat against one
  another) as the starting point. See [docs/audio-model.md](docs/audio-model.md).
- **Encoding.** libx264, libx265 and the hardware encoders FFmpeg has on your machine (NVENC, QSV, AMF,
  VideoToolbox, VAAPI), found by a test encode. Optional 1440p scaling, and a green background for compositing in an
  editor. With only the sound, the picture is copied as it is.
- **Several videos.** A queue renders many videos with the same settings, or all the videos of a folder. A video made
  of several recordings takes several `.osd` files, which are put one after the other.
- **Checks.** The result is checked after the render: the length of the picture and of the sound, and how well the OSD
  covers the video.

Support: the overlay and the SRT work with INAV and Betaflight OSD files. The sound is read from INAV OSDs. Betaflight
files get the overlay and no sound until the rules for its screens are written (see [PLAN.md](PLAN.md), phase P4b).

**You get what the files give.** Each part needs its own files, and a part that lacks them is switched off, with the
reason on its step, while the rest goes on: the OSD needs the `.osd` file and the font, the SRT data need the `.srt`
file, and the sound needs an INAV `.osd` that shows the throttle. A video with only an `.srt` still gets the SRT data.
When the OSD does not show something the sound needs, the sound step says which element to turn on in the INAV
Configurator (OSD tab), such as "Throttle Position", to get it on the next flight. The same is done, video by video,
for a folder.

## Install

Download the archive of your system from the releases page, with `SHA256SUMS` to check it.

- **Linux:** unpack, then `./install.sh` (installs in `~/.local`, or in the prefix you give).
- **Windows:** unpack the zip and run `flight-replay.exe`.
- **macOS:** unpack the zip and open `Flight Replay.app`.

The releases are not signed, so Windows and macOS warn the first time: see [docs/build.md](docs/build.md#releases).

Flight Replay needs **FFmpeg 6 or newer** (`ffmpeg` and `ffprobe`), which it does not include:

```sh
sudo apt install ffmpeg      # Debian, Ubuntu (dnf, pacman and the others work too)
brew install ffmpeg          # macOS
winget install Gyan.FFmpeg   # Windows
```

## Use

1. **Welcome.** It shows where FFmpeg was found and which encoders work. Choose the colour scheme. "Reset all settings…"
   brings every setting back to the default, after asking you to confirm.
2. **Input.** Drop the video on the window, or choose it. The `.osd` and `.srt` with the same name next to it are
   opened by themselves. Choose the OSD font: one of the list ("Built in"), or the file for your firmware, a
   `WS_..._24.png` sheet such as `WS_INAV_Europa_24.png`. The program carries the default font of INAV and a Betaflight
   font, each for 720p and 1080p videos; the one that suits the video is used. The step warns when the font is for
   another firmware than the `.osd` file. The fonts of the goggles' makers and of other designers are not included,
   as their licenses do not allow it, so use the file for those. What you choose is remembered.
3. **OSD.** Check the position on the preview, centre it, mask elements by clicking them, and correct the timing. Play
   ten seconds of the video to see the OSD in motion.
4. **SRT data.** Choose the fields and where they go. Play ten seconds of the video to see them in motion.
5. **Motor sound.** See what the recording gives the sound, choose the kind of motor and propeller, change the
   parameters, and listen to a moment of the flight: the picture and the OSD follow the sound, so you can check the
   timing by eye.
6. **Output.** Encoder, bitrate, 1440p, file name. You can render only a stretch first, as a quick test.
7. **Render.** The summary and the progress. The queue is here too. Tick "Show the FFmpeg command before rendering"
   to see, and copy, the commands that will run; it is off until you ask for it.

While a preview plays, the picture is shown at reduced quality (smaller, at most 30 frames per second) so that it keeps
up. The video you render is not affected.

The preview of steps 3, 4 and 5 shows the OSD and the SRT data together, as the render will draw them. To see only
the layer of the step, untick "Also show the SRT data in the preview" (step 3) or "Also show the OSD in the preview"
(step 4); in step 5, "Also show the SRT and OSD data in the preview" adds or removes both. It only changes the preview.

A video can also be given on the command line: `flight-replay video.mp4`.

**What is remembered.** Every choice on the screens is saved as soon as you make it, and is there the next time you
open the program: the colour scheme, the folder of FFmpeg if you chose one, the OSD font, what to draw, the position
and the hidden elements, the timing of the OSD, the SRT fields, the sound (the parameters, the format, the saved
sounds and the one chosen), the encoder and the bitrates, the other options of the output, "Replace the file if it
exists", the start and the length of the stretch, and the options of the previews. Not remembered, on purpose: the
video and the files that go with it, the name of the output file (it follows the video), "Render only a stretch"
(it is off whenever the program opens, so that nothing is rendered short by mistake), the mode for hiding elements
by clicking, and the name typed to save a sound before you press Save.

Nothing leaves your computer. The application makes no network requests.

## Build from source

```sh
make build      # debug build
make run        # run the application
make check      # formatting, lints, tests and the dependency policy
make dist       # the archive of this system
```

The requirements for each system, the targets, the tests and the releases are in [docs/build.md](docs/build.md).
How the code is organised is in [docs/architecture.md](docs/architecture.md). The plan and the state of the work are
in [PLAN.md](PLAN.md). The formats are described in [docs/formats](docs/formats).

## License

Flight Replay is free software, licensed under the GNU General Public License, version 3 (GPL-3.0-only).
See [LICENSE](LICENSE). The notices for the code it adapts from other projects are in [NOTICE](NOTICE), and those of
the libraries it uses are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). The OSD fonts that come with the program
keep their own licenses (GPL-3.0 for the INAV font, CC BY-SA 4.0 for the Betaflight one): see
[assets/osd-fonts/README.md](assets/osd-fonts/README.md).

## Disclaimer

This project is not affiliated with Walksnail or Caddx.
