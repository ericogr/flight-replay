// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/osddecode/rules*.go (Apache-2.0, Copyright 2026 EricoGR), whose rules come
// from fpv-osd-decoder (MIT, Copyright (c) 2026 EricoGR).

//! The decoding rules, grouped by subject.

mod battery;
mod flight;
mod link;
mod position;
mod sensors;
mod speed;
mod system;
mod time;

use super::rule::Rule;

/// Every rule, grouped by subject. The order matters: a recognized span is erased before the next rule runs, so the
/// more specific rules must come before the generic ones they could collide with. Each group puts its own rules in
/// that order. Between groups, one dependency: the link rules go before the speed rules, because the `MAX` glyph
/// that stands for an RSSI of 100% also starts the maximum speeds.
///
/// Some elements cannot be told apart from another one by glyphs, because the firmware draws them in the same way;
/// the first one on the screen keeps the column (see [`Rule::apply`]).
pub(super) fn build_rules() -> Vec<Rule> {
    let mut rules = Vec::new();
    for group in [
        link::rules(),
        time::rules(),
        battery::rules(),
        flight::rules(),
        position::rules(),
        speed::rules(),
        speed::performance_rules(),
        sensors::rules(),
        system::rules(),
    ] {
        rules.extend(group);
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_expression_compiles() {
        assert!(build_rules().len() > 60);
    }
}
