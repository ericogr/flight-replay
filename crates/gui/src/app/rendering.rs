// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The plan of the render, and running it.

use std::path::PathBuf;

use flight_replay_core::ffmpeg::Tools;
use flight_replay_core::paths::folder_or_current;
use flight_replay_core::render::{
    CancelToken, Event, PlanInputs, RenderPlan, Stage, build_plan, command_preview,
};
use iced::Task;
use std::sync::Arc;

use super::FlightReplay;
use crate::message::{Message, RenderMessage};
use crate::state::{RenderState, Running};
use crate::tasks;

impl FlightReplay {
    /// Builds the plan from what the user chose.
    pub fn build_plan(&self) -> Result<RenderPlan, Vec<String>> {
        let Some(video) = &self.video else {
            return Err(vec!["Open a video.".into()]);
        };
        // Only what the files allow: the rest is left out of the render, and the steps say why.
        let settings = self.availability().restrict(&self.settings);
        build_plan(&PlanInputs {
            video: video.path.clone(),
            info: video.info.clone(),
            osd: self.osd.as_ref().map(|o| &*o.file),
            osd_path: self.osd.as_ref().map(|o| o.path().to_path_buf()),
            srt: self.srt.as_ref().map(|s| &*s.file),
            font: self.font.as_ref().map(|f| &f.font),
            settings: &settings,
            encoders: self.encoders.as_ref().map_or(&[], |list| list.as_slice()),
            sync: self.sync.options(),
            window: self.stretch.window(),
            output: PathBuf::from(self.output_path.trim()),
            overwrite: self.overwrite,
            replace_audio: self.replace_audio,
        })
    }

    /// The FFmpeg commands of the plan, for the user to see before the render starts.
    pub(super) fn refresh_command_lines(&mut self) {
        self.command_lines = match (self.build_plan(), self.tools()) {
            (Ok(plan), Some(tools)) => command_preview(&plan, tools.ffmpeg()),
            _ => Vec::new(),
        };
    }

    /// Starts a render in the background, and shows its progress.
    pub(super) fn begin_render(&mut self, tools: Arc<Tools>, plan: RenderPlan) -> Task<Message> {
        let cancel = CancelToken::new();
        self.render = RenderState::Running(Running::new(cancel.clone()));
        tasks::start_render(tools, plan, cancel)
    }

    pub(super) fn start_render(&mut self) -> Task<Message> {
        let Some(tools) = self.tools() else {
            return Task::none();
        };
        match self.build_plan() {
            Ok(plan) => {
                self.save_settings();
                self.begin_render(tools, plan)
            }
            Err(problems) => {
                self.render = RenderState::Finished(Err(problems.join("\n")));
                Task::none()
            }
        }
    }

    /// Stops the render that is running. The videos of the queue that have not started stay in it, waiting.
    pub(super) fn cancel_render(&mut self) {
        if let RenderState::Running(running) = &mut self.render {
            running.cancel.cancel();
            running.cancelling = true;
        }
        self.queue.running = false;
    }

    pub(super) fn render_message(&mut self, message: RenderMessage) -> Task<Message> {
        match message {
            RenderMessage::Event(event) => {
                if let RenderState::Running(running) = &mut self.render {
                    match event {
                        Event::Stage(stage) => running.stage = stage,
                        Event::Progress { stage, done, total } => match stage {
                            Stage::Audio => running.audio = (done, total),
                            Stage::Video => running.video = (done, total),
                            _ => {}
                        },
                        Event::Warning(text) => running.warnings.push(text),
                    }
                }
                Task::none()
            }
            RenderMessage::Done(result) => {
                let finished = self.queue.current.take();
                if let Some(index) = finished {
                    self.queue_item_done(index, &result);
                }
                self.render = RenderState::Finished(
                    result
                        .map(|outcome| Box::new((*outcome).clone()))
                        .map_err(|error| error.clone()),
                );
                if finished.is_some() && self.queue.running {
                    self.queue_next()
                } else {
                    Task::none()
                }
            }
        }
    }

    /// The folder the finished render wrote its file in.
    pub fn output_folder(&self) -> Option<PathBuf> {
        match &self.render {
            RenderState::Finished(Ok(outcome)) => {
                Some(folder_or_current(&outcome.output).to_path_buf())
            }
            _ => None,
        }
    }

    pub(super) fn open_output_folder(&self) {
        if let Some(folder) = self.output_folder() {
            tasks::open_folder(&folder);
        }
    }
}
