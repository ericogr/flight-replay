# OSD font sheets

Font sheets for the OSD, in the format the goggles use and Flight Replay reads (see
[docs/formats/font.md](../../docs/formats/font.md)). Only fonts whose license allows redistribution are here. The
sheets are data files, and each set keeps its own license. They are compiled into the program (see
`crates/core/src/font/bundled.rs`), which offers them in a list, and the release archives carry their license texts.
Adding a font here is not enough to offer it: it must also be listed in `bundled.rs`.

| File | Firmware | Cell | Glyphs | License |
|------|----------|------|--------|---------|
| `inav/WS_INAV_Default_24.png` | INAV | 24 × 36 (720p) | 512 | GPL-3.0 |
| `inav/WS_INAV_Default_36.png` | INAV | 36 × 54 (1080p) | 512 | GPL-3.0 |
| `betaflight/WS_BTFL_Material_24.png` | Betaflight | 24 × 36 (720p) | 256 | CC-BY-SA 4.0 |
| `betaflight/WS_BTFL_Material_36.png` | Betaflight | 36 × 54 (1080p) | 256 | CC-BY-SA 4.0 |

Flight Replay scales a sheet to the size of the video, so a 1080p sheet also serves the other sizes, with some loss
of sharpness when it is scaled up.

## INAV: default font

- **Source:** the default HD font of the INAV Configurator, `resources/osd/digital/default`,
  <https://github.com/iNavFlight/inav-configurator>, commit `95504dcea167685e2f9adb08f5f17d6ff1a7462f`
  (2026-09-20). The images were drawn by the INAV project (Darren Lines, "MrD-RC", and other contributors).
- **License:** GPL-3.0, the license of the INAV Configurator. It has no separate notice for the images. This is also
  the license of Flight Replay, so the text is the `LICENSE` file at the root.
- **Changes:** none to the images. They are arranged in one column of 512 cells, the way the goggles read them.
- **Not included:** the 12 × 18 size, which INAV says is not used, and which Flight Replay does not read.

### How the sheets were made

With the HD OSD Font Tool of the same author ([MrD-RC/hdosd-font-tool](https://github.com/MrD-RC/hdosd-font-tool),
GPL-3.0, commit `47e9dcf7bb924028287a4ed57ca0a8d2313d63ac`), which the INAV README names for this. The tool is only
used here; none of its code is in this repository.

```sh
git clone --depth 1 https://github.com/iNavFlight/inav-configurator
mkdir fontroot && cp -r inav-configurator/resources/osd/digital/default fontroot/default
hdosd-font-tool -p fontroot -n WS_INAV_     # writes WS_INAV__default_24.png and WS_INAV__default_36.png
```

Two things about the tool on Linux, found while making these files:

- It does not compile with a recent g++ unless `<cstring>` is included (`g++ -include cstring ...`).
- It reads the character images in the order the file system lists them, which is the name order only on Windows.
  On Linux the order is arbitrary and the sheet comes out with 256 cells instead of 512, with characters missing.
  The files here were made with a copy of the tool that sorts the file names before reading them
  (`std::sort` over `fs::directory_iterator` in `capturePNGCharacters`). The result was checked: 512 cells, and every
  cell that comes from a single image, or from an image of two characters, has the same pixels as that image.

## Betaflight: Material OSD

- **Source:** Material OSD by Knifa, <https://github.com/Knifa/material-osd>, release v0.0.4 (2023-01-21, tag commit
  `6483fe6ba0e509d28c5a921719ba308608ae9796`), the Walksnail package `material-osd-ws-v0.0.4.zip`, files
  `user_bf_24.png` and `user_bf_36.png`.
- **License:** [Creative Commons Attribution-ShareAlike 4.0](https://creativecommons.org/licenses/by-sa/4.0/)
  ([text](licenses/CC-BY-SA-4.0.txt)). The author says so in the README of the project: the generator is GPL-3.0 and
  the generated fonts are CC BY-SA 4.0.
- **Made from:** [Material Design Icons](https://pictogrammers.com/library/mdi/) by Pictogrammers (Apache-2.0,
  [text](licenses/Apache-2.0.txt)) and Bebas Neue, Copyright 2010 Dharma Type (SIL Open Font License 1.1,
  [text](licenses/BebasNeue-OFL.txt)).
- **Changes:** none to the pixels. The files were renamed to say the firmware (`user_bf_24.png` is
  `WS_BTFL_Material_24.png`).
- **Limit:** one page of 256 glyphs. Betaflight 4.5 can use three more pages for coloured warnings; those glyphs are
  not in this sheet.

The CC BY-SA terms cover these two files, not the program. If you change them and share the result, share it under
the same license and say what you changed.

## Checksums (SHA-256)

```
9132e5aeedf1034ff0984848ab4e947a37808fef7bd1b1944ab03becd7bbc109  inav/WS_INAV_Default_24.png
25353cf1aee26063a87255cd5502a4113fd77e3e028d79dbdc4b23645d31cb01  inav/WS_INAV_Default_36.png
ec2196b87528699b67f19898a04cfb690edd820093cb2fecb1c6b29572755da7  betaflight/WS_BTFL_Material_24.png
3f27db1f893376b42955ea5f4c0f58490f68f1cfa49c2c11ba2ad7d21b0993b4  betaflight/WS_BTFL_Material_36.png
```

## Fonts that are not here, and why

Do not add a font to this folder unless its license, or a written permission to this project, allows redistribution.

- **SNEAKY_FPV fonts** (Unify, Europa, Sphere and others): free for personal use only. Use outside that needs the
  author's permission. The `msp-osd` and ArduPilot projects carry copies; their permission does not pass to this
  repository.
- **Fonts in the ArduPilot repository** (`libraries/AP_OSD/fonts/HDFonts`): they are added there without an author or
  a license of their own, and most are the SNEAKY_FPV fonts.
- **Walksnail and Caddx fonts** (the ones inside the goggles' firmware): the manufacturer's files, with no public
  license.
- **WS-Neue**: the author gives a joke license ("Let's say CC-BY-SA 4.0"), and the repository has no finished sheet.
- **hdzero-osd-font-library**: no license.
