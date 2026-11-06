// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/inav/flightmode.go (Apache-2.0, Copyright 2026 EricoGR).

/// Width of the flight mode element: the OSD shows four characters, padding the short names with blanks.
pub const FLIGHT_MODE_WIDTH: usize = 4;

/// The texts the flight mode element can show, blanks included. ACRO, ANGL and RTH appear in the sample
/// recordings; the others are not checked against any recording yet.
const FLIGHT_MODES: &[&str] = &[
    "ACRO", "ANGL", "ANGH", "HOR ", "MANU", "RTH ", "WRTH", "HOLD", "LOTR", "CRUZ", "CRSH", " WP ",
    " AH ", "GEO ", "TURT", "LAND", "!FS!",
];

/// Whether `text` is exactly what the flight mode element can show: four characters, with the padding blanks.
#[must_use]
pub fn is_flight_mode(text: &str) -> bool {
    FLIGHT_MODES.contains(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_the_modes_with_their_padding() {
        assert!(is_flight_mode("ACRO"));
        assert!(is_flight_mode("RTH "));
        assert!(is_flight_mode(" WP "));
        assert!(!is_flight_mode("RTH"));
        assert!(!is_flight_mode("ACR0"));
    }
}
