// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::io::Read;
use std::process::{ChildStderr, ChildStdout};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};

/// Keeps the last part of what a program writes to its error stream. The text is returned when the thread
/// is joined, after the program has exited.
pub(crate) fn collect_stderr(mut stderr: ChildStderr) -> JoinHandle<String> {
    const KEEP: usize = 16 * 1024;
    thread::spawn(move || {
        let mut kept: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(read) = stderr.read(&mut chunk) {
            if read == 0 {
                break;
            }
            kept.extend_from_slice(&chunk[..read]);
            if kept.len() > KEEP {
                kept.drain(..kept.len() - KEEP);
            }
        }
        String::from_utf8_lossy(&kept).trim().to_owned()
    })
}

/// Reads the `-progress` lines of a program and sends one snapshot per block to `sender`.
pub(crate) fn read_progress<T, F>(
    stdout: ChildStdout,
    sender: Sender<T>,
    mut parse: F,
) -> JoinHandle<()>
where
    T: Send + 'static,
    F: FnMut(&str) -> Option<T> + Send + 'static,
{
    thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Some(update) = parse(&line) else {
                continue;
            };
            if sender.send(update).is_err() {
                break;
            }
        }
    })
}
