// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The queue: several videos rendered one after the other, with the settings of the wizard.

use std::path::PathBuf;
use std::sync::Arc;

use flight_replay_core::paths::file_name;
use flight_replay_core::render::{FlightDefaults, Outcome, RenderError};
use flight_replay_core::timeline::{ScaleMode, SyncOptions};
use iced::Task;

use super::FlightReplay;
use crate::message::Message;
use crate::state::{FolderQueue, QueueItem, QueueStatus};
use crate::tasks;

impl FlightReplay {
    /// Puts the video that is open in the queue, with what is chosen now.
    pub(super) fn queue_add(&mut self) -> Task<Message> {
        match self.build_plan() {
            Ok(plan) => {
                if self.queue_writes(&plan.output.path) {
                    self.notices
                        .push("A video with that output file is in the queue already.".to_owned());
                } else {
                    self.queue.items.push(QueueItem::new(plan));
                }
            }
            Err(problems) => self.notices.push(problems.join(" ")),
        }
        Task::none()
    }

    /// Whether a video of the queue writes this file.
    fn queue_writes(&self, output: &PathBuf) -> bool {
        self.queue
            .items
            .iter()
            .any(|item| &item.plan.output.path == output)
    }

    /// Plans the videos of a folder, in the background.
    pub(super) fn queue_plan_folder(&mut self, folder: PathBuf) -> Task<Message> {
        let Some(tools) = self.tools() else {
            return Task::none();
        };
        self.queue.preparing = true;
        let defaults = FlightDefaults {
            settings: self.settings.clone(),
            encoders: self
                .encoders
                .as_ref()
                .map_or_else(Vec::new, |list| list.to_vec()),
            font: self.font.as_ref().map(|font| font.font.clone()),
            // The speed of the OSD is worked out for each video. A manual scale belongs to the one that is open.
            sync: SyncOptions {
                scale: ScaleMode::Auto,
                ..self.preferences.sync.options()
            },
            overwrite: self.preferences.overwrite,
            replace_audio: self.preferences.replace_audio,
        };
        let planned = self
            .queue
            .items
            .iter()
            .map(|item| item.plan.output.path.clone())
            .collect();
        tasks::plan_folder(tools, folder, defaults, planned)
    }

    /// Puts the videos of a folder in the queue, and says which ones were left out and why.
    pub(super) fn queue_folder_planned(&mut self, result: Result<Arc<FolderQueue>, String>) {
        self.queue.preparing = false;
        match result {
            Ok(planned) => {
                self.queue.items.extend(planned.items.iter().cloned());
                let added = planned.items.len();
                self.notices.push(format!(
                    "{added} {} added to the queue.",
                    if added == 1 {
                        "video was"
                    } else {
                        "videos were"
                    }
                ));
                for reason in &planned.left_out {
                    self.notices
                        .push(format!("Added without part of it: {reason}"));
                }
                for reason in &planned.skipped {
                    self.notices.push(format!("Left out: {reason}"));
                }
            }
            Err(error) => self.notices.push(error),
        }
    }

    pub(super) fn queue_remove(&mut self, index: usize) {
        if index >= self.queue.items.len() || self.queue.current == Some(index) {
            return;
        }
        self.queue.items.remove(index);
        if let Some(current) = &mut self.queue.current
            && *current > index
        {
            *current -= 1;
        }
    }

    /// Takes out the videos that are done, failed or stopped.
    pub(super) fn queue_clear_finished(&mut self) {
        self.queue
            .items
            .retain(|item| matches!(item.status, QueueStatus::Waiting | QueueStatus::Running));
        self.queue.current = self
            .queue
            .items
            .iter()
            .position(|item| item.status == QueueStatus::Running);
    }

    pub(super) fn queue_start(&mut self) -> Task<Message> {
        if self.queue.running || self.is_rendering() {
            return Task::none();
        }
        self.queue.running = true;
        self.queue_next()
    }

    /// Starts the next video that is waiting, or ends the queue.
    pub(super) fn queue_next(&mut self) -> Task<Message> {
        let Some(index) = self
            .queue
            .items
            .iter()
            .position(|item| item.status == QueueStatus::Waiting)
        else {
            self.queue.running = false;
            return Task::none();
        };
        let Some(tools) = self.tools() else {
            self.queue.running = false;
            return Task::none();
        };
        let item = &mut self.queue.items[index];
        item.status = QueueStatus::Running;
        let plan = (*item.plan).clone();
        self.queue.current = Some(index);
        self.begin_render(tools, plan)
    }

    /// Records how the render of a video of the queue ended. A video that was stopped stops the queue.
    pub(super) fn queue_item_done(&mut self, index: usize, result: &Result<Arc<Outcome>, String>) {
        let Some(item) = self.queue.items.get_mut(index) else {
            return;
        };
        item.status = match result {
            Ok(outcome) => QueueStatus::Done(format!(
                "saved {} in {:.1} s",
                file_name(&outcome.output),
                outcome.elapsed.as_secs_f64()
            )),
            Err(error) if *error == RenderError::Cancelled.to_string() => QueueStatus::Cancelled,
            Err(error) => QueueStatus::Failed(error.clone()),
        };
        if item.status == QueueStatus::Cancelled {
            self.queue.running = false;
        }
    }
}
