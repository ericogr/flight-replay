// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! `cargo xtask fixtures`: makes a test fixture from a recording.
//!
//! The recording stays outside the repository. The fixture is a short clip of the video (stream copy), the `.osd`
//! and `.srt` cut to the same window, and the GPS coordinates of the `.osd` moved to a point in the open ocean.
//!
//! The coordinates are moved by one constant offset, so the track keeps its shape, speed and distances. The offset
//! is the difference between the first real position in the clip and the fake target, so it is computed while the
//! command runs and is never written anywhere. The target itself is fake, and is safe to keep in the repository.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use flight_replay_core::ffmpeg::{Tools, probe};
use flight_replay_core::osd::{Frame, HEADER_BYTES, OsdFile};
use flight_replay_core::srt::SrtFile;
use flight_replay_core::telemetry::catalog::{KEY_GPS_LAT, KEY_GPS_LON};
use flight_replay_core::telemetry::decoder::{Decoder, Options, Sample, Value};
use flight_replay_core::telemetry::inav;
use flight_replay_core::timeline::{SyncOptions, Timeline};

const HELP: &str = "\
Usage: cargo xtask fixtures --source DIR --recording NAME [options]

Cuts DIR/NAME.mp4, DIR/NAME.osd and DIR/NAME.srt into a short fixture, and moves the GPS to open ocean.

Options:
  --source DIR       folder of the recording (required)
  --recording NAME   base name of the recording, such as AvatarG0153 (required)
  --start SECONDS    where the clip starts, in video time (default 120). It must be at a keyframe
  --duration SECONDS length of the clip (default 3)
  --out DIR          where the fixture goes (default tests/fixtures)
  --name NAME        base name of the fixture files (default clip-3s)
  --target-lat DEG   fake latitude of the first position (default -35.2, South Atlantic)
  --target-lon DEG   fake longitude of the first position (default -31.4, South Atlantic)";

struct Args {
    source: PathBuf,
    recording: String,
    start: f64,
    duration: f64,
    out: PathBuf,
    name: String,
    target_lat: f64,
    target_lon: f64,
}

fn parse(args: Vec<String>) -> Result<Option<Args>> {
    let mut source = None;
    let mut recording = None;
    let mut parsed = Args {
        source: PathBuf::new(),
        recording: String::new(),
        start: 120.0,
        duration: 3.0,
        out: PathBuf::from("tests/fixtures"),
        name: "clip-3s".to_owned(),
        target_lat: -35.2,
        target_lon: -31.4,
    };
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        if flag == "--help" || flag == "-h" {
            println!("{HELP}");
            return Ok(None);
        }
        let value = iter
            .next()
            .with_context(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--source" => source = Some(PathBuf::from(value)),
            "--recording" => recording = Some(value),
            "--start" => parsed.start = value.parse().context("--start")?,
            "--duration" => parsed.duration = value.parse().context("--duration")?,
            "--out" => parsed.out = PathBuf::from(value),
            "--name" => parsed.name = value,
            "--target-lat" => parsed.target_lat = value.parse().context("--target-lat")?,
            "--target-lon" => parsed.target_lon = value.parse().context("--target-lon")?,
            other => bail!("unknown option {other:?}\n\n{HELP}"),
        }
    }
    parsed.source = source.context("--source is required")?;
    parsed.recording = recording.context("--recording is required")?;
    ensure!(parsed.duration > 0.0, "--duration must be positive");
    Ok(Some(parsed))
}

