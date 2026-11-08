// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The picture of the program. The window icon and the welcome step show the same one, decoded once.

use std::sync::LazyLock;

use iced::widget::image::Handle;

/// The icon at 256 px, built into the executable. The welcome step shows it at half that size, so a screen with a
/// scale of 2 shows it as it is.
const PNG: &[u8] = include_bytes!("../../../assets/icon/flight-replay-256.png");

/// The icon as RGBA pixels. It is built in, so a failure to read it is a mistake of the build and not of the user.
static PICTURE: LazyLock<image::RgbaImage> = LazyLock::new(|| {
    image::load_from_memory_with_format(PNG, image::ImageFormat::Png)
        .expect("the icon built into the program is a PNG")
        .into_rgba8()
});

/// The icon of the window. The window has no icon when the system refuses the pixels.
pub fn window_icon() -> Option<iced::window::Icon> {
    let (width, height) = PICTURE.dimensions();
    iced::window::icon::from_rgba(PICTURE.as_raw().clone(), width, height).ok()
}

/// The icon for an image widget. The handle is made once: a handle that is made again at every view is a new picture
/// for the renderer each time.
pub fn handle() -> Handle {
    static HANDLE: LazyLock<Handle> = LazyLock::new(|| {
        let (width, height) = PICTURE.dimensions();
        Handle::from_rgba(width, height, PICTURE.as_raw().clone())
    });
    HANDLE.clone()
}
