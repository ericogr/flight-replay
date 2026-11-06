// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// The stream-copy mux follows fpv-motorsound's internal/media/mux.go (Apache-2.0, same author), ported to Rust:
// the video is copied, the audio is written to FFmpeg's standard input, and the output is written to a
// `.partial` file that is renamed when FFmpeg succeeds.

use std::ffi::OsString;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;

use super::process::{collect_stderr, read_progress};
use super::progress::{Progress, parse_progress_line};
use super::tools::command;
use super::{FfmpegError, Tools};
use crate::paths::parent_folder;

/// Where the picture comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum VideoInput {
    /// Raw RGBA frames, written to the encoder's standard input.
    Raw {
        /// Picture width, in pixels.
        width: u32,
        /// Picture height, in pixels.
        height: u32,
        /// Frames per second.
        frame_rate: f64,
    },
    /// The video stream of a file, copied without re-encoding (`-c:v copy`).
    Copy {
        /// The file whose video stream is copied.
        path: PathBuf,
        /// Where the copy starts, in seconds (0 for the start). The copy starts at the keyframe at or before it.
        start_s: f64,
    },
}

/// Where the sound comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioInput {
    /// No sound track. Any sound in a copied file is dropped.
    None,
    /// Signed 16-bit little-endian PCM, written to the encoder's standard input.
    PcmStdin {
        /// Samples per second, per channel.
        sample_rate: u32,
        /// Number of channels (1 or 2).
        channels: u16,
    },
    /// A WAV file that FFmpeg reads.
    WavFile {
        /// Path of the WAV file.
        path: PathBuf,
    },
}

/// How the picture is encoded. Required with raw frames, and not used when the video is copied.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoEncoding {
    /// FFmpeg encoder name, such as `libx264`.
    pub encoder: String,
    /// Target bitrate, in megabits per second.
    pub bitrate_mbps: u32,
    /// Encoder options besides the bitrate.
    pub extra_args: Vec<String>,
    /// Scale the picture to 2560x1440 (height 1440, width kept in proportion).
    pub upscale_1440p: bool,
}

/// One render: picture, sound, encoding and output.
#[derive(Debug, Clone, PartialEq)]
pub struct EncodeJob {
    /// Source of the picture.
    pub video: VideoInput,
    /// Source of the sound.
    pub audio: AudioInput,
    /// Encoding of the picture. Must be `Some` for raw frames, and `None` for a copied stream.
    pub encoding: Option<VideoEncoding>,
    /// AAC bitrate, in kilobits per second.
    pub audio_bitrate_kbps: u32,
    /// Moves the MP4 index to the start of the file.
    pub faststart: bool,
    /// Stops the output after this many seconds.
    pub duration_s: Option<f64>,
    /// Final path of the file. The encode is written to `<output>.partial` first.
    pub output: PathBuf,
}

impl EncodeJob {
    /// The file the encode is written to until it succeeds.
    #[must_use]
    pub fn partial_path(&self) -> PathBuf {
        let mut name = self.output.as_os_str().to_owned();
        name.push(".partial");
        PathBuf::from(name)
    }

    /// Checks that the job can run.
    pub fn validate(&self) -> Result<(), FfmpegError> {
        match (&self.video, &self.encoding) {
            (VideoInput::Raw { .. }, None) => {
                return Err(FfmpegError::InvalidJob("raw frames need a video encoding"));
            }
            (VideoInput::Copy { .. }, Some(_)) => {
                return Err(FfmpegError::InvalidJob("a copied video cannot be encoded"));
            }
            _ => {}
        }
        let raw_video = matches!(self.video, VideoInput::Raw { .. });
        let pcm_on_stdin = matches!(self.audio, AudioInput::PcmStdin { .. });
        if raw_video && pcm_on_stdin {
            return Err(FfmpegError::InvalidJob(
                "raw frames and PCM cannot both use standard input",
            ));
        }
        if let AudioInput::PcmStdin { channels: 0, .. } = self.audio {
            return Err(FfmpegError::InvalidJob("PCM needs at least one channel"));
        }
        Ok(())
    }