pub fn run(args: Vec<String>) -> Result<()> {
    let Some(args) = parse(args)? else {
        return Ok(());
    };
    let tools = Tools::locate(None)?;
    let video = args.source.join(format!("{}.mp4", args.recording));
    let osd_path = args.source.join(format!("{}.osd", args.recording));
    let srt_path = args.source.join(format!("{}.srt", args.recording));
    fs::create_dir_all(&args.out)?;

    // The timeline of the whole recording tells which OSD frames belong to the window.
    let info = probe(&tools, &video).context("probing the source video")?;
    let osd_bytes = fs::read(&osd_path).with_context(|| osd_path.display().to_string())?;
    let osd = OsdFile::parse(&osd_bytes)?;
    let timeline = Timeline::new(SyncOptions::default(), info.duration_s, osd.duration_s());

    let clip = args.out.join(format!("{}.mp4", args.name));
    cut_video(&tools, &video, args.start, args.duration, &clip)?;
    let clip_info = probe(&tools, &clip)?;
    println!(
        "video  : {} frames, {:.3} s, {}x{}",
        clip_info.frame_count, clip_info.duration_s, clip_info.width, clip_info.height
    );

    // OSD: the frames that are current during the window, with times counted from the start of the clip.
    let start_ms = timeline.telemetry_time(args.start) * 1000.0;
    let end_ms = timeline.telemetry_time(args.start + args.duration) * 1000.0;
    let first = osd.frame_index_at(start_ms);
    let original: Vec<Frame> = osd.frames[first..]
        .iter()
        .take_while(|frame| f64::from(frame.time_ms) < end_ms)
        .map(|frame| Frame {
            time_ms: (f64::from(frame.time_ms) - start_ms).max(0.0).round() as u32,
            ..frame.clone()
        })
        .collect();
    ensure!(!original.is_empty(), "no OSD frames in the window");

    let mut anonymized = original.clone();
    let mut mover = GpsMover::new(args.target_lat, args.target_lon);
    for frame in &mut anonymized {
        mover.move_frame(frame)?;
    }
    check_no_plus_code(&anonymized)?;
    verify(&osd, &original, &anonymized, &mover)?;
    println!(
        "osd    : {} frames, {} positions moved to {:.4}, {:.4} (a fake point in the South Atlantic)",
        original.len(),
        mover.moved,
        args.target_lat,
        args.target_lon
    );

    let osd_out = args.out.join(format!("{}.osd", args.name));
    fs::write(
        &osd_out,
        encode_osd(&osd_bytes[..HEADER_BYTES], &anonymized),
    )?;

    let srt_text = fs::read_to_string(&srt_path)
        .or_else(|_| fs::read(&srt_path).map(|b| b.iter().map(|&c| char::from(c)).collect()))?;
    let (srt, cues) = slice_srt(&srt_text, args.start, args.start + args.duration);
    ensure!(cues > 0, "no SRT cues in the window");
    fs::write(args.out.join(format!("{}.srt", args.name)), srt)?;
    println!("srt    : {cues} cues");
    Ok(())
}

fn cut_video(tools: &Tools, video: &Path, start: f64, duration: f64, out: &Path) -> Result<()> {
    let status = Command::new(tools.ffmpeg())
        .args(["-v", "error", "-y", "-ss", &format!("{start}"), "-i"])
        .arg(video)
        .args([
            "-t",
            &format!("{duration}"),
            "-map",
            "0:v:0",
            "-an",
            "-sn",
            "-dn",
            "-c",
            "copy",
        ])
        .args(["-movflags", "+faststart"])
        .arg(out)
        .status()?;
    ensure!(status.success(), "ffmpeg could not cut the clip");
    Ok(())
}

/// Moves the coordinates shown by the OSD by one constant offset.
struct GpsMover {
    target_lat: f64,
    target_lon: f64,
    /// Offsets in degrees. They are known after the first position and never leave this struct.
    offset_lat: Option<f64>,
    offset_lon: Option<f64>,
    moved: usize,
}

impl GpsMover {
    fn new(target_lat: f64, target_lon: f64) -> Self {
        Self {
            target_lat,
            target_lon,
            offset_lat: None,
            offset_lon: None,
            moved: 0,
        }
    }

    /// Finds every latitude and longitude of the frame (a label glyph followed by a number) and moves it.
    fn move_frame(&mut self, frame: &mut Frame) -> Result<()> {
        let columns = frame.columns as usize;
        for row in frame.cells.chunks_mut(columns) {
            for x in 0..columns {
                let label = row[x];
                if label != inav::LABEL_LATITUDE && label != inav::LABEL_LONGITUDE {
                    continue;
                }
                let end = (x + 1..columns)
                    .find(|&i| !is_number_cell(row[i]))
                    .unwrap_or(columns);
                if end > x + 1 {
                    self.move_number(label == inav::LABEL_LATITUDE, &mut row[x + 1..end])?;
                }
            }
        }
        Ok(())
    }

