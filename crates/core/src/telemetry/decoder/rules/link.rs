// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/rules_link.go (Apache-2.0, Copyright 2026 EricoGR).

use super::super::Sample;
use super::super::rule::{
    NOT_WORD_BOUNDARY, NUM_RE, Rule, SPACE_RE, each, first_glyph, int_field, num_field, parse_num,
    rule, sym, syms, text_field,
};
use crate::telemetry::catalog::{
    KEY_LQ_DOWN_PCT, KEY_LQ_PCT, KEY_RF_MODE, KEY_RSSI_ANTENNA, KEY_RSSI_DBM, KEY_RSSI_PCT,
    KEY_RX_BAND, KEY_RX_MODE, KEY_RX_POWER_MW, KEY_SNR_DB, KEY_TX_POWER_MW, KEY_VTX_CHANNEL,
    KEY_VTX_POWER,
};
use crate::telemetry::inav;

/// Reads the radio link (RSSI, link quality, power, receiver) and the video transmitter.
pub(super) fn rules() -> Vec<Rule> {
    let rssi_icons = format!("({})", syms(&[inav::ICON_RSSI, inav::ICON_RSSI_ALT]));
    let rssi_percent_group = format!("(?:([0-9]+)|({}))", sym(inav::LABEL_MAX));
    vec![
        rule(
            &[&rssi_icons, SPACE_RE, NUM_RE, &sym(inav::UNIT_DBM)],
            Box::new(rssi_dbm),
        ),
        // The downlink quality ends with a down arrow; it must go first or the uplink rule below would take it.
        rule(
            &[
                &sym(inav::ICON_LINK_QUALITY),
                SPACE_RE,
                "([0-9]+)",
                &sym(inav::ARROW_DOWN_SMALL),
            ],
            int_field(KEY_LQ_DOWN_PCT, 1),
        ),
        // "n:LQ" (RF mode and quality) or just the quality
        rule(
            &[
                &sym(inav::ICON_LINK_QUALITY),
                SPACE_RE,
                "(?:([0-9]+):)?",
                SPACE_RE,
                "([0-9]+)",
            ],
            each(vec![int_field(KEY_RF_MODE, 1), int_field(KEY_LQ_PCT, 2)]),
        ),
        // RSSI as a percentage. The dBm rule has already taken the elements that show dBm, which start with the
        // same icon.
        rule(
            &[&sym(inav::ICON_RSSI), SPACE_RE, &rssi_percent_group],
            Box::new(rssi_percent),
        ),
        rule(
            &[
                &sym(inav::LABEL_SNR),
                SPACE_RE,
                "(-?[0-9]+)",
                &sym(inav::UNIT_DB),
            ],
            int_field(KEY_SNR_DB, 1),
        ),
        // The downlink power ends with a down arrow and must go before the uplink one.
        rule(
            &[
                NUM_RE,
                &sym(inav::UNIT_MILLIWATT),
                &sym(inav::ARROW_DOWN_SMALL),
            ],
            num_field(KEY_RX_POWER_MW, 1),
        ),
        rule(
            &[NUM_RE, &sym(inav::UNIT_MILLIWATT)],
            num_field(KEY_TX_POWER_MW, 1),
        ),
        rule(
            &[&sym(inav::LABEL_RX_BAND), "([ -~]{1,4})"],
            text_field(KEY_RX_BAND, 1),
        ),
        rule(
            &[&sym(inav::LABEL_RX_MODE), "([ -~]{1,6})"],
            text_field(KEY_RX_MODE, 1),
        ),
        // "CH:", the band letter, the channel number, ":" and the power letter. It goes before the VTX power
        // element, which shows the same letter: if both are on screen the channel must not be lost.
        rule(
            &["CH:([A-Z][0-9A-Z]*):([0-9A-Z]?)"],
            each(vec![
                text_field(KEY_VTX_CHANNEL, 1),
                text_field(KEY_VTX_POWER, 2),
            ]),
        ),
        // The glyph sits on the apostrophe, so it only counts at the start of a word: "DON'T" in a text is not a
        // transmitter power.
        rule(
            &[NOT_WORD_BOUNDARY, &sym(inav::LABEL_VTX_POWER), "([0-9A-Z])"],
            text_field(KEY_VTX_POWER, 1),
        ),
    ]
}

/// Reads the RSSI in dBm (group 2). The icon (group 1) says which antenna is active.
fn rssi_dbm(groups: &[String], sample: &mut Sample) {
    let Some(value) = parse_num(&groups[2]) else {
        return;
    };
    sample.num(KEY_RSSI_DBM, value);
    let antenna = if first_glyph(&groups[1]) == inav::ICON_RSSI_ALT {
        2.0
    } else {
        1.0
    };
    sample.num(KEY_RSSI_ANTENNA, antenna);
}

/// Reads the RSSI in percent: the number in group 1, or the `MAX` glyph in group 2, which stands for 100.
fn rssi_percent(groups: &[String], sample: &mut Sample) {
    if !groups[2].is_empty() {
        sample.num(KEY_RSSI_PCT, 100.0);
        return;
    }
    int_field(KEY_RSSI_PCT, 1)(groups, sample);
}