    /// The command line of the job, for showing to the user. The output is the `.partial` file, which is renamed when
    /// FFmpeg succeeds.
    #[must_use]
    pub fn command_line(&self, ffmpeg: &Path) -> String {
        super::quote_command(ffmpeg, &self.args(&self.partial_path()))
    }

    /// Whether the job reads from the encoder's standard input.
    #[must_use]
    pub fn uses_stdin(&self) -> bool {
        matches!(self.video, VideoInput::Raw { .. })
            || matches!(self.audio, AudioInput::PcmStdin { .. })
    }

    fn args(&self, partial: &Path) -> Vec<OsString> {
        let mut args: Vec<OsString> = Vec::new();
        push_all(
            &mut args,
            [
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostats",
                "-y",
                "-progress",
                "pipe:1",
            ],
        );

        match &self.video {
            VideoInput::Raw {
                width,
                height,
                frame_rate,
            } => {
                push_all(&mut args, ["-f", "rawvideo", "-pix_fmt", "rgba"]);
                args.push("-s".into());
                args.push(format!("{width}x{height}").into());
                args.push("-r".into());
                args.push(format!("{frame_rate:.6}").into());
                push_all(&mut args, ["-i", "pipe:0"]);
            }
            VideoInput::Copy { path, start_s } => {
                if *start_s > 0.0 {
                    args.push("-ss".into());
                    args.push(format!("{start_s:.6}").into());
                }
                args.push("-i".into());
                args.push(path.as_os_str().to_owned());
            }
        }

        match &self.audio {
            AudioInput::None => {}
            AudioInput::PcmStdin {
                sample_rate,
                channels,
            } => {
                push_all(&mut args, ["-f", "s16le"]);
                args.push("-ar".into());
                args.push(sample_rate.to_string().into());
                args.push("-ac".into());
                args.push(channels.to_string().into());
                push_all(&mut args, ["-i", "pipe:0"]);
            }
            AudioInput::WavFile { path } => {
                args.push("-i".into());
                args.push(path.as_os_str().to_owned());
            }
        }

        // The video is always input 0 and the sound always input 1.
        push_all(&mut args, ["-map", "0:v:0"]);
        if !matches!(self.audio, AudioInput::None) {
            push_all(&mut args, ["-map", "1:a:0"]);
        }

        match &self.encoding {
            None => push_all(&mut args, ["-c:v", "copy"]),
            Some(encoding) => {
                if encoding.upscale_1440p {
                    push_all(&mut args, ["-vf", "scale=-2:1440:flags=bicubic"]);
                }
                args.push("-c:v".into());
                args.push(encoding.encoder.clone().into());
                push_all(&mut args, ["-pix_fmt", "yuv420p"]);
                args.push("-b:v".into());
                args.push(format!("{}M", encoding.bitrate_mbps).into());
                args.extend(encoding.extra_args.iter().map(|arg| arg.clone().into()));
            }
        }

        if !matches!(self.audio, AudioInput::None) {
            push_all(&mut args, ["-c:a", "aac"]);
            args.push("-b:a".into());
            args.push(format!("{}k", self.audio_bitrate_kbps).into());
        }
        if let Some(duration) = self.duration_s {
            args.push("-t".into());
            args.push(format!("{duration:.6}").into());
        }
        if self.faststart {
            push_all(&mut args, ["-movflags", "+faststart"]);
        }
        push_all(&mut args, ["-f", "mp4"]);
        args.push(partial.as_os_str().to_owned());
        args
    }
}

fn push_all<const N: usize>(args: &mut Vec<OsString>, values: [&str; N]) {
    args.extend(values.into_iter().map(OsString::from));
}

