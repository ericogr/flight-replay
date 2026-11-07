// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The choices of the steps: what to draw and where, the timing, the sound and the output.

use std::path::PathBuf;

use flight_replay_core::config::Settings;
use flight_replay_core::synth::{MOTOR_PRESETS, SoundSettings};
use iced::Task;

use super::{FlightReplay, ToolsState};
use crate::message::{Message, OsdEdit, OutputEdit, SoundEdit, SrtEdit, SyncEdit};
use crate::preferences::Preferences;
use crate::sound_params;
use crate::state::FileKind;
use crate::tasks;

impl FlightReplay {
    /// The user asked to reset everything: nothing changes until they confirm.
    pub(super) fn ask_reset_all(&mut self) -> Task<Message> {
        self.confirming_reset = !self.is_rendering();
        Task::none()
    }

    /// Brings every setting and preference back to the default, as in the first run. The file is written with the
    /// defaults by the usual save after a message. The video and the files that go with it stay open: they are not
    /// settings. It does nothing unless [`Self::ask_reset_all`] was answered with a yes.
    pub(super) fn reset_all(&mut self) -> Task<Message> {
        if !std::mem::take(&mut self.confirming_reset) || self.is_rendering() {
            return Task::none();
        }
        let ffmpeg_was_chosen = self.settings.ffmpeg_dir.is_some();
        self.settings = Settings::default();
        self.preferences = Preferences::default();
        self.preset_name.clear();
        self.stretch_on = false;
        self.recompute_signals();
        // The default encoder may not be one that works here.
        let encoders = self
            .encoders
            .clone()
            .map_or_else(Task::none, |list| self.encoders_detected(list));
        self.notices
            .push("Every setting was brought back to the default.".to_owned());
        // The font is a setting too. Taking it out also draws the preview again.
        let preview = self.clear_file(FileKind::Font);
        if ffmpeg_was_chosen {
            // Without the folder the user chose, FFmpeg is looked for where a first run would look.
            self.tools = ToolsState::Searching;
            return Task::batch([encoders, preview, tasks::locate_tools(None)]);
        }
        Task::batch([encoders, preview])
    }
    pub(super) fn set_draw_osd(&mut self, on: bool) -> Task<Message> {
        self.settings.draw_osd = on;
        self.refresh_preview_if_open()
    }

    pub(super) fn set_draw_srt(&mut self, on: bool) -> Task<Message> {
        self.settings.draw_srt = on;
        self.refresh_preview_if_open()
    }

    pub(super) fn edit_osd(&mut self, edit: OsdEdit) -> Task<Message> {
        match edit {
            OsdEdit::X(x) => self.settings.osd.x_offset = x,
            OsdEdit::Y(y) => self.settings.osd.y_offset = y,
            OsdEdit::Reset => {
                self.settings.osd.x_offset = 0;
                self.settings.osd.y_offset = 0;
            }
            OsdEdit::Center => self.center_osd(),
            OsdEdit::MaskMode(on) => self.preview.mask_mode = on,
            OsdEdit::ClearMasks => self.settings.osd.masked_cells.clear(),
        }
        self.refresh_preview_if_open()
    }

    /// Centres what the OSD shows at the moment of the preview on the picture.
    fn center_osd(&mut self) {
        let (Some(video), Some(osd), Some(font), Some(timeline)) =
            (&self.video, &self.osd, &self.scaled_font, self.timeline())
        else {
            return;
        };
        let frame = timeline.osd_frame(&osd.file, self.preview.time_s);
        self.settings
            .osd
            .center_on(frame, video.info.width, video.info.height, &font.size());
    }

    pub(super) fn edit_srt(&mut self, edit: SrtEdit) -> Task<Message> {
        let srt = &mut self.settings.srt;
        match edit {
            SrtEdit::Time(v) => srt.show_time = v,
            SrtEdit::SkyBattery(v) => srt.show_sky_battery = v,
            SrtEdit::GroundBattery(v) => srt.show_ground_battery = v,
            SrtEdit::Signal(v) => srt.show_signal = v,
            SrtEdit::Latency(v) => srt.show_latency = v,
            SrtEdit::Bitrate(v) => srt.show_bitrate = v,
            SrtEdit::Distance(v) => srt.show_distance = v,
            SrtEdit::PositionX(v) => srt.position_percent.0 = v,
            SrtEdit::PositionY(v) => srt.position_percent.1 = v,
            SrtEdit::Scale(v) => srt.scale = v,
        }
        self.refresh_preview_if_open()
    }

