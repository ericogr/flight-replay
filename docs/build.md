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
`C:\ffmpeg\bin`, the WinGet links folder), the folder of the executable, and last the `ffmpeg` folder in the data
folder of the application. A copy older than version 6 is skipped and the search goes on; when all of them are too
old, step 1 says so. A build made from git, whose version cannot be read, is accepted.

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
| `cross-lint` | The same clippy for the Windows and macOS targets (each needs `rustup target add <target>`; nothing is run). Code gated by `cfg(windows)`, `cfg(unix)` or `cfg(target_os = ...)` is only linted by its own system, so run this after touching any |
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
- It would make each download about 100 MB larger for the people who already have their own FFmpeg, and each security
  fix of FFmpeg would be a release of this project.

### Installing a copy on request

For a person who has no FFmpeg, the program can install one: **Settings, Install FFmpeg**. Nothing is fetched until
that click. The program runs `curl` to get one build for the system, fixed in the code with its address, size and
SHA-256 (`crates/core/src/ffmpeg/install/builds.rs`):

The code is the source of truth (`builds.rs`); this table is a copy of it.

| System | Build | Package |
|---|---|---|
| Windows, x86-64 | gyan.dev, release "essentials" 9.0.2 | one `.zip`, 115 MB |
| macOS, Apple Silicon | ffmpeg.martin-riedl.de, release 9.0.2 | two `.zip` (`ffmpeg`, `ffprobe`), 28 MB each |
| Linux, x86-64 | BtbN FFmpeg-Builds, GPL, automatic build of 2026-10-31 (9.0.2) | one `.tar.xz`, 153 MB |

Each was opened and checked by hand: it is GPL v3, has `libx264` and `libx265`, and was not built with
`--enable-nonfree` (a build with it cannot be redistributed). The program checks the SHA-256 of what it downloaded,
takes out only `ffmpeg` and `ffprobe`, runs both once, and only then puts them in the `ffmpeg` folder of its data
folder (`~/.local/share/flight-replay/ffmpeg` on Linux). It moves the old copy aside to do so, and puts it back if the
swap fails, so a failure or a cancel leaves what was there before. The
folder also gets `ABOUT.txt`, with where the files came from, and the license text when the package has it. The
project does not host or redistribute the builds: the user gets them from the provider, who gives the source.

This copy is the last place searched, so an FFmpeg of your own always wins. To use a newer build, check the new file
the same way (license, `--enable-nonfree`, encoders), then change the address, size and SHA-256 in `builds.rs`. The
providers do not promise to keep their addresses: BtbN keeps the last build of each month for two years, which is why
a month-end build is used; when an address goes away, the install says so and the other ways stay on the same screen.

If you want a portable folder, put `ffmpeg` and `ffprobe` next to the executable. They are found there.
