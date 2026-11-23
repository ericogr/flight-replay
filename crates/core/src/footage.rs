// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The video of a flight: one file, or several that follow one another.
//!
//! The goggles split a long recording into several files, each with an `.osd` and an `.srt` of its own. [`Footage`]
//! puts the videos on one timeline, so that the rest of the program can treat them as one video: the preview, the
//! sound and the render ask it where a time is (which file, and when in it) and which stretch of which file a
//! stretch of the whole is.
//!
//! The videos follow one another by whole frames: the next one starts on the frame where the one before it ends, so
//! the picture of the output has no gap and no repeat at a join, and the sound, which follows the same starts, stays
//! with it.

use std::path::{Path, PathBuf};

use crate::ffmpeg::{DecodeOptions, VideoInfo};
use crate::osd::JoinPart;
use crate::paths::file_name;
use crate::timeline::{Leg, SyncOptions, Timeline};

/// How much two frame rates may differ and still be the same rate, in frames per second.
const RATE_TOLERANCE: f64 = 0.01;

/// One video file of a flight.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoPart {
    /// The file.
    pub path: PathBuf,
    /// What is known about it.
    pub info: VideoInfo,
}

/// Why videos cannot be put on one timeline.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FootageError {
    /// No video was given.
    #[error("there are no videos")]
    Empty,
    /// The pictures differ in size, so the videos were not recorded by the same setup.
    #[error(
        "{first} is {}x{}, but {other} is {}x{}: they do not belong to the same flight",
        .first_size.0, .first_size.1, .other_size.0, .other_size.1
    )]
    SizeMismatch {
        /// The first video.
        first: String,
        /// Its width and height, in pixels.
        first_size: (u32, u32),
        /// The video that differs.
        other: String,
        /// Its width and height, in pixels.
        other_size: (u32, u32),
    },
}

/// A stretch of one video of a [`Footage`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slice {
    /// Which video, from 0.
    pub part: usize,
    /// Where the stretch starts in that video, in seconds.
    pub start_s: f64,
    /// How long it is, in seconds.
    pub duration_s: f64,
    /// Where it starts on the timeline of the whole, in seconds.
    pub at_s: f64,
}

impl Slice {
    /// How many frames the stretch has at `rate` frames per second. A stretch has at least one.
    #[must_use]
    pub fn frames(&self, rate: f64) -> u64 {
        frames_in(self.duration_s, rate) as u64
    }
}

/// The videos of a flight, one after another.
#[derive(Debug, Clone, PartialEq)]
pub struct Footage {
    /// At least one, in the order they play.
    parts: Vec<VideoPart>,
    /// Where each starts on the timeline of the whole, in seconds.
    starts_s: Vec<f64>,
    /// What the whole looks like: the picture of the first, and the length of all.
    info: VideoInfo,
}

impl Footage {
    /// Puts the videos, in the order they play, on one timeline.
    pub fn new(parts: Vec<VideoPart>) -> Result<Self, FootageError> {
        let first = parts.first().ok_or(FootageError::Empty)?;
        let size = |part: &VideoPart| (part.info.width, part.info.height);
        if let Some(other) = parts.iter().find(|part| size(part) != size(first)) {
            return Err(FootageError::SizeMismatch {
                first: file_name(&first.path),
                first_size: size(first),
                other: file_name(&other.path),
                other_size: size(other),
            });
        }
        Ok(Self::assemble(parts))
    }

    /// A flight of one video.
    #[must_use]
    pub fn single(path: PathBuf, info: VideoInfo) -> Self {
        Self::assemble(vec![VideoPart { path, info }])
    }

    /// Works out the starts and the whole of videos that are known to fit. There is at least one.
    fn assemble(parts: Vec<VideoPart>) -> Self {
        let starts_s = starts(&parts);
        let info = whole(&parts, &starts_s);
        Self {
            parts,
            starts_s,
            info,
        }
    }

    /// The videos, in the order they play. There is always at least one.
    #[must_use]
    pub fn parts(&self) -> &[VideoPart] {
        &self.parts
    }

