// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

/// A snapshot of an encode in progress, from FFmpeg's `-progress` output.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Progress {
    /// Video frames written so far.
    pub frame: u64,
    /// Position in the output, in seconds.
    pub out_time_s: f64,
    /// True for the last snapshot, when FFmpeg reports `progress=end`.
    pub finished: bool,
}

/// Reads one line of `-progress` output into `current`. Returns a snapshot when a block ends.
///
/// FFmpeg writes `key=value` lines, and each block ends with a `progress=` line.
pub fn parse_progress_line(line: &str, current: &mut Progress) -> Option<Progress> {
    let (key, value) = line.split_once('=')?;
    let value = value.trim();
    match key.trim() {
        "frame" => {
            if let Ok(frame) = value.parse() {
                current.frame = frame;
            }
        }
        "out_time_us" => {
            // FFmpeg writes N/A before the first sample.
            if let Ok(us) = value.parse::<i64>() {
                current.out_time_s = us as f64 / 1_000_000.0;
            }
        }
        "progress" => {
            current.finished = value == "end";
            return Some(*current);
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_snapshot_at_the_end_of_each_block() {
        let mut current = Progress::default();
        let lines = [
            "frame=120",
            "out_time_us=2000000",
            "speed=1.5x",
            "progress=continue",
            "frame=180",
            "out_time_us=3000000",
            "progress=end",
        ];
        let snapshots: Vec<Progress> = lines
            .iter()
            .filter_map(|line| parse_progress_line(line, &mut current))
            .collect();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].frame, 120);
        assert!((snapshots[0].out_time_s - 2.0).abs() < 1e-9);
        assert!(!snapshots[0].finished);
        assert_eq!(snapshots[1].frame, 180);
        assert!(snapshots[1].finished);
    }

    #[test]
    fn ignores_not_available_times() {
        let mut current = Progress::default();
        assert_eq!(parse_progress_line("out_time_us=N/A", &mut current), None);
        assert_eq!(current.out_time_s, 0.0);
    }
}