    fn move_number(&mut self, latitude: bool, run: &mut [u16]) -> Result<()> {
        let text = inav::text(run);
        let Ok(value) = text.parse::<f64>() else {
            return Ok(());
        };
        let decimals = text
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len());
        let (target, offset) = if latitude {
            (self.target_lat, &mut self.offset_lat)
        } else {
            (self.target_lon, &mut self.offset_lon)
        };
        let offset = *offset.get_or_insert(target - value);
        let new_text = format!("{:.*}", decimals, value + offset);

        let old_digits = text.chars().filter(char::is_ascii_digit).count();
        let new_digits: Vec<u16> = new_text
            .chars()
            .filter_map(|c| c.to_digit(10))
            .map(|d| d as u16)
            .collect();
        ensure!(
            old_digits == new_digits.len() && text.starts_with('-') == new_text.starts_with('-'),
            "the fake position does not fit the cells of the real one ({} digits, sign {}): choose another target",
            old_digits,
            if text.starts_with('-') { "-" } else { "+" }
        );
        let mut digits = new_digits.into_iter();
        for cell in run.iter_mut() {
            let base = match *cell {
                0x30..=0x39 => 0x30,
                inav::TRAILING_DOT_FIRST..=inav::TRAILING_DOT_LAST => inav::TRAILING_DOT_FIRST,
                inav::LEADING_DOT_FIRST..=inav::LEADING_DOT_LAST => inav::LEADING_DOT_FIRST,
                _ => continue,
            };
            *cell = base + digits.next().expect("the digit counts were checked");
        }
        self.moved += 1;
        Ok(())
    }
}

/// Cells that can be part of a number: the sign, the point, the digits and the digits with half a point.
fn is_number_cell(code: u16) -> bool {
    code == u16::from(b'-')
        || code == u16::from(b'.')
        || (0x30..=0x39).contains(&code)
        || inav::is_half_dot(code)
}

/// The OSD can show the position as a plus code, which would give the real place away.
fn check_no_plus_code(frames: &[Frame]) -> Result<()> {
    for frame in frames {
        for row in frame.cells.chunks(frame.columns as usize) {
            let text: Vec<u8> = row
                .iter()
                .map(|&c| {
                    if (0x20..0x7f).contains(&c) {
                        c as u8
                    } else {
                        b' '
                    }
                })
                .collect();
            let found = text.iter().enumerate().any(|(i, &c)| {
                c == b'+'
                    && i >= 8
                    && text[i - 8..i]
                        .iter()
                        .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
                    && text.get(i + 1).is_some_and(u8::is_ascii_alphanumeric)
            });
            ensure!(
                !found,
                "the OSD shows a plus code, which gives the position away; pick a clip without it"
            );
        }
    }
    Ok(())
}

/// Checks the result: the moved positions are the original ones plus the offset, and nothing else changed.
fn verify(file: &OsdFile, original: &[Frame], changed: &[Frame], mover: &GpsMover) -> Result<()> {
    let decoder = Decoder::new(Options::default());
    let (Some(offset_lat), Some(offset_lon)) = (mover.offset_lat, mover.offset_lon) else {
        println!("osd    : the clip shows no GPS position, so there is nothing to move");
        return Ok(());
    };
    let mut positions = 0;
    for (before, after) in original.iter().zip(changed) {
        let (a, _) = decoder.decode(file, before);
        let (b, _) = decoder.decode(file, after);
        ensure!(
            a.len() == b.len() && a.keys().all(|key| b.contains(key)),
            "the set of readings changed at {} ms",
            before.time_ms
        );
        for (key, value) in a.iter() {
            let shifted = match key {
                KEY_GPS_LAT => Some(offset_lat),
                KEY_GPS_LON => Some(offset_lon),
                _ => None,
            };
            match (value, shifted) {
                (Value::Num(old), Some(offset)) => {
                    let new = number(&b, key)?;
                    ensure!(
                        (new - (old + offset)).abs() < 1e-6,
                        "{key} was not moved by the offset at {} ms",
                        before.time_ms
                    );
                    positions += 1;
                }
                _ => ensure!(
                    Some(value) == b.get(key),
                    "{key} changed at {} ms",
                    before.time_ms
                ),
            }
        }
    }
    ensure!(
        positions > 0,
        "the clip has positions, but none was read back"
    );
    println!(
        "check  : {positions} moved readings equal the original plus the offset; every other reading is unchanged"
    );
    Ok(())
}

