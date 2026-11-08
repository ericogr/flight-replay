// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Asking long work to stop: a render, or the install of FFmpeg.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Asks work that takes a while to stop. Cloning gives another handle to the same request.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token that has not been cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the work to stop. It stops at the next frame, block or chunk of data, and removes what it wrote.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether a stop was asked for.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_clone_sees_the_request() {
        let token = CancelToken::new();
        let other = token.clone();
        assert!(!other.is_cancelled());
        token.cancel();
        assert!(other.is_cancelled());
    }
}
