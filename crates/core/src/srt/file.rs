// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::fs;
use std::io;
use std::path::Path;

use super::SrtData;

/// One subtitle of the SRT file: the time it applies to and the data it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct SrtCue {
    /// Start time, in seconds from the start of the video.
    pub start_s: f64,
    /// End time, in seconds.
    pub end_s: f64,
    /// The text of the cue as it is in the file (the lines after the times).
    pub text: String,
    /// The data of the line, or `None` if the line holds no data Flight Replay can read.
    pub data: Option<SrtData>,
}

/// The subtitles of a Walksnail SRT file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SrtFile {
    /// Cues in file order.
    pub cues: Vec<SrtCue>,
}

/// Errors when reading an SRT file.
#[derive(Debug, thiserror::Error)]
pub enum SrtError {
    /// The file could not be read.
    #[error("could not read the SRT file: {0}")]
    Io(#[from] io::Error),
    /// The file has no timed subtitle.
    #[error("the SRT file has no subtitles")]
    NoCues,
}

impl SrtFile {
    /// Reads an SRT file.
    pub fn open(path: &Path) -> Result<Self, SrtError> {
        let text = fs::read_to_string(path).or_else(|error| {
            if error.kind() == io::ErrorKind::InvalidData {
                // Some goggles write the file in a legacy encoding. The data lines are ASCII, so reading the bytes
                // as Latin-1 gives the same numbers.
                fs::read(path).map(|bytes| bytes.iter().map(|&b| char::from(b)).collect())
            } else {
                Err(error)
            }
        })?;
        let file = Self::parse(&text);
        if file.cues.is_empty() {
            return Err(SrtError::NoCues);
        }
        Ok(file)
    }

    /// Parses the text of an SRT file. Blocks that cannot be read are skipped.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut cues = Vec::new();
        let mut skipped = 0usize;
        let mut block: Vec<&str> = Vec::new();
        for line in text.lines().map(str::trim_end).chain(std::iter::once("")) {
            if !line.trim().is_empty() {
                block.push(line);
                continue;
            }
            if !block.is_empty() {
                match parse_block(&block) {
                    Some(cue) => cues.push(cue),
                    None => skipped += 1,
                }
                block.clear();
            }
        }
        if skipped > 0 {
            tracing::warn!(
                skipped,
                "some SRT blocks could not be read and were skipped"
            );
        }
        Self { cues }
    }

    /// Length of the subtitles, in seconds: the end of the last cue.
    #[must_use]
    pub fn duration_s(&self) -> f64 {
        self.cues.iter().map(|cue| cue.end_s).fold(0.0, f64::max)
    }

    /// The cue that applies at `time_s`: the last one that started at or before that time. Before the first cue,
    /// the first one applies.
    #[must_use]
    pub fn cue_at(&self, time_s: f64) -> Option<&SrtCue> {
        let index = self
            .cues
            .partition_point(|cue| cue.start_s <= time_s)
            .saturating_sub(1);
        self.cues.get(index)
    }
}

fn parse_block(lines: &[&str]) -> Option<SrtCue> {
    let arrow = lines.iter().position(|line| line.contains("-->"))?;
    let (start, end) = lines[arrow].split_once("-->")?;
    let start_s = parse_time(start.trim())?;
    let end_s = parse_time(end.trim())?;
    let text = lines[arrow + 1..].join("\n");
    Some(SrtCue {
        start_s,
        end_s,
        data: SrtData::parse(&text),
        text,
    })
}

/// Reads a time written as `HH:MM:SS,mmm` (a point is accepted instead of the comma), in seconds.
fn parse_time(text: &str) -> Option<f64> {
    let (clock, millis) = text.split_once([',', '.'])?;
    let mut parts = clock.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let millis: f64 = millis.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds + millis / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "1\r\n00:00:00,000 --> 00:00:00,150\r\nSignal:4 CH:7 FlightTime:0 SBat:15.9V GBat:7.8V \
        Delay:37ms Bitrate:25.0Mbps Distance:2m\r\n\r\n\
        2\r\n00:00:00,150 --> 00:00:00,300\r\nSignal:4 CH:7 FlightTime:0 SBat:15.9V GBat:7.8V Delay:37ms \
        Bitrate:25.0Mbps Distance:5m\r\n\r\n\
        3\r\n00:00:01.000 --> 00:00:01.150\r\nbroken line without data\r\n";

    #[test]
    fn parses_cues_with_windows_line_endings() {
        let file = SrtFile::parse(SAMPLE);
        assert_eq!(file.cues.len(), 3);
        assert!((file.cues[1].start_s - 0.15).abs() < 1e-9);
        assert_eq!(file.cues[1].data.as_ref().unwrap().distance_m, 5);
        assert!(file.cues[2].data.is_none());
        assert!((file.duration_s() - 1.15).abs() < 1e-9);
    }

    #[test]
    fn picks_the_cue_that_applies_at_a_time() {
        let file = SrtFile::parse(SAMPLE);
        assert!((file.cue_at(0.0).unwrap().start_s).abs() < 1e-9);
        assert!((file.cue_at(0.2).unwrap().start_s - 0.15).abs() < 1e-9);
        assert!((file.cue_at(50.0).unwrap().start_s - 1.0).abs() < 1e-9);
        assert!(SrtFile::default().cue_at(1.0).is_none());
    }

    #[test]
    fn skips_blocks_without_a_time_line() {
        let file = SrtFile::parse("garbage\n\n1\n00:00:00,000 --> 00:00:00,100\nSignal:1\n");
        assert_eq!(file.cues.len(), 1);
    }
}
