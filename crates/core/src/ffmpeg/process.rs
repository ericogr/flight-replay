// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::io::Read;
use std::process::{Child, ChildStderr, ChildStdout};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};

/// Stops a program, and on Windows the programs it started.
///
/// The `ffmpeg.exe` that Chocolatey and Scoop put in `PATH` is a small shim that runs the real FFmpeg as a child.
/// Killing the shim alone leaves the real one running: it holds the pipes open, so the threads that read them never
/// see the end, and it holds the output file, so a cancelled render cannot be deleted. `taskkill /T` stops the whole
/// tree. It runs first, because it finds the children through the shim, which has to be alive for that.
pub(crate) fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::path::Path;
        use std::process::Stdio;

        let _ = super::tools::command(Path::new("taskkill"))
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

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

#[cfg(all(test, windows))]
mod tests {
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    /// A program that starts another and waits for it, as the shims of Chocolatey and Scoop do. The pipe closes only
    /// when every program that holds it has ended, so it closing quickly shows that the child was stopped too.
    #[test]
    fn killing_a_program_also_stops_the_program_it_started() {
        let mut shim = Command::new("cmd")
            .args(["/C", "ping -n 60 127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("cmd starts");
        let mut pipe = shim.stdout.take().expect("stdout is piped");
        let (closed, wait_for_close) = mpsc::channel();
        thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = pipe.read_to_end(&mut sink);
            let _ = closed.send(());
        });
        // Time for cmd to start ping.
        thread::sleep(Duration::from_secs(1));

        kill_tree(&mut shim);
        let _ = shim.wait();

        assert!(
            wait_for_close.recv_timeout(Duration::from_secs(10)).is_ok(),
            "the program started by the one that was killed is still running"
        );
    }
}