    /// The file of the video `part`.
    #[must_use]
    pub fn path(&self, part: usize) -> &Path {
        &self.parts[part].path
    }

    /// The first video, which names the flight.
    #[must_use]
    pub fn first(&self) -> &VideoPart {
        &self.parts[0]
    }

    /// What the whole looks like: the codec, the picture and the rate of the first video, and the length and the
    /// number of frames of all of them. With one video, what is known about it.
    #[must_use]
    pub fn info(&self) -> &VideoInfo {
        &self.info
    }

    /// Where the video `part` starts on the timeline of the whole, in seconds.
    #[must_use]
    pub fn start_s(&self, part: usize) -> f64 {
        self.starts_s[part]
    }

    /// The time of the last frame, in seconds. A picture cannot be made at the very end of the video, where no frame
    /// starts, so this is the last time a preview can show.
    #[must_use]
    pub fn last_frame_s(&self) -> f64 {
        (self.info.duration_s - 1.0 / self.info.nominal_frame_rate).max(0.0)
    }

    /// The video that holds `time_s` and the time in it, in seconds. A time before the start is the start of the
    /// first video, and one after the end is the end of the last.
    #[must_use]
    pub fn locate(&self, time_s: f64) -> (usize, f64) {
        let index = self
            .starts_s
            .partition_point(|&start| start <= time_s)
            .saturating_sub(1);
        let local = (time_s - self.starts_s[index]).clamp(0.0, self.parts[index].info.duration_s);
        (index, local)
    }

    /// The stretches of the videos that make up the stretch of the whole that starts at `start_s` and lasts
    /// `duration_s` (to the end when `None`), in order. A stretch is cut to the whole.
    #[must_use]
    pub fn slices(&self, start_s: f64, duration_s: Option<f64>) -> Vec<Slice> {
        let total = self.info.duration_s;
        let start = start_s.clamp(0.0, total);
        let end = duration_s.map_or(total, |duration| (start + duration).min(total));
        (0..self.parts.len())
            .filter_map(|part| {
                let from = self.starts_s[part];
                // A video ends where the next one starts, which is on a frame.
                let to = self.starts_s.get(part + 1).copied().unwrap_or(total);
                let (low, high) = (start.max(from), end.min(to));
                (high - low > 1e-9).then_some(Slice {
                    part,
                    start_s: low - from,
                    duration_s: high - low,
                    at_s: low,
                })
            })
            .collect()
    }

    /// One [`Leg`] for each video, with the recording that goes with it: `osd` says where each `.osd` file sits on the
    /// joined timeline of the recordings (see [`crate::osd::join`]). It is one for each video, or any number for a
    /// single video that was recorded in several files. `None` when the recordings do not fit the videos.
    #[must_use]
    pub fn legs(&self, osd: &[JoinPart]) -> Option<Vec<Leg>> {
        // The first recording owns everything before it, so a leg starts at the start of the timeline, as a single
        // video always has.
        let spans: Vec<(f64, f64)> = if osd.len() == self.parts.len() {
            osd.iter()
                .enumerate()
                .map(|(index, part)| (if index == 0 { 0.0 } else { part.start_s }, part.end_s))
                .collect()
        } else if let (1, Some(last)) = (self.parts.len(), osd.last()) {
            vec![(0.0, last.end_s)]
        } else {
            return None;
        };
        Some(
            self.parts
                .iter()
                .zip(&self.starts_s)
                .zip(spans)
                .map(|((part, &video_start_s), (osd_start_s, osd_end_s))| Leg {
                    video_start_s,
                    video_duration_s: part.info.duration_s,
                    osd_start_s,
                    osd_duration_s: osd_end_s - osd_start_s,
                })
                .collect(),
        )
    }

    /// How the frames of a stretch of one video are decoded at `rate` frames per second: at the same rate for every
    /// video, so that frame `n` of the stretch is at time `n / rate`. The picture is not scaled.
    #[must_use]
    pub fn decode_options(&self, slice: &Slice, rate: f64) -> DecodeOptions {
        DecodeOptions {
            width: self.info.width,
            height: self.info.height,
            start_s: slice.start_s,
            duration_s: Some(slice.duration_s),
            constant_rate: Some(rate),
            scale_to: None,
        }
    }

