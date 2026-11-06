// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::fmt;

/// Flight controller firmware named in the first bytes of an `.osd` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Firmware {
    /// INAV (`INAV`).
    Inav,
    /// Betaflight (`BTFL`).
    Betaflight,
    /// ArduPilot (`ARDU`).
    ArduPilot,
    /// KISS (`KISS`).
    Kiss,
    /// KISS Ultra (`ULTR`).
    KissUltra,
    /// Any other four-letter code, kept as written.
    Other(String),
}

impl Firmware {
    /// Reads the four-byte code at the start of an `.osd` file.
    #[must_use]
    pub fn from_code(code: &[u8]) -> Self {
        match code {
            b"INAV" => Self::Inav,
            b"BTFL" => Self::Betaflight,
            b"ARDU" => Self::ArduPilot,
            b"KISS" => Self::Kiss,
            b"ULTR" => Self::KissUltra,
            other => {
                let text = String::from_utf8_lossy(other)
                    .trim_matches(|c: char| c == '\0' || c.is_whitespace())
                    .to_owned();
                Self::Other(text)
            }
        }
    }
}

impl fmt::Display for Firmware {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inav => f.write_str("INAV"),
            Self::Betaflight => f.write_str("Betaflight"),
            Self::ArduPilot => f.write_str("ArduPilot"),
            Self::Kiss => f.write_str("KISS"),
            Self::KissUltra => f.write_str("KISS Ultra"),
            Self::Other(code) if code.is_empty() => f.write_str("unknown"),
            Self::Other(code) => write!(f, "unknown ({code})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_the_firmware_codes() {
        assert_eq!(Firmware::from_code(b"INAV"), Firmware::Inav);
        assert_eq!(Firmware::from_code(b"BTFL"), Firmware::Betaflight);
        assert_eq!(Firmware::from_code(b"ARDU"), Firmware::ArduPilot);
    }

    #[test]
    fn keeps_unknown_codes_readable() {
        assert_eq!(Firmware::from_code(b"XY\0\0"), Firmware::Other("XY".into()));
        assert_eq!(Firmware::from_code(&[0, 0, 0, 0]).to_string(), "unknown");
    }
}
