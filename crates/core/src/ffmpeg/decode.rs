// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::io::{BufReader, Read};
use std::path::Path;
use std::process::{Child, ChildStdout, Stdio};
use std::thread::JoinHandle;

use super::process::collect_stderr;
use super::tools::command;
use super::{FfmpegError, Tools};

/// Reads the frames of a video as RGBA pictures, one after another.
///
/// FFmpeg decodes the video in a child process and writes raw frames to a pipe. Only one frame is held in
/// memory at a time, through the buffer given to [`FrameReader::read_frame`].
pub struct FrameReader {
    child: Option<Child>,
    stdout: BufReader<ChildStdout>,
    stderr: Option<JoinHandle<String>>,
    width: u32,
    height: u32,
    frames_read: u64,
}

/// How to decode a video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecodeOptions {
    /// Picture width, in pixels.
    pub width: u32,
    /// Picture height, in pixels.
    pub height: u32,
    /// Where to start, in seconds.
    pub start_s: f64,
    /// How long to decode, in seconds. `None` decodes to the end.
    pub duration_s: Option<f64>,
    /// Gives frames at a constant rate: a gap in the video repeats the previous frame, so that frame `n` is always at
    /// time `n / rate`. Without it, the frames come as they are stored.
    pub constant_rate: Option<f64>,
}

impl DecodeOptions {
    /// Decodes the whole video as stored.
    #[must_use]
    pub fn whole(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            start_s: 0.0,
            duration_s: None,
            constant_rate: None,
        }
    }
}

impl FrameReader {
    /// Starts decoding `input`.
    ///
    /// The width and height are the picture size reported by [`super::probe`].
    pub fn open(tools: &Tools, input: &Path, options: &DecodeOptions) -> Result<Self, FfmpegError> {
        let mut command = command(tools.ffmpeg());
        command.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
        if options.start_s > 0.0 {
            command.arg("-ss").arg(format!("{:.6}", options.start_s));
        }
        command.arg("-i").arg(input);
        if let Some(duration) = options.duration_s {
            command.arg("-t").arg(format!("{duration:.6}"));
        }
        if let Some(rate) = options.constant_rate {
            command.arg("-vf").arg(format!("fps={rate:.6}"));
        }
        command
            .args([
                "-map", "0:v:0", "-an", "-sn", "-dn", "-f", "rawvideo", "-pix_fmt", "rgba", "-",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|source| FfmpegError::Spawn {
            tool: "ffmpeg",
            source,
        })?;
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().map(collect_stderr);
        Ok(Self {
            child: Some(child),
            stdout: BufReader::with_capacity(1 << 20, stdout),
            stderr,
            width: options.width,
            height: options.height,
            frames_read: 0,
        })
    }

    /// Size of one frame in bytes (RGBA, 4 bytes per pixel).
    #[must_use]
    pub fn frame_bytes(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }

    /// Number of frames read so far.
    #[must_use]
    pub fn frames_read(&self) -> u64 {
        self.frames_read
    }

    /// Reads the next frame into `buffer`, which is resized to one frame.
    ///
    /// Returns `Ok(false)` at the end of the video.
    pub fn read_frame(&mut self, buffer: &mut Vec<u8>) -> Result<bool, FfmpegError> {
        let size = self.frame_bytes();
        buffer.resize(size, 0);
        let mut filled = 0;
        while filled < size {
            let read = self.stdout.read(&mut buffer[filled..])?;
            if read == 0 {
                break;
            }
            filled += read;
        }
        match filled {
            0 => Ok(false),
            n if n < size => Err(FfmpegError::TruncatedFrame),
            _ => {
                self.frames_read += 1;
                Ok(true)
            }
        }
    }

    /// Waits for FFmpeg to finish. Reports an error if it failed before the end of the video.
    pub fn finish(mut self) -> Result<(), FfmpegError> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        let status = child.wait()?;
        let stderr = self
            .stderr
            .take()
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();
        if status.success() {
            Ok(())
        } else {
            Err(FfmpegError::Failed {
                tool: "ffmpeg",
                stderr,
            })
        }
    }

    /// Stops the decoder early. Used when the rest of the video is not needed.
    pub fn cancel(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(handle) = self.stderr.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for FrameReader {
    fn drop(&mut self) {
        self.stop();
    }
}