    /// The mapping between the time of the video and the time of the recordings, or `None` when the recordings do
    /// not fit the videos (see [`Footage::legs`]).
    #[must_use]
    pub fn timeline(&self, options: SyncOptions, osd: &[JoinPart]) -> Option<Timeline> {
        self.legs(osd).map(|legs| Timeline::joined(options, &legs))
    }

    /// What the videos do not have in common that stops them being joined without encoding them again: the codec
    /// and the rate. Each is a sentence, naming the video.
    #[must_use]
    pub fn differences(&self) -> Vec<String> {
        let mut differences = self.codec_differences();
        differences.extend(self.rate_differences());
        differences
    }

    /// The videos that have another codec than the first.
    fn codec_differences(&self) -> Vec<String> {
        let first = self.first();
        self.parts[1..]
            .iter()
            .filter(|part| part.info.codec != first.info.codec)
            .map(|part| {
                format!(
                    "{} is {} and {} is {}",
                    file_name(&first.path),
                    first.info.codec,
                    file_name(&part.path),
                    part.info.codec
                )
            })
            .collect()
    }

    /// The videos that run at another rate than the first.
    #[must_use]
    pub fn rate_differences(&self) -> Vec<String> {
        let first = self.first();
        self.parts[1..]
            .iter()
            .filter(|part| {
                (part.info.nominal_frame_rate - first.info.nominal_frame_rate).abs()
                    > RATE_TOLERANCE
            })
            .map(|part| {
                format!(
                    "{} runs at {:.2} fps and {} at {:.2} fps",
                    file_name(&first.path),
                    first.info.nominal_frame_rate,
                    file_name(&part.path),
                    part.info.nominal_frame_rate
                )
            })
            .collect()
    }
}

/// How many frames a stretch of `duration_s` seconds has at `rate` frames per second. A stretch has at least one.
fn frames_in(duration_s: f64, rate: f64) -> f64 {
    (duration_s * rate).round().max(1.0)
}

/// Where each video starts: after a whole number of frames of the ones before it.
fn starts(parts: &[VideoPart]) -> Vec<f64> {
    let rate = parts[0].info.nominal_frame_rate;
    let mut at = 0.0;
    parts
        .iter()
        .map(|part| {
            let start = at;
            at += if rate.is_finite() && rate > 0.0 {
                frames_in(part.info.duration_s, rate) / rate
            } else {
                part.info.duration_s
            };
            start
        })
        .collect()
}

