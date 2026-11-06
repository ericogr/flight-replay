// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Which telemetry rules apply to a recording, from the firmware named in its header.

use crate::osd::Firmware;

/// The set of decoding rules for one flight controller firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// INAV. The rules are implemented and checked against real recordings.
    Inav,
    /// Betaflight. Its OSD layouts need their own rules, which are not written yet (they need recordings to check
    /// them against).
    Betaflight,
    /// Any other firmware: ArduPilot, KISS and so on.
    Other,
}

/// Whether the telemetry of a recording can be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetrySupport {
    /// The rules exist, so the telemetry (and the sound that follows it) can be used.
    Available,
    /// The firmware is meant to be supported, but its rules are not written yet.
    Planned,
    /// The firmware is not supported.
    Unsupported,
}

impl Profile {
    /// The profile for a firmware.
    #[must_use]
    pub fn from_firmware(firmware: &Firmware) -> Self {
        match firmware {
            Firmware::Inav => Self::Inav,
            Firmware::Betaflight => Self::Betaflight,
            _ => Self::Other,
        }
    }

    /// Whether the telemetry can be read for this profile.
    #[must_use]
    pub fn telemetry_support(self) -> TelemetrySupport {
        match self {
            Self::Inav => TelemetrySupport::Available,
            Self::Betaflight => TelemetrySupport::Planned,
            Self::Other => TelemetrySupport::Unsupported,
        }
    }

    /// A sentence for the user about what the profile allows.
    #[must_use]
    pub fn explanation(self) -> &'static str {
        match self {
            Self::Inav => "INAV: the telemetry and the sound are available.",
            Self::Betaflight => {
                "Betaflight: the OSD and the SRT data can be drawn, but the telemetry rules for Betaflight are not available yet, so there is no sound."
            }
            Self::Other => {
                "This firmware is not supported for telemetry: the OSD and the SRT data can be drawn, but there is no sound."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_inav_has_telemetry_for_now() {
        assert_eq!(
            Profile::from_firmware(&Firmware::Inav).telemetry_support(),
            TelemetrySupport::Available
        );
        assert_eq!(
            Profile::from_firmware(&Firmware::Betaflight).telemetry_support(),
            TelemetrySupport::Planned
        );
        assert_eq!(
            Profile::from_firmware(&Firmware::ArduPilot).telemetry_support(),
            TelemetrySupport::Unsupported
        );
        assert_eq!(
            Profile::from_firmware(&Firmware::Other("XY".into())).telemetry_support(),
            TelemetrySupport::Unsupported
        );
    }
}
