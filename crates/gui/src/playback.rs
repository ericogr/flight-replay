// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The picture of a preview that plays: the video as a stream, paced by a clock.
//!
//! One FFmpeg process decodes the stretch that is played, already shrunk to the size of the preview. A thread reads
//! its frames, draws the OSD and the SRT data on each, and hands it to the window at the moment the clock has
//! reached it. The clock is the sound when there is one, and the clock of the computer when there is not. Asking
//! for a picture for each moment instead starts FFmpeg and decodes from the nearest key frame every time, which is
//! far too slow to follow a clock.
//!
//! This is a preview and not the video. The picture is smaller than the video, it has at most [`MAX_FPS`] frames a
//! second, and a frame that comes too late for the clock is skipped. The render is not affected.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use flight_replay_core::ffmpeg::{DecodeOptions, FrameReader};
use iced::futures::channel::mpsc;
use image::RgbaImage;

use crate::preview::{Layers, Scene, shown_size};

/// The most frames a second that are shown. A video with more is shown at a whole fraction of its rate: 60 as 30.
pub const MAX_FPS: f64 = 30.0;

/// Frames that may wait for the window. When the window is behind, a frame that finds the line full is dropped: a
/// late picture is worse than a missing one.
const WAITING_FRAMES: usize = 2;

/// The longest the thread sleeps without looking at whether it was told to stop.
const SLEEP_SLICE: Duration = Duration::from_millis(20);

/// The longest the picture goes without a new frame because late frames are being skipped.
const MAX_STALL: Duration = Duration::from_millis(200);

/// What the thread answers: a frame, or why there are no more.
pub type Item = Result<PlayFrame, String>;

/// One picture of the stream.
#[derive(Debug)]
pub struct PlayFrame {
    /// The video time of the picture, in seconds.
    pub time_s: f64,
    /// The frame with the OSD and the SRT data drawn on it.
    pub picture: RgbaImage,
}

/// What to play.
pub struct PlayRequest {
    pub scene: Scene,
    /// The video time to start from, in seconds.
    pub start_s: f64,
    /// How much to play, in seconds.
    pub duration_s: f64,
    /// The moment that the clock was at `start_s`. A frame is shown when the clock has got to its time.
    pub started: Instant,
}

/// A stream that is going. The stream stops when this is dropped.
pub struct Playback {
    id: u64,
    stop: Arc<AtomicBool>,
}