/// What the videos are, taken as one.
fn whole(parts: &[VideoPart], starts_s: &[f64]) -> VideoInfo {
    let first = &parts[0].info;
    if parts.len() == 1 {
        return first.clone();
    }
    let last = parts.len() - 1;
    VideoInfo {
        duration_s: starts_s[last] + parts[last].info.duration_s,
        frame_count: parts.iter().map(|part| part.info.frame_count).sum(),
        audio_streams: parts
            .iter()
            .map(|part| part.info.audio_streams)
            .max()
            .unwrap_or(0),
        audio_duration_s: None,
        ..first.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(duration_s: f64, rate: f64) -> VideoInfo {
        VideoInfo {
            codec: "h264".into(),
            width: 1280,
            height: 720,
            frame_rate: rate,
            nominal_frame_rate: rate,
            duration_s,
            frame_count: (duration_s * rate).round() as u64,
            audio_streams: 0,
            audio_duration_s: None,
        }
    }

    fn part(name: &str, duration_s: f64) -> VideoPart {
        VideoPart {
            path: PathBuf::from(name),
            info: info(duration_s, 60.0),
        }
    }

    /// Three videos: 10 s, 10.004 s (a frame and a quarter more than 10 s at 60 fps) and 5 s.
    fn three() -> Footage {
        Footage::new(vec![
            part("a.mp4", 10.0),
            part("b.mp4", 10.004),
            part("c.mp4", 5.0),
        ])
        .unwrap()
    }

    #[test]
    fn one_video_is_the_footage_of_one_video() {
        let single = Footage::single(PathBuf::from("a.mp4"), info(12.5, 60.0));
        assert_eq!(single.info(), &info(12.5, 60.0));
        assert_eq!(single.parts().len(), 1);
        assert_eq!(single.start_s(0), 0.0);
        assert_eq!(single, Footage::new(vec![part("a.mp4", 12.5)]).unwrap());
    }

    #[test]
    fn the_last_time_that_can_be_shown_is_the_last_frame() {
        let footage = |duration_s| Footage::single(PathBuf::from("a.mp4"), info(duration_s, 60.0));
        assert!((footage(3.0).last_frame_s() - (3.0 - 1.0 / 60.0)).abs() < 1e-12);
        assert_eq!(footage(0.0).last_frame_s(), 0.0, "a video with no length");
        // Of several videos, the last frame of the last one.
        assert!((three().last_frame_s() - (25.0 - 1.0 / 60.0)).abs() < 1e-12);
    }

    #[test]
    fn a_video_starts_on_the_frame_where_the_one_before_it_ends() {
        let footage = three();
        assert_eq!(footage.start_s(0), 0.0);
        assert!((footage.start_s(1) - 10.0).abs() < 1e-9);
        // 10.004 s is 600.24 frames: the join is at frame 600, and the next video starts there.
        assert!((footage.start_s(2) - 20.0).abs() < 1e-9);
        // The whole ends where the last video ends.
        assert!((footage.info().duration_s - 25.0).abs() < 1e-9);
        assert_eq!(footage.info().frame_count, 600 + 600 + 300);
    }

    #[test]
    fn a_time_is_in_the_video_that_holds_it() {
        let footage = three();
        assert_eq!(footage.locate(0.0), (0, 0.0));
        assert_eq!(footage.locate(9.99).0, 0);
        let (index, local) = footage.locate(12.5);
        assert_eq!(index, 1);
        assert!((local - 2.5).abs() < 1e-9);
        assert_eq!(footage.locate(-3.0), (0, 0.0));
        let (index, local) = footage.locate(99.0);
        assert_eq!(index, 2);
        assert!((local - 5.0).abs() < 1e-9, "never past the end");
    }

    #[test]
    fn the_whole_is_cut_into_the_stretches_of_its_videos() {
        let footage = three();
        let all = footage.slices(0.0, None);
        assert_eq!(all.iter().map(|s| s.part).collect::<Vec<_>>(), [0, 1, 2]);
        // The frames add up to those of the whole, with none lost or repeated at a join.
        let frames: u64 = all.iter().map(|s| s.frames(60.0)).sum();
        assert_eq!(frames, 1500);

        // A stretch from the middle of the second video to the middle of the third.
        let cut = footage.slices(15.0, Some(7.5));
        assert_eq!(cut.len(), 2);
        assert_eq!(cut[0].part, 1);
        assert!((cut[0].start_s - 5.0).abs() < 1e-9);
        assert!((cut[0].duration_s - 5.0).abs() < 1e-9);
        assert!((cut[0].at_s - 15.0).abs() < 1e-9);
        assert_eq!(cut[1].part, 2);
        assert_eq!(cut[1].start_s, 0.0);
        assert!((cut[1].duration_s - 2.5).abs() < 1e-9);
        assert!((cut[1].at_s - 20.0).abs() < 1e-9);

        // A stretch inside one video, and one that runs past the end.
        assert_eq!(footage.slices(1.0, Some(2.0)).len(), 1);
        let tail = footage.slices(23.0, Some(100.0));
        assert_eq!(tail.len(), 1);
        assert!((tail[0].duration_s - 2.0).abs() < 1e-9);
        assert_eq!(footage.slices(30.0, None), vec![]);
    }

    #[test]
    fn a_stretch_is_read_from_its_own_file_at_the_rate_it_is_given() {
        let footage = three();
        let slice = footage.slices(15.0, Some(7.5))[0];
        assert_eq!(footage.path(slice.part), Path::new("b.mp4"));
        let options = footage.decode_options(&slice, 30.0);
        assert_eq!((options.width, options.height), (1280, 720));
        assert!((options.start_s - 5.0).abs() < 1e-9);
        assert_eq!(options.duration_s, Some(slice.duration_s));
        assert_eq!(options.constant_rate, Some(30.0));
        assert_eq!(options.scale_to, None);
    }

    #[test]
    fn a_single_video_is_cut_the_way_a_stretch_always_was() {
        let single = Footage::single(PathBuf::from("a.mp4"), info(100.0, 60.0));
        let slices = single.slices(90.0, Some(30.0));
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].start_s, 90.0);
        assert_eq!(slices[0].duration_s, 10.0);
        assert_eq!(slices[0].frames(60.0), 600);
    }

    #[test]
    fn videos_of_another_size_are_refused() {
        let mut other = part("b.mp4", 10.0);
        other.info.width = 1920;
        other.info.height = 1080;
        let error = Footage::new(vec![part("a.mp4", 10.0), other]).unwrap_err();
        assert!(
            matches!(error, FootageError::SizeMismatch { .. }),
            "{error}"
        );
        assert!(
            error
                .to_string()
                .contains("a.mp4 is 1280x720, but b.mp4 is 1920x1080"),
            "{error}"
        );
        assert_eq!(Footage::new(Vec::new()), Err(FootageError::Empty));
    }

    #[test]
    fn what_stops_a_join_without_encoding_is_named() {
        assert_eq!(three().differences(), Vec::<String>::new());
        let mut other = part("b.mp4", 10.0);
        other.info.codec = "hevc".into();
        other.info.nominal_frame_rate = 30.0;
        let footage = Footage::new(vec![part("a.mp4", 10.0), other]).unwrap();
        let differences = footage.differences();
        assert_eq!(differences.len(), 2);
        assert!(differences[0].contains("a.mp4 is h264 and b.mp4 is hevc"));
        assert!(differences[1].contains("60.00 fps") && differences[1].contains("30.00 fps"));
    }

    fn recording(name: &str, start_s: f64, end_s: f64) -> JoinPart {
        JoinPart {
            name: name.into(),
            start_s,
            end_s,
            frames: 10,
        }
    }

    #[test]
    fn each_video_has_the_recording_that_goes_with_it() {
        let footage = three();
        let legs = footage
            .legs(&[
                recording("a.osd", 0.0, 9.5),
                recording("b.osd", 9.5, 19.0),
                recording("c.osd", 19.0, 24.0),
            ])
            .unwrap();
        assert_eq!(legs.len(), 3);
        assert_eq!(legs[1].video_start_s, footage.start_s(1));
        assert_eq!(legs[1].video_duration_s, 10.004);
        assert_eq!((legs[1].osd_start_s, legs[1].osd_duration_s), (9.5, 9.5));
        // Three recordings for two videos do not fit.
        assert!(footage.legs(&[recording("a.osd", 0.0, 9.5)]).is_none());
    }

    #[test]
    fn one_video_may_have_several_recordings() {
        let single = Footage::single(PathBuf::from("a.mp4"), info(30.0, 60.0));
        // The first recording starts a little after zero: the leg still starts at the start of the timeline.
        let legs = single
            .legs(&[
                recording("a.osd", 0.3, 14.0),
                recording("b.osd", 14.0, 29.0),
            ])
            .unwrap();
        assert_eq!(legs.len(), 1);
        assert_eq!((legs[0].osd_start_s, legs[0].osd_duration_s), (0.0, 29.0));
        assert!(single.legs(&[]).is_none(), "no recording is no leg");
    }

    #[test]
    fn the_timeline_follows_the_legs() {
        let footage = three();
        let osd = [
            recording("a.osd", 0.0, 9.5),
            recording("b.osd", 9.5, 19.0),
            recording("c.osd", 19.0, 24.0),
        ];
        let timeline = footage.timeline(SyncOptions::default(), &osd).unwrap();
        assert_eq!(timeline.scales().count(), 3);
        // The end of the first video is the end of its recording.
        assert!((timeline.telemetry_time(footage.start_s(1)) - 9.5).abs() < 1e-9);
        assert!(
            footage
                .timeline(SyncOptions::default(), &osd[..1])
                .is_none()
        );
    }
}
