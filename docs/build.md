# Building Flight Replay

## What you need

| | Linux | Windows | macOS |
|---|---|---|---|
| Rust | `rustup`. The toolchain in `rust-toolchain.toml` is installed by itself the first time you build. | same | same |
| C toolchain | `build-essential` (or your distribution's equivalent) | Visual Studio Build Tools, with the "Desktop development with C++" part | Xcode command line tools (`xcode-select --install`) |
| GNU Make | `make` | `choco install make` | comes with the command line tools |
| Sound (`rodio`) | ALSA headers and `pkg-config` | nothing | nothing |
| FFmpeg 6 or newer | needed to run the application and the tests that render | same | same |

Install the Linux packages with:

```sh
# Debian, Ubuntu
sudo apt install build-essential pkg-config libasound2-dev ffmpeg
# Fedora
sudo dnf install gcc pkgconf-pkg-config alsa-lib-devel ffmpeg
# Arch
sudo pacman -S base-devel pkgconf alsa-lib ffmpeg
```

To run on Linux you also need a Vulkan or OpenGL driver for the window, and `xdg-desktop-portal` with a back end for
your desktop, which is what opens the file dialogs. The sound preview needs ALSA or PipeWire.

Install FFmpeg on the other systems with `winget install Gyan.FFmpeg` (Windows) or `brew install ffmpeg` (macOS).
Flight Replay looks for `ffmpeg` and `ffprobe` in this order: the folder chosen in the application (or the
`FLIGHT_REPLAY_FFMPEG` variable), `PATH`, the usual install folders (`/opt/homebrew/bin`, `/usr/local/bin`,
`C:\ffmpeg\bin`, the WinGet links folder), and the folder of the executable.

## The Makefile

`make help` lists the targets.

| Target | What it does |
|---|---|
| `build` | Debug build of all crates |
| `release` | Optimised build of the application |
| `run` | Runs the application. `make run ARGS="video.mp4"` opens a video |
| `test` | The tests. Those that need FFmpeg print a message and pass when it is not installed |
| `fmt`, `fmt-check` | Format the code, or fail if it is not formatted |
| `lint` | Clippy, with warnings as errors |
| `deny` | Licenses, advisories and sources of the dependencies (`cargo-deny`) |
| `check` | All of the above. Run it before each commit |
| `dist` | Archive of the release build for this system, and `SHA256SUMS`, in `dist/`. `make dist SOURCE=1` adds the source archive |
| `app-bundle` | macOS: writes `dist/Flight Replay.app` |
| `notices` | Rewrites `THIRD_PARTY_NOTICES.md` from the dependencies (needs `cargo install cargo-about --features cli`) |
| `golden` | Rewrites the golden images of the overlay tests. Look at the pictures before you commit them |
| `fixtures` | Cuts the test clip from a local recording (see `tests/fixtures/README.md`) |
| `install` | `cargo install` of the application |
| `clean` | Removes the build output and `dist/` |

Windows: `make` needs a shell. The packaging itself is plain Cargo (`cargo build --release -p flight-replay`, then
`cargo xtask dist`), which is what the release workflow runs.

## Tests

`make test` runs the unit tests, the tests with the real FFmpeg on the fixture clip, the golden images of the overlay,
and the tests of the wizard. The wizard tests need no desktop and no GPU: iced's simulator drives the real `update`
and `view` and draws with the software renderer (`ICED_TEST_BACKEND` in `.cargo/config.toml`), and the render tests run
the whole pipeline. `FLIGHT_REPLAY_SNAPSHOTS=<folder> make test` saves a picture of every
step the wizard tests go through, which is the way to look at a layout change.

Variables that the tests read:

| Variable | Meaning |
|---|---|
| `FLIGHT_REPLAY_SAMPLES` | Folder with real goggle recordings. Tests that use them skip when it is not set |
| `FLIGHT_REPLAY_MOTORSOUND` | Path of the `fpv-motorsound` binary, to compare the sound byte for byte |
| `FLIGHT_REPLAY_DECODER_SAMPLES` | Reference CSV of `fpv-osd-decoder`, to compare the telemetry |
| `FLIGHT_REPLAY_UPDATE_GOLDEN` | Set to `1` by `make golden` |

## Releases

A tag `vX.Y.Z` runs `.github/workflows/release.yml`. Each system builds and packs its own archive (`make dist`), and a
last job adds the source archive and `SHA256SUMS`, then publishes the release. The source archive goes with every
release because the GPL asks for it.

The releases are not signed. The first time you open them:

- **Windows:** SmartScreen says the publisher is unknown. Choose "More info", then "Run anyway".
- **macOS:** Gatekeeper refuses an unsigned application. Right-click `Flight Replay.app`, choose "Open", then "Open"
  again. Or run `xattr -dr com.apple.quarantine "Flight Replay.app"` once.
- **Linux:** run `./install.sh` from the archive to install in `~/.local` (it also takes a prefix), or run
  `flight-replay` from the folder.

Only the Apple Silicon build of macOS is published. On an Intel Mac, build from source.

## FFmpeg is not bundled

Flight Replay starts `ffmpeg` and `ffprobe` as separate programs, and never links to them. The releases do not include
them, for these reasons:

- The license of FFmpeg depends on how it was built. A build with `libx264` is GPL, and shipping one means offering
  its source too. Leaving the choice to the person who installs it keeps the release small and its license plain.
- The hardware encoders (NVENC, QSV, AMF, VideoToolbox, VAAPI) work only with a build that has them and a driver that
  matches the machine. The build of the system's package manager is the one that matches.
- Every system has a maintained FFmpeg package.

If you want a portable folder, put `ffmpeg` and `ffprobe` next to the executable. They are found there.
