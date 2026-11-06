// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Puts the icon and the version information in `flight-replay.exe`, when the build runs on Windows.

fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../assets/icon/flight-replay.ico");
        resource.set("ProductName", "Flight Replay");
        resource.set("FileDescription", "Flight Replay");
        resource.set("LegalCopyright", "Copyright (C) 2026 EricoGR");
        if let Err(error) = resource.compile() {
            println!("cargo::warning=the Windows resources were not added: {error}");
        }
    }
}