impl Playback {
    /// Tells the frames of this stream from those of an older one.
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// A number that no other call has given: it tells one play, or one stream, from another.
pub fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The frames per second that a video is shown at: its own rate, or the whole fraction of it that is at most
/// [`MAX_FPS`].
pub fn play_rate(nominal_frame_rate: f64) -> f64 {
    let rate = if nominal_frame_rate.is_finite() && nominal_frame_rate > 0.0 {
        nominal_frame_rate
    } else {
        MAX_FPS
    };
    rate / (rate / MAX_FPS).ceil().max(1.0)
}

/// Whether a frame is skipped. It is when it is more than one frame late and a picture was shown not long ago:
/// skipping lets the picture catch up with the clock. When the decoding is slower than the clock every frame is late,
/// and skipping them all would freeze the picture, so one is shown at least every [`MAX_STALL`], late as it is.
fn should_skip(late_by: Duration, since_last_shown: Duration, one_frame: Duration) -> bool {
    late_by > one_frame && since_last_shown < MAX_STALL
}

/// Starts playing. The frames come from the receiver, in time, until the stretch ends or the [`Playback`] is
/// dropped.
pub fn start(request: PlayRequest) -> (Playback, mpsc::Receiver<Item>) {
    let stop = Arc::new(AtomicBool::new(false));
    let (mut sender, receiver) = mpsc::channel(WAITING_FRAMES);
    let flag = Arc::clone(&stop);
    std::thread::spawn(move || {
        if let Err(error) = run(&request, &mut sender, &flag) {
            send_error(&mut sender, error, &flag);
        }
    });
    let playback = Playback {
        id: next_id(),
        stop,
    };
    (playback, receiver)
}

/// Reads the frames, draws on them and sends each at its time. Returns when the stretch is over, when it was told to
/// stop, or when the window is gone.
fn run(
    request: &PlayRequest,
    sender: &mut mpsc::Sender<Item>,
    stop: &AtomicBool,
) -> Result<(), String> {
    let scene = &request.scene;
    let info = &scene.info;
    let (width, height) = shown_size(info.width, info.height);
    let rate = play_rate(info.nominal_frame_rate);
    let mut reader = FrameReader::open(
        &scene.tools,
        &scene.video,
        &DecodeOptions {
            width: info.width,
            height: info.height,
            start_s: request.start_s,
            duration_s: Some(request.duration_s),
            constant_rate: Some(rate),
            scale_to: ((width, height) != (info.width, info.height)).then_some((width, height)),
        },
    )
    .map_err(|e| e.to_string())?;
    // FFmpeg is already seeking, so the glyphs are made while it does.
    let layers = Layers::new(scene, f64::from(width) / f64::from(info.width.max(1)))?;

    let frame_s = 1.0 / rate;
    let one_frame = Duration::from_secs_f64(frame_s);
    let mut buffer = Vec::new();
    let mut index = 0u64;
    let mut last_shown = Instant::now();
    let mut ended = false;
    while !stop.load(Ordering::Relaxed) {
        match reader.read_frame(&mut buffer) {
            Ok(true) => {}
            Ok(false) => {
                ended = true;
                break;
            }
            Err(_) if stop.load(Ordering::Relaxed) => break,
            Err(error) => return Err(error.to_string()),
        }
        let offset_s = index as f64 * frame_s;
        index += 1;
        let due = request.started + Duration::from_secs_f64(offset_s);
        let late_by = Instant::now().saturating_duration_since(due);
        if should_skip(late_by, last_shown.elapsed(), one_frame) {
            continue;
        }
        let mut picture = RgbaImage::from_raw(width, height, std::mem::take(&mut buffer))
            .ok_or_else(|| "the frame has an unexpected size".to_owned())?;
        let time_s = request.start_s + offset_s;
        layers.draw(&mut picture, time_s);
        wait_until(due, stop);
        if let Err(error) = sender.try_send(Ok(PlayFrame { time_s, picture }))
            && error.is_disconnected()
        {
            break;
        }
        last_shown = Instant::now();
    }
    if ended {
        // The end of the stretch is not an error, but a failure of FFmpeg before it is.
        reader.finish().map_err(|e| e.to_string())
    } else {
        reader.cancel();
        Ok(())
    }
}

/// Sleeps until `moment`, or until told to stop.
fn wait_until(moment: Instant, stop: &AtomicBool) {
    while let Some(left) = moment.checked_duration_since(Instant::now()) {
        if left.is_zero() || stop.load(Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(left.min(SLEEP_SLICE));
    }
}

/// Sends the reason the stream failed. Unlike a frame it is not dropped when the line is full: it waits for room,
/// for a little while.
fn send_error(sender: &mut mpsc::Sender<Item>, error: String, stop: &AtomicBool) {
    let mut item = Err(error);
    for _ in 0..100 {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        match sender.try_send(item) {
            Err(refused) if refused.is_full() => item = refused.into_inner(),
            // It was sent, or the window is gone.
            Ok(()) | Err(_) => return,
        }
        std::thread::sleep(SLEEP_SLICE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_video_is_shown_at_its_rate_or_at_a_whole_fraction_of_it_under_the_limit() {
        for (video, shown) in [
            (24.0, 24.0),
            (25.0, 25.0),
            (30.0, 30.0),
            (50.0, 25.0),
            (60.0, 30.0),
            (120.0, 30.0),
        ] {
            assert!((play_rate(video) - shown).abs() < 1e-9, "{video} fps");
        }
        // 59.94 fps is every second frame, which keeps the pace of the video.
        assert!((play_rate(59.94) - 29.97).abs() < 1e-9);
        // A rate that cannot be read is not allowed to stop the play.
        for bad in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            assert!((play_rate(bad) - MAX_FPS).abs() < 1e-9, "{bad}");
        }
    }

    #[test]
    fn a_late_frame_is_skipped_unless_the_picture_has_stood_still_too_long() {
        let frame = Duration::from_millis(33);
        let ms = Duration::from_millis;
        // On time, or less than a frame late: shown.
        assert!(!should_skip(ms(0), ms(10), frame));
        assert!(!should_skip(ms(33), ms(10), frame));
        // More than a frame late, with a picture shown a moment ago: skipped, so the picture catches up.
        assert!(should_skip(ms(34), ms(10), frame));
        assert!(should_skip(ms(900), ms(199), frame));
        // Late as it is, once the picture has stood still for the limit: shown, so it never freezes.
        assert!(!should_skip(ms(900), ms(200), frame));
    }
}
