// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Core library of Flight Replay.
//!
//! The library has no GUI dependency. The graphical application lives in the `flight-replay` crate and only
//! calls into this library, so a command-line interface can be added later as another crate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Product name, as shown to the user.
pub const APP_NAME: &str = "Flight Replay";

/// Version of the core library, from the package metadata.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_name_is_the_product_name() {
        assert_eq!(APP_NAME, "Flight Replay");
    }

    #[test]
    fn version_comes_from_the_package() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
