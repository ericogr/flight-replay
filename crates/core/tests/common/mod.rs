// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Helpers shared by the integration tests.
//!
//! Each test file compiles this module on its own and uses only some of the helpers.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use flight_replay_core::font::{CharacterSize, FontSheet, ScaledFont};
use image::{Rgba, RgbaImage};

/// A file of the fixtures folder of the repository.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

/// A 512-glyph font sheet made for the tests, so no font of the goggles has to be in the repository. Every glyph has
/// a white border and interior stripes that follow the bits of its code, so any two glyphs look different.
pub fn synthetic_font(size: CharacterSize) -> ScaledFont {
    let (width, height) = (size.width(), size.height());
    let mut sheet = RgbaImage::new(width, height * 512);
    for code in 0..512u32 {
        for y in 0..height {
            for x in 0..width {
                let border = x < 2 || y < 2 || x + 2 >= width || y + 2 >= height;
                let pixel = if border {
                    Rgba([255, 255, 255, 255])
                } else {
                    let bit = (code >> ((y / 4) % 9)) & 1;
                    Rgba([255, (bit * 255) as u8, (code % 256) as u8, 210])
                };
                sheet.put_pixel(x, code * height + y, pixel);
            }
        }
    }
    FontSheet::from_image(&sheet)
        .expect("a valid sheet")
        .scaled(size)
}

/// Compares an image with the golden image of the same name in `tests/golden`.
///
/// With `FLIGHT_REPLAY_UPDATE_GOLDEN=1` the golden image is written instead (`make golden`). `tolerance` is the
/// largest difference allowed in any channel of any pixel.
pub fn assert_golden(name: &str, image: &RgbaImage, tolerance: u8) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden")
        .join(name);
    if std::env::var_os("FLIGHT_REPLAY_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image.save(&path).unwrap();
        eprintln!("wrote {}", path.display());
        return;
    }
    let golden = image::open(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (run `make golden` to create it)", path.display()))
        .into_rgba8();
    assert_eq!(golden.dimensions(), image.dimensions(), "size of {name}");
    let worst = golden
        .pixels()
        .zip(image.pixels())
        .flat_map(|(a, b)| a.0.iter().zip(b.0.iter()).map(|(x, y)| x.abs_diff(*y)))
        .max()
        .unwrap_or(0);
    assert!(
        worst <= tolerance,
        "{name} differs from its golden image by up to {worst} (allowed {tolerance})"
    );
}
