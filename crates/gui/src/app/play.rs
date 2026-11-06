// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The preview in motion: ten seconds of the video, played from the time of the preview.
//!
//! The step of the sound plays the picture with the motor sound; the steps of the OSD and of the SRT data play the
//! picture alone, to check the layer in motion. Either way the picture is a stream (see `playback`) that is paced by
//! a clock, the clock of the sound or the clock of the computer. It is a preview at reduced quality, and the steps
//! say so.

use std::sync::Arc;
use std::time::Instant;

use iced::Task;

use super::FlightReplay;
use crate::audio::AudioPlayer;
use crate::message::Message;
use crate::playback::{self, PlayFrame, PlayRequest, Playback};
use crate::tasks;
use crate::wizard::Step;

/// How often, in milliseconds, the end of the preview is looked for.
const PLAY_TICK_MS: u64 = 250;

/// How long, in milliseconds, the picture waits after an edit before it starts again with what is chosen. Dragging a
/// slider makes many edits a second, and each start of the picture runs FFmpeg.
const RESTART_DELAY_MS: u64 = 150;

/// How much the preview plays, in seconds, at most.
const PREVIEW_SECONDS: f64 = 10.0;

/// Less than this is not worth playing, in seconds.
const MIN_PREVIEW_SECONDS: f64 = 0.2;

/// A preview that is playing.
pub(super) struct Playing {
    /// Tells this play from an earlier one: the ticks and the restarts that were asked for by that one are ignored.
    id: u64,
    /// The video time that it started from, in seconds.
    start_s: f64,
    /// How long it lasts, in seconds.
    duration_s: f64,
    /// The moment that the video was at `start_s`. The picture is paced by it.
    started: Instant,
    /// The motor sound, in the step of the sound. The steps of the OSD and of the SRT data have none.
    sound: Option<AudioPlayer>,
    /// The stream of the picture.
    picture: Option<Playback>,
    /// Counts the edits that ask for the picture to start again, so that only the last one does.
    edits: u64,
}

impl Playing {
    /// Whether it has played to the end.
    fn is_over(&self) -> bool {
        match &self.sound {
            Some(sound) => sound.is_finished(),
            None => self.started.elapsed().as_secs_f64() >= self.duration_s,
        }
    }

    /// Where the clock is: the video time now, and how much is left to play, in seconds.
    fn position(&self) -> (f64, f64) {
        let elapsed = self.started.elapsed().as_secs_f64();
        (self.start_s + elapsed, self.duration_s - elapsed)
    }
}

impl FlightReplay {
    /// Whether the preview is playing.
    pub fn is_playing(&self) -> bool {
        self.playing.is_some()
    }

    /// Stops what is playing, the sound and the picture. The picture shown stays as it is.
    pub(super) fn cancel_play(&mut self) {
        self.playing = None;
    }

    /// How much of the video can be played from the time of the preview, in seconds. The picture cannot go past the
    /// end of the video. Tells the user when there is nothing left.
    fn seconds_to_play(&mut self) -> Option<f64> {
        let video = self.video.as_ref()?;
        let seconds = PREVIEW_SECONDS.min(video.info.duration_s - self.preview.time_s);
        if seconds < MIN_PREVIEW_SECONDS {
            self.play_error = Some("There is no video left to play from here.".to_owned());
            return None;
        }
        Some(seconds)
    }

    /// Makes the sound from the time of the preview. It plays, with the picture, when it is ready.
    pub(super) fn play_audio(&mut self) -> Task<Message> {
        self.cancel_play();
        self.play_error = None;
        let (Some(signals), Some(timeline)) = (self.signals(), self.timeline()) else {
            return Task::none();
        };
        let Some(seconds) = self.seconds_to_play() else {
            return Task::none();
        };
        self.busy.audio = true;
        tasks::render_audio_preview(
            self.settings.sound.clone(),
            signals,
            timeline,
            self.preview.time_s,
            seconds,
        )
    }

    /// The sound is made: plays it with the picture.
    pub(super) fn audio_rendered(
        &mut self,
        result: Result<Arc<Vec<i16>>, String>,
    ) -> Task<Message> {
        self.busy.audio = false;
        // The sound belongs to its step. If the user left the step while it was being made, it is not played there.
        if self.step != Step::Sound {
            return Task::none();
        }
        let pcm = match result {
            Ok(pcm) => pcm,
            Err(error) => {
                self.play_error = Some(error);
                return Task::none();
            }
        };
        let audio = &self.settings.sound.audio;
        let (rate, channels) = (audio.sample_rate, audio.channels);
        match AudioPlayer::play(&pcm, rate, channels) {
            Ok(sound) => {
                let duration_s = pcm.len() as f64 / (f64::from(rate) * f64::from(channels));
                self.begin_play(self.preview.time_s, duration_s, Some(sound))
            }
            Err(error) => {
                self.play_error = Some(error);
                Task::none()
            }
        }
    }