    pub(super) fn edit_sync(&mut self, edit: SyncEdit) -> Task<Message> {
        match edit {
            SyncEdit::Scale(choice) => self.preferences.sync.choice = choice,
            SyncEdit::ManualScale(text) => self.preferences.sync.manual_scale = text,
            SyncEdit::Offset(text) => self.preferences.sync.offset = text,
        }
        self.refresh_preview_if_open()
    }

    pub(super) fn edit_sound(&mut self, edit: SoundEdit) -> Task<Message> {
        match edit {
            SoundEdit::Param(param, value) => {
                sound_params::set(&mut self.settings.sound, param, value);
            }
            SoundEdit::SampleRate(rate) => self.settings.sound.audio.sample_rate = rate,
            SoundEdit::Stereo(stereo) => {
                self.settings.sound.audio.channels = if stereo { 2 } else { 1 };
            }
            SoundEdit::Aircraft(kind) => {
                // Choosing the kind that is already chosen would throw away what was changed by hand.
                if self.settings.sound.aircraft != kind {
                    kind.apply(&mut self.settings.sound);
                    self.preferences.selected_preset = None;
                }
            }
            SoundEdit::MotorPreset(index) => {
                if let Some(preset) = MOTOR_PRESETS.get(index) {
                    preset.apply(&mut self.settings.sound);
                    // The saved preset that was chosen no longer describes the sound.
                    self.preferences.selected_preset = None;
                }
            }
            SoundEdit::Preset(name) => {
                if let Some(preset) = self.settings.presets.get(&name) {
                    self.settings.sound = preset.clone();
                }
                self.preferences.selected_preset = Some(name.clone());
                self.preset_name = name;
            }
            SoundEdit::PresetName(name) => self.preset_name = name,
            SoundEdit::SavePreset => {
                let name = self.preset_name.trim().to_owned();
                if !name.is_empty() {
                    self.settings
                        .presets
                        .insert(name.clone(), self.settings.sound.clone());
                    self.preferences.selected_preset = Some(name);
                }
            }
            SoundEdit::DeletePreset => {
                if let Some(name) = self.preferences.selected_preset.take() {
                    self.settings.presets.remove(&name);
                }
            }
            SoundEdit::ResetToDefaults => {
                self.settings.sound = SoundSettings::default();
                self.preferences.selected_preset = None;
            }
        }
        self.recompute_signals();
        Task::none()
    }

    pub(super) fn edit_output(&mut self, edit: OutputEdit) -> Task<Message> {
        let output = &mut self.settings.output;
        match edit {
            OutputEdit::Encoder(name) => output.encoder = name,
            OutputEdit::Bitrate(v) => output.bitrate_mbps = v,
            OutputEdit::Upscale(v) => output.upscale_1440p = v,
            OutputEdit::ChromaKey(v) => output.chroma_key = v,
            OutputEdit::AudioBitrate(v) => output.audio_bitrate_kbps = v,
            OutputEdit::Faststart(v) => output.faststart = v,
            OutputEdit::ShowUndetected(v) => output.show_undetected_encoders = v,
            OutputEdit::Path(text) => self.output_path = text,
            OutputEdit::Overwrite(v) => self.preferences.overwrite = v,
            OutputEdit::ReplaceAudio(v) => self.preferences.replace_audio = v,
            OutputEdit::Stretch(v) => self.stretch_on = v,
            OutputEdit::StretchStart(text) => self.preferences.stretch.start = text,
            OutputEdit::StretchDuration(text) => self.preferences.stretch.duration = text,
        }
        Task::none()
    }

    /// Asks the user where to save the output.
    pub(super) fn browse_output(&mut self) -> Task<Message> {
        tasks::pick_output(PathBuf::from(self.output_path.trim()))
    }
}
