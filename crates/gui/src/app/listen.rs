// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The sound preview: ten seconds of the sound, played while the picture follows it.

use std::sync::Arc;

use iced::Task;

use super::FlightReplay;
use crate::audio::AudioPlayer;
use crate::message::Message;
use crate::tasks;

/// How often, in milliseconds, the picture catches up with the sound.
const AUDIO_TICK_MS: u64 = 250;

/// How much sound the preview plays, in seconds, at most.
const PREVIEW_SECONDS: f64 = 10.0;

/// Less sound than this is not worth playing, in seconds.
const MIN_PREVIEW_SECONDS: f64 = 0.2;

impl FlightReplay {
    /// Whether the sound preview is playing.
    pub fn is_playing(&self) -> bool {
        self.audio.is_some()
    }

    /// Makes the sound from the time of the preview. It plays when it is ready.
    pub(super) fn play_audio(&mut self) -> Task<Message> {
        self.audio = None;
        self.audio_error = None;
        let (Some(video), Some(signals), Some(timeline)) =
            (&self.video, self.signals(), self.timeline())
        else {
            return Task::none();
        };
        // Only as much sound as there is video: the picture follows the sound, and it cannot go past its end.
        let seconds = PREVIEW_SECONDS.min(video.info.duration_s - self.preview.time_s);
        if seconds < MIN_PREVIEW_SECONDS {
            self.audio_error = Some("There is no video left to play from here.".to_owned());
            return Task::none();
        }
        self.busy.audio = true;
        tasks::render_audio_preview(
            self.settings.sound.clone(),
            signals,
            timeline,
            self.preview.time_s,
            seconds,
        )
    }

    /// The sound is made: plays it, and starts the ticks that move the picture along.
    pub(super) fn audio_rendered(
        &mut self,
        result: Result<Arc<Vec<i16>>, String>,
    ) -> Task<Message> {
        self.busy.audio = false;
        let pcm = match result {
            Ok(pcm) => pcm,
            Err(error) => {
                self.audio_error = Some(error);
                return Task::none();
            }
        };
        let audio = &self.settings.sound.audio;
        let start_s = self.preview.time_s;
        match AudioPlayer::play(&pcm, audio.sample_rate, audio.channels, start_s) {
            Ok(player) => {
                self.audio = Some(player);
                tasks::audio_tick_after(AUDIO_TICK_MS)
            }
            Err(error) => {
                self.audio_error = Some(error);
                Task::none()
            }
        }
    }

    /// Stops the sound, and puts the picture back at the moment that was chosen to listen from.
    pub(super) fn stop_audio(&mut self) -> Task<Message> {
        let Some(player) = self.audio.take() else {
            return Task::none();
        };
        self.preview.time_s = player.start_s();
        self.refresh_preview_if_open()
    }

    /// Moves the picture to where the sound is, or ends the preview when the sound is over.
    pub(super) fn audio_tick(&mut self) -> Task<Message> {
        let Some(player) = &self.audio else {
            return Task::none();
        };
        if player.is_finished() {
            return self.stop_audio();
        }
        // A picture is asked for only when the last one has arrived, so a slow video falls behind instead of
        // piling up requests.
        let last = self.video.as_ref().map_or(0.0, |v| v.last_frame_s());
        self.preview.time_s = player.position_s().min(last);
        let picture = if self.preview.busy {
            Task::none()
        } else {
            self.refresh_preview_if_open()
        };
        Task::batch([picture, tasks::audio_tick_after(AUDIO_TICK_MS)])
    }
}
