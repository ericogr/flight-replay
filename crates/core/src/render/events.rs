// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A step of a render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Reading the telemetry and checking the plan.
    Preparing,
    /// Making the sound.
    Audio,
    /// Drawing and encoding the picture.
    Video,
    /// Closing the output.
    Finishing,
    /// Checking the output.
    Checking,
}

impl Stage {
    /// What the step is called to the user.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Preparing => "Preparing",
            Self::Audio => "Making the sound",
            Self::Video => "Drawing and encoding the video",
            Self::Finishing => "Finishing",
            Self::Checking => "Checking the result",
        }
    }
}

/// What a render reports while it runs.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A step starts.
    Stage(Stage),
    /// Progress inside a step: `done` of `total`, in frames.
    Progress {
        /// The step.
        stage: Stage,
        /// Frames done.
        done: u64,
        /// Frames in all.
        total: u64,
    },
    /// Something the user should know.
    Warning(String),
}

/// Asks a render to stop. Cloning gives another handle to the same request.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token that has not been cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the render to stop. It stops at the next frame or block, and removes what it wrote.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether a stop was asked for.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