/// A running encode. Write the input to it, then call [`EncodeProcess::finish`] or [`EncodeProcess::abort`].
pub struct EncodeProcess {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stderr: Option<JoinHandle<String>>,
    progress_thread: Option<JoinHandle<()>>,
    progress: Receiver<Progress>,
    partial: PathBuf,
    output: PathBuf,
}

impl EncodeProcess {
    /// Starts FFmpeg for `job`.
    pub fn spawn(tools: &Tools, job: &EncodeJob) -> Result<Self, FfmpegError> {
        job.validate()?;
        let partial = job.partial_path();
        if let Some(parent) = parent_folder(&job.output) {
            fs::create_dir_all(parent)?;
        }

        let mut command = command(tools.ffmpeg());
        command
            .args(job.args(&partial))
            .stdin(if job.uses_stdin() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(FfmpegError::spawn("ffmpeg"))?;

        let stdin = child.stdin.take();
        let stderr = child.stderr.take().map(collect_stderr);
        let (sender, progress) = mpsc::channel();
        let mut current = Progress::default();
        let progress_thread = child.stdout.take().map(|stdout| {
            read_progress(stdout, sender, move |line| {
                parse_progress_line(line, &mut current)
            })
        });

        Ok(Self {
            child: Some(child),
            stdin,
            stderr,
            progress_thread,
            progress,
            partial,
            output: job.output.clone(),
        })
    }

    /// Writes input data (raw frames or PCM) to FFmpeg.
    ///
    /// If FFmpeg has stopped, the error contains its messages.
    pub fn write(&mut self, data: &[u8]) -> Result<(), FfmpegError> {
        let Some(stdin) = self.stdin.as_mut() else {
            return Err(FfmpegError::InvalidJob(
                "this job does not read standard input",
            ));
        };
        match stdin.write_all(data) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::BrokenPipe => Err(self.stop_after_failure()),
            Err(error) => Err(FfmpegError::Io(error)),
        }
    }

    /// Snapshots of the encode's progress. The channel closes when FFmpeg exits.
    #[must_use]
    pub fn progress(&self) -> &Receiver<Progress> {
        &self.progress
    }

    /// Closes the input, waits for FFmpeg and moves the output into place.
    pub fn finish(mut self) -> Result<PathBuf, FfmpegError> {
        drop(self.stdin.take());
        let Some(mut child) = self.child.take() else {
            return Err(FfmpegError::InvalidJob("the encode already ended"));
        };
        let status = child.wait()?;
        let stderr = self.join_stderr();
        self.join_progress();

        if status.success() {
            fs::rename(&self.partial, &self.output)?;
            Ok(self.output.clone())
        } else {
            let _ = fs::remove_file(&self.partial);
            Err(FfmpegError::Failed {
                tool: "ffmpeg",
                stderr,
            })
        }
    }

    /// Stops FFmpeg and removes the partial output. Nothing is left behind.
    pub fn abort(mut self) {
        self.kill_and_clean();
    }

    fn stop_after_failure(&mut self) -> FfmpegError {
        drop(self.stdin.take());
        let stderr = match self.child.take() {
            Some(mut child) => {
                let _ = child.wait();
                self.join_stderr()
            }
            None => String::new(),
        };
        self.join_progress();
        let _ = fs::remove_file(&self.partial);
        FfmpegError::Failed {
            tool: "ffmpeg",
            stderr,
        }
    }

    fn kill_and_clean(&mut self) {
        drop(self.stdin.take());
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = self.join_stderr();
        self.join_progress();
        let _ = fs::remove_file(&self.partial);
    }

    fn join_stderr(&mut self) -> String {
        self.stderr
            .take()
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default()
    }

    fn join_progress(&mut self) {
        if let Some(handle) = self.progress_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for EncodeProcess {
    fn drop(&mut self) {
        // A process that is dropped without finishing is cancelled.
        if self.child.is_some() {
            self.kill_and_clean();
        }
    }
}
