// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// The stream-copy mux follows fpv-motorsound's internal/media/mux.go (Apache-2.0, same author), ported to Rust:
// the video is copied, the audio is written to FFmpeg's standard input, and the output is written to a
// `.partial` file that is renamed when FFmpeg succeeds.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;

use super::process::{collect_stderr, kill_tree, read_progress};
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
    /// The video stream of a file, or of several that follow one another, copied without re-encoding (`-c:v copy`).
    /// Several files must have the same codec and picture: FFmpeg joins them as they are.
    Copy {
        /// The files whose video stream is copied, in the order they play. There is at least one.
        paths: Vec<PathBuf>,
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

    /// Where the list of files goes when several videos are copied one after the other (FFmpeg reads them from a
    /// list, with its `concat` demuxer): next to the output. `None` for any other job.
    fn concat_list_path(&self) -> Option<PathBuf> {
        match &self.video {
            VideoInput::Copy { paths, .. } if paths.len() > 1 => {
                let mut name = self.output.as_os_str().to_owned();
                name.push(".parts.txt");
                Some(PathBuf::from(name))
            }
            _ => None,
        }
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
            (VideoInput::Copy { paths, .. }, None) if paths.is_empty() => {
                return Err(FfmpegError::InvalidJob("there is no video to copy"));
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
            VideoInput::Copy { paths, start_s } => {
                if *start_s > 0.0 {
                    args.push("-ss".into());
                    args.push(format!("{start_s:.6}").into());
                }
                if let Some(list) = self.concat_list_path() {
                    push_all(&mut args, ["-f", "concat", "-safe", "0"]);
                    args.push("-i".into());
                    args.push(list.into_os_string());
                } else {
                    args.push("-i".into());
                    args.push(paths[0].as_os_str().to_owned());
                }
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

/// The text of the list that FFmpeg's `concat` demuxer reads: a line for each file. The names are read from the folder
/// of the list, so they are written whole, and a quote in a name is closed, escaped and opened again.
fn concat_list_text(paths: &[PathBuf]) -> String {
    let mut lines = String::new();
    for path in paths {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.clone());
        let quoted = path.to_string_lossy().replace('\'', "'\\''");
        let _ = writeln!(lines, "file '{quoted}'");
    }
    lines
}

/// Removes the list of videos that was written for a job, if there is one.
fn remove_list(list: Option<&Path>) {
    if let Some(list) = list {
        let _ = fs::remove_file(list);
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
    /// The list of the videos that are copied one after the other, removed when the encode ends.
    list: Option<PathBuf>,
}

impl EncodeProcess {
    /// Starts FFmpeg for `job`.
    pub fn spawn(tools: &Tools, job: &EncodeJob) -> Result<Self, FfmpegError> {
        job.validate()?;
        let partial = job.partial_path();
        if let Some(parent) = parent_folder(&job.output) {
            fs::create_dir_all(parent)?;
        }

        let list = job.concat_list_path();
        if let (Some(path), VideoInput::Copy { paths, .. }) = (&list, &job.video) {
            fs::write(path, concat_list_text(paths))?;
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
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                remove_list(list.as_deref());
                return Err(FfmpegError::spawn("ffmpeg")(error));
            }
        };

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
            list,
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
        remove_list(self.list.as_deref());

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
        remove_list(self.list.as_deref());
        let _ = fs::remove_file(&self.partial);
        FfmpegError::Failed {
            tool: "ffmpeg",
            stderr,
        }
    }

    fn kill_and_clean(&mut self) {
        drop(self.stdin.take());
        if let Some(mut child) = self.child.take() {
            kill_tree(&mut child);
            let _ = child.wait();
        }
        let _ = self.join_stderr();
        self.join_progress();
        remove_list(self.list.as_deref());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn copy_job(paths: &[&str]) -> EncodeJob {
        EncodeJob {
            video: VideoInput::Copy {
                paths: paths.iter().map(PathBuf::from).collect(),
                start_s: 12.5,
            },
            audio: AudioInput::PcmStdin {
                sample_rate: 48_000,
                channels: 2,
            },
            encoding: None,
            audio_bitrate_kbps: 192,
            faststart: true,
            duration_s: Some(30.0),
            output: PathBuf::from("/out/flight.mp4"),
        }
    }

    fn arguments(job: &EncodeJob) -> Vec<String> {
        job.args(&job.partial_path())
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn one_video_is_copied_as_it_always_was() {
        let job = copy_job(&["/in/a.mp4"]);
        assert_eq!(job.concat_list_path(), None);
        let args = arguments(&job);
        let at = args.iter().position(|a| a == "-i").unwrap();
        assert_eq!(args[at + 1], "/in/a.mp4");
        assert!(!args.contains(&"concat".to_owned()));
    }

    #[test]
    fn several_videos_are_read_from_a_list_that_starts_the_copy_where_asked() {
        let job = copy_job(&["/in/a.mp4", "/in/it's b.mp4"]);
        let path = job.concat_list_path().expect("a list for two videos");
        assert_eq!(path, PathBuf::from("/out/flight.mp4.parts.txt"));
        // A quote in a name is closed, escaped and opened again, as the list format wants.
        let text = concat_list_text(&[PathBuf::from("/in/a.mp4"), PathBuf::from("/in/it's b.mp4")]);
        assert_eq!(text, "file '/in/a.mp4'\nfile '/in/it'\\''s b.mp4'\n");

        let args = arguments(&job);
        let seek = args.iter().position(|a| a == "-ss").unwrap();
        let input = args.iter().position(|a| a == "-i").unwrap();
        assert!(
            seek < input,
            "the start is an option of the input: {args:?}"
        );
        assert_eq!(
            args[input - 4..=input + 1],
            [
                "-f",
                "concat",
                "-safe",
                "0",
                "-i",
                "/out/flight.mp4.parts.txt"
            ]
        );
    }

    #[test]
    fn a_copy_of_no_video_is_not_a_job() {
        let error = copy_job(&[]).validate().unwrap_err();
        assert!(error.to_string().contains("no video to copy"), "{error}");
    }
}