fn number(sample: &Sample, key: &str) -> Result<f64> {
    sample
        .number(key)
        .with_context(|| format!("{key} is not a number"))
}

fn encode_osd(header: &[u8], frames: &[Frame]) -> Vec<u8> {
    let mut bytes = header.to_vec();
    for frame in frames {
        bytes.extend_from_slice(&frame.time_ms.to_le_bytes());
        for cell in &frame.cells {
            bytes.extend_from_slice(&cell.to_le_bytes());
        }
    }
    bytes
}

/// Keeps the SRT cues that overlap `[start, end)`, with their times counted from `start`. Returns the text and the
/// number of cues.
fn slice_srt(text: &str, start: f64, end: f64) -> (String, usize) {
    let mut out = String::new();
    let mut count = 0;
    for cue in SrtFile::parse(text)
        .cues
        .iter()
        .filter(|cue| cue.start_s < end && cue.end_s > start)
    {
        count += 1;
        let _ = write!(
            out,
            "{count}\n{} --> {}\n{}\n\n",
            format_time((cue.start_s - start).max(0.0)),
            format_time(cue.end_s - start),
            cue.text
        );
    }
    (out, count)
}

fn format_time(seconds: f64) -> String {
    let ms = (seconds * 1000.0).round() as i64;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_a_position_and_keeps_the_cells_valid() {
        // "LAT -23.5" drawn with plain digits and a plain point
        let text = "-23.5";
        let mut cells = vec![inav::LABEL_LATITUDE];
        cells.extend(text.bytes().map(u16::from));
        cells.extend([0, 0]);
        let mut frame = Frame {
            time_ms: 0,
            columns: cells.len() as u32,
            cells,
        };
        let mut mover = GpsMover::new(-35.2, -31.4);
        mover.move_frame(&mut frame).unwrap();
        let run = &frame.cells[1..6];
        assert_eq!(inav::text(run), "-35.2");
        assert_eq!(mover.moved, 1);
    }

    #[test]
    fn keeps_half_point_digits_in_the_same_cells() {
        // -23.5 where the digits next to the point carry half a point: [-][2][3.][.5]
        let cells = vec![
            inav::LABEL_LONGITUDE,
            u16::from(b'-'),
            u16::from(b'2'),
            inav::TRAILING_DOT_FIRST + 3,
            inav::LEADING_DOT_FIRST + 5,
        ];
        let mut frame = Frame {
            time_ms: 0,
            columns: 5,
            cells,
        };
        let mut mover = GpsMover::new(-35.2, -31.4);
        mover.move_frame(&mut frame).unwrap();
        // -23.5 moved so that it lands on -31.4: [-][3][1.][.4]
        assert_eq!(frame.cells[3], inav::TRAILING_DOT_FIRST + 1);
        assert_eq!(frame.cells[4], inav::LEADING_DOT_FIRST + 4);
        assert_eq!(inav::text(&frame.cells[1..]), "-31.4");
    }

    #[test]
    fn a_target_with_another_number_of_digits_is_refused() {
        let mut cells = vec![inav::LABEL_LATITUDE];
        cells.extend("-5.5".bytes().map(u16::from));
        let columns = cells.len() as u32;
        let mut frame = Frame {
            time_ms: 0,
            columns,
            cells,
        };
        assert!(GpsMover::new(-35.2, -31.4).move_frame(&mut frame).is_err());
    }

    #[test]
    fn the_srt_slice_counts_times_from_the_start_of_the_clip() {
        let text = "1\n00:00:01,000 --> 00:00:01,150\nSignal:4\n\n2\n00:00:02,000 --> 00:00:02,150\nSignal:3\n\n";
        let (out, count) = slice_srt(text, 1.5, 3.0);
        assert_eq!(count, 1);
        assert_eq!(out, "1\n00:00:00,500 --> 00:00:00,650\nSignal:3\n\n");
    }
}
