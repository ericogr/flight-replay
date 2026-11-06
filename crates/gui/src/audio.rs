// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Plays the sound preview on the default audio device.

use std::num::NonZero;

use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};

/// A sound that is playing. The sound stops when this is dropped.
pub struct AudioPlayer {
    // The device must live as long as the player.
    _device: MixerDeviceSink,
    player: Player,
}

impl AudioPlayer {
    /// Starts playing interleaved 16-bit samples on the default audio device.
    pub fn play(samples: &[i16], sample_rate: u32, channels: u16) -> Result<Self, String> {
        let sample_rate = NonZero::new(sample_rate).ok_or("the sample rate is zero")?;
        let channels = NonZero::new(channels).ok_or("there are no audio channels")?;
        let device = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("there is no audio device to play on ({error})"))?;
        let player = Player::connect_new(device.mixer());
        let floats: Vec<f32> = samples.iter().map(|&s| f32::from(s) / 32768.0).collect();
        player.append(SamplesBuffer::new(channels, sample_rate, floats));
        Ok(Self {
            _device: device,
            player,
        })
    }

    /// Whether the sound has played to the end.
    pub fn is_finished(&self) -> bool {
        self.player.empty()
    }
}