    /// Plays the picture alone from the time of the preview, to check the OSD or the SRT data in motion.
    pub(super) fn play_picture(&mut self) -> Task<Message> {
        self.cancel_play();
        self.play_error = None;
        if self.tools().is_none() {
            return Task::none();
        }
        let Some(duration_s) = self.seconds_to_play() else {
            return Task::none();
        };
        self.begin_play(self.preview.time_s, duration_s, None)
    }

    /// Starts a play: `duration_s` seconds of the video from `start_s`, with the sound if there is one. The clock
    /// starts now.
    fn begin_play(
        &mut self,
        start_s: f64,
        duration_s: f64,
        sound: Option<AudioPlayer>,
    ) -> Task<Message> {
        let (id, started) = (playback::next_id(), Instant::now());
        self.playing = Some(Playing {
            id,
            start_s,
            duration_s,
            started,
            sound,
            picture: None,
            edits: 0,
        });
        Task::batch([
            self.start_picture(start_s, duration_s, started),
            tasks::after(PLAY_TICK_MS, Message::PlayTick(id)),
        ])
    }

    /// Stops the preview, and puts the picture back at the moment that it started from.
    pub(super) fn stop_play(&mut self) -> Task<Message> {
        let Some(playing) = self.playing.take() else {
            return Task::none();
        };
        self.preview.time_s = playing.start_s;
        self.refresh_preview_if_open()
    }

    /// Ends the play with this number when it is over, and goes on looking while it plays. The picture does not wait
    /// for this: it comes from the stream.
    pub(super) fn play_tick(&mut self, id: u64) -> Task<Message> {
        let Some(playing) = self.playing.as_ref().filter(|p| p.id == id) else {
            return Task::none();
        };
        if playing.is_over() {
            return self.stop_play();
        }
        tasks::after(PLAY_TICK_MS, Message::PlayTick(id))
    }

    /// Starts the stream of the picture of the play that is going: `duration_s` seconds of the video from `start_s`,
    /// each frame shown when the clock gets to it. `started` is the moment that the clock was at `start_s`. A stream
    /// that was going is stopped.
    fn start_picture(&mut self, start_s: f64, duration_s: f64, started: Instant) -> Task<Message> {
        let Some(scene) = self.scene() else {
            return Task::none();
        };
        let Some(playing) = &mut self.playing else {
            return Task::none();
        };
        let (picture, frames) = playback::start(PlayRequest {
            scene,
            start_s,
            duration_s,
            started,
        });
        let stream = picture.id();
        playing.picture = Some(picture);
        self.preview.error = None;
        Task::run(frames, move |item| {
            Message::PlayFrame(stream, item.map(Arc::new))
        })
    }

    /// Asks for the picture to start again from where the clock is, so that it shows what is chosen now. It does so
    /// when no other edit has come for a moment. What plays goes on meanwhile.
    pub(super) fn restart_playback(&mut self) -> Task<Message> {
        let Some(playing) = &mut self.playing else {
            return Task::none();
        };
        playing.edits += 1;
        tasks::after(
            RESTART_DELAY_MS,
            Message::RestartPlayback {
                play: playing.id,
                edit: playing.edits,
            },
        )
    }

    /// The moment to start the picture again has come, unless the play is another one or there was another edit since.
    pub(super) fn playback_restart_due(&mut self, play: u64, edit: u64) -> Task<Message> {
        let Some(playing) = self
            .playing
            .as_ref()
            .filter(|p| p.id == play && p.edits == edit)
        else {
            return Task::none();
        };
        let (start_s, left) = playing.position();
        if left < MIN_PREVIEW_SECONDS {
            return Task::none();
        }
        self.start_picture(start_s, left, Instant::now())
    }

    /// A picture of a stream. One from a stream that was stopped is ignored.
    pub(super) fn play_frame(
        &mut self,
        stream: u64,
        result: Result<Arc<PlayFrame>, String>,
    ) -> Task<Message> {
        let current = self
            .playing
            .as_ref()
            .and_then(|playing| playing.picture.as_ref())
            .map(Playback::id);
        if current != Some(stream) {
            return Task::none();
        }
        match result {
            Ok(frame) => {
                // Nothing else holds the frame, so its picture is not copied.
                let PlayFrame { time_s, picture } =
                    Arc::try_unwrap(frame).unwrap_or_else(|shared| PlayFrame {
                        time_s: shared.time_s,
                        picture: shared.picture.clone(),
                    });
                let (width, height) = picture.dimensions();
                self.preview.time_s = time_s;
                self.preview.set_picture(width, height, picture.into_raw());
                self.preview.error = None;
            }
            Err(error) => self.preview.error = Some(error),
        }
        Task::none()
    }
}
