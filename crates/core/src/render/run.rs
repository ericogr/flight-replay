// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// The overlay pipeline is based on walksnail-osd-tool, backend/src/ffmpeg/render.rs (GPL-3.0-only), Copyright (C)
// Alexander van Saase, and the sound pipeline on fpv-motorsound, internal/pipeline/render.go (Apache-2.0, Copyright
// 2026 EricoGR). Modified for Flight Replay: one pipeline for both, with progress events, cancellation and checks.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ab_glyph::FontRef;
use image::RgbaImage;

use super::events::{CancelToken, Event, Stage};
use super::plan::{Mode, Overwrite, RenderPlan, ResolvedWindow, VideoOutput};
use crate::assets::SRT_FONT;
use crate::audio::{pcm16le, write_wav_header};
use crate::ffmpeg::{
    AudioInput, DecodeOptions, EncodeJob, EncodeProcess, FfmpegError, FrameReader, Tools,
    VideoEncoding, VideoInput, probe,
};
use crate::font::{CharacterSize, ScaledFont};
use crate::osd::OsdFile;
use crate::overlay::{chroma_background, draw_osd, draw_srt};
use crate::synth::{
    Engine, EngineError, RenderStats, SignalError, SignalSet, SoundSettings, frames_for,
};
use crate::telemetry::{LoadOptions, Recording, RecordingError};
use crate::timeline::{SyncTolerance, Timeline};

/// Frames of sound made at a time.
const BLOCK_FRAMES: u64 = 8192;
/// A progress event is sent every this many video frames.
const PROGRESS_EVERY: u64 = 15;
/// How far the sound and the picture of the output may differ in length before the check warns, in seconds. AAC adds
/// a little padding, so a few tens of milliseconds are normal.
const DURATION_TOLERANCE_S: f64 = 0.05;

/// Why a render did not finish.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The plan cannot run.
    #[error("the render cannot start:\n  - {}", .0.join("\n  - "))]
    InvalidPlan(Vec<String>),
    /// The output exists and the plan does not allow replacing it.
    #[error("{} already exists", .0.display())]
    OutputExists(PathBuf),
    /// The video has sound and the plan does not allow replacing it.
    #[error("the video already has sound. Allow replacing it to add the new sound")]
    SourceHasAudio,
    /// There is not enough free space for the temporary sound file.
    #[error(
        "the sound of this render needs about {} MB of free space in {}, and only {} MB are free",
        .needed / 1_000_000, .dir.display(), .available / 1_000_000
    )]
    NotEnoughSpace {
        /// Bytes the sound file needs.
        needed: u64,
        /// Bytes that are free.
        available: u64,
        /// The folder the file goes in.
        dir: PathBuf,
    },
    /// The render was cancelled. Nothing is left behind.
    #[error("the render was cancelled")]
    Cancelled,
    /// FFmpeg failed.
    #[error(transparent)]
    Ffmpeg(#[from] FfmpegError),
    /// The telemetry could not be read.
    #[error(transparent)]
    Recording(#[from] RecordingError),
    /// The recording lacks what the sound needs.
    #[error(transparent)]
    Signals(#[from] SignalError),
    /// The sound settings are not valid.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// A file could not be written.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// What the output file turned out to be.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputCheck {
    /// Codec of the video stream.
    pub codec: String,
    /// Duration of the video stream, in seconds.
    pub video_duration_s: f64,
    /// Duration of the sound, in seconds, if there is any.
    pub audio_duration_s: Option<f64>,
    /// Number of video frames.
    pub frames: u64,
}

/// What a finished render produced.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The output file.
    pub output: PathBuf,
    /// The stretch of the video that was rendered.
    pub window: ResolvedWindow,
    /// How the render was done.
    pub mode: Mode,
    /// Peak and level of the sound, if there is one.
    pub audio_stats: Option<RenderStats>,
    /// Everything worth telling the user.
    pub warnings: Vec<String>,
    /// What the output file holds.
    pub check: Option<OutputCheck>,
    /// How long the render took.
    pub elapsed: Duration,
}

/// Collects the events and the warnings of a render.
struct Reporter<'a> {
    on_event: &'a mut dyn FnMut(Event),
    warnings: Vec<String>,
}

impl Reporter<'_> {
    fn stage(&mut self, stage: Stage) {
        (self.on_event)(Event::Stage(stage));
    }

    fn progress(&mut self, stage: Stage, done: u64, total: u64) {
        (self.on_event)(Event::Progress { stage, done, total });
    }

    fn warn(&mut self, text: impl Into<String>) {
        let text = text.into();
        (self.on_event)(Event::Warning(text.clone()));
        self.warnings.push(text);
    }
}

/// Renders the plan: the sound alone with the video copied, or the picture drawn and encoded again, with or without
/// sound.
///
/// `on_event` is called with the progress. When `cancel` is cancelled, the render stops and removes the files it
/// wrote.
pub fn render(
    tools: &Tools,
    plan: &RenderPlan,
    cancel: &CancelToken,
    on_event: &mut dyn FnMut(Event),
) -> Result<Outcome, RenderError> {
    let started = Instant::now();
    plan.validate().map_err(RenderError::InvalidPlan)?;
    if plan.output.path.exists() && plan.output.overwrite == Overwrite::Refuse {
        return Err(RenderError::OutputExists(plan.output.path.clone()));
    }
    if plan.sound.is_some() && plan.info.audio_streams > 0 && !plan.output.replace_audio {
        return Err(RenderError::SourceHasAudio);
    }
    let mut reporter = Reporter {
        on_event,
        warnings: Vec::new(),
    };
    reporter.stage(Stage::Preparing);
    let window = plan.resolved_window();
    let mode = plan.mode();

    let timeline = plan
        .osd
        .as_ref()
        .map(|osd| check_sync(plan, osd, &mut reporter));
    let sound = prepare_sound(plan, &mut reporter)?;
    if mode == Mode::Overlay && plan.sound.is_none() && plan.info.audio_streams > 0 {
        reporter.warn("the sound of the input video is not kept in the output");
    }
    if let Some(parent) = plan
        .output
        .path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let (output, audio_stats) = match mode {
        Mode::SoundOnly => {
            let (settings, signals) = sound
                .as_ref()
                .expect("a plan without drawing has sound: validate checked");
            let timeline = timeline.expect("the sound has an OSD file");
            let stats = sound_only(
                tools,
                plan,
                window,
                timeline,
                settings,
                signals,
                cancel,
                &mut reporter,
            )?;
            (plan.output.path.clone(), Some(stats))
        }
        Mode::Overlay => {
            let sound = sound
                .as_ref()
                .map(|(settings, signals)| (*settings, signals));
            overlay(tools, plan, window, timeline, sound, cancel, &mut reporter)?
        }
    };

    reporter.stage(Stage::Checking);
    let check = match check_output(tools, &output, window) {
        Ok((check, warnings)) => {
            for warning in warnings {
                reporter.warn(warning);
            }
            Some(check)
        }
        Err(error) => {
            reporter.warn(format!(
                "the output could not be opened to check it: {error}"
            ));
            None
        }
    };
    Ok(Outcome {
        output,
        window,
        mode,
        audio_stats,
        warnings: reporter.warnings,
        check,
        elapsed: started.elapsed(),
    })
}

/// Maps video time to telemetry time, and warns if the telemetry does not span the video.
fn check_sync(plan: &RenderPlan, osd: &OsdFile, reporter: &mut Reporter<'_>) -> Timeline {
    let timeline = Timeline::new(plan.sync, plan.info.duration_s, osd.duration_s());
    let end_s = osd
        .frames
        .last()
        .map_or(0.0, |f| f64::from(f.time_ms) / 1000.0);
    for warning in timeline.check(plan.info.duration_s, end_s, SyncTolerance::default()) {
        reporter.warn(warning.to_string());
    }
    timeline
}

/// Reads the telemetry for the sound, if the plan has sound, and warns about what is missing.
fn prepare_sound<'a>(
    plan: &'a RenderPlan,
    reporter: &mut Reporter<'_>,
) -> Result<Option<(&'a SoundSettings, SignalSet)>, RenderError> {
    let (Some(settings), Some(osd)) = (&plan.sound, &plan.osd) else {
        return Ok(None);
    };
    let recording = Recording::decode(osd, LoadOptions::default())?;
    let name = plan
        .osd_path
        .as_deref()
        .and_then(Path::file_name)
        .map_or_else(
            || "the .osd".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
    let signals = SignalSet::collect(&recording.table, settings, &name, recording.frames)?;
    for warning in &signals.warnings {
        reporter.warn(warning.clone());
    }
    Ok(Some((settings, signals)))
}

/// The video stream is copied and the sound is written to FFmpeg as it is made.
#[allow(clippy::too_many_arguments)]
fn sound_only(
    tools: &Tools,
    plan: &RenderPlan,
    window: ResolvedWindow,
    timeline: Timeline,
    settings: &SoundSettings,
    signals: &SignalSet,
    cancel: &CancelToken,
    reporter: &mut Reporter<'_>,
) -> Result<RenderStats, RenderError> {
    let mut engine = Engine::new(
        settings.clone(),
        signals.controls(),
        timeline,
        window.start_s,
    )?;
    let total = frames_for(window.duration_s, settings.audio.sample_rate) as u64;
    let job = EncodeJob {
        video: VideoInput::Copy {
            path: plan.video.clone(),
            start_s: window.start_s,
        },
        audio: AudioInput::PcmStdin {
            sample_rate: settings.audio.sample_rate,
            channels: settings.audio.channels,
        },
        encoding: None,
        audio_bitrate_kbps: plan.output.audio_bitrate_kbps,
        faststart: plan.output.faststart,
        duration_s: Some(window.duration_s),
        output: plan.output.path.clone(),
    };
    let mut process = EncodeProcess::spawn(tools, &job)?;
    reporter.stage(Stage::Audio);
    let mut done = 0;
    while done < total {
        if cancel.is_cancelled() {
            process.abort();
            return Err(RenderError::Cancelled);
        }
        let block = BLOCK_FRAMES.min(total - done);
        process.write(&pcm16le(&engine.render_s16(block as usize)))?;
        done += block;
        reporter.progress(Stage::Audio, done, total);
    }
    reporter.stage(Stage::Finishing);
    process.finish()?;
    Ok(engine.stats())
}

/// A file that is removed when it goes out of scope.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// What is drawn on every frame.
struct Layers<'a> {
    plan: &'a RenderPlan,
    font: Option<ScaledFont>,
    srt_font: FontRef<'static>,
    timeline: Option<Timeline>,
}

impl<'a> Layers<'a> {
    fn new(plan: &'a RenderPlan, timeline: Option<Timeline>) -> Self {
        Self {
            plan,
            font: plan.osd_layer.as_ref().map(|layer| {
                layer
                    .font
                    .scaled(CharacterSize::for_video_height(plan.info.height))
            }),
            srt_font: FontRef::try_from_slice(SRT_FONT).expect("the bundled font is valid"),
            timeline,
        }
    }

    /// Draws the OSD and the SRT data of video time `time_s` on the picture.
    fn draw(&self, canvas: &mut RgbaImage, time_s: f64) {
        if let (Some(layer), Some(font), Some(osd), Some(timeline)) = (
            &self.plan.osd_layer,
            &self.font,
            &self.plan.osd,
            self.timeline,
        ) {
            let telemetry_ms = timeline.telemetry_time(time_s) * 1000.0;
            draw_osd(
                canvas,
                &osd.frames[osd.frame_index_at(telemetry_ms)],
                font,
                &layer.options,
            );
        }
        if let Some(layer) = &self.plan.srt_layer
            && let Some(data) = layer.file.cue_at(time_s).and_then(|cue| cue.data.as_ref())
        {
            draw_srt(canvas, data, &self.srt_font, &layer.options);
        }
    }
}

/// The picture is drawn on and encoded again. The sound, if any, is made first, into a temporary WAV file.
fn overlay(
    tools: &Tools,
    plan: &RenderPlan,
    window: ResolvedWindow,
    timeline: Option<Timeline>,
    sound: Option<(&SoundSettings, &SignalSet)>,
    cancel: &CancelToken,
    reporter: &mut Reporter<'_>,
) -> Result<(PathBuf, Option<RenderStats>), RenderError> {
    let VideoOutput::Encode(choice) = &plan.output.video else {
        unreachable!("validate: drawing needs an encoder");
    };
    let info = &plan.info;
    let rate = info.nominal_frame_rate;
    let total = (window.duration_s * rate).round().max(1.0) as u64;

    // The sound first, into a file next to the output. The guard removes it on every way out.
    let mut audio_stats = None;
    let (audio, _wav_guard) = match sound {
        Some((settings, signals)) => {
            let path = temp_wav_path(&plan.output.path);
            let wav_bytes = frames_for(window.duration_s, settings.audio.sample_rate) as u64
                * u64::from(settings.audio.channels)
                * 2;
            let dir = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            check_free_space(dir, wav_bytes)?;
            let guard = TempFile(path.clone());
            let timeline = timeline.expect("the sound has an OSD file");
            audio_stats = Some(write_wav(
                &path, settings, signals, timeline, window, cancel, reporter,
            )?);
            (AudioInput::WavFile { path }, Some(guard))
        }
        None => (AudioInput::None, None),
    };

    let job = EncodeJob {
        video: VideoInput::Raw {
            width: info.width,
            height: info.height,
            frame_rate: rate,
        },
        audio,
        encoding: Some(VideoEncoding {
            encoder: choice.name.clone(),
            bitrate_mbps: choice.bitrate_mbps,
            extra_args: choice.extra_args.clone(),
            upscale_1440p: choice.upscale_1440p,
        }),
        audio_bitrate_kbps: plan.output.audio_bitrate_kbps,
        faststart: plan.output.faststart,
        duration_s: Some(window.duration_s),
        output: plan.output.path.clone(),
    };
    let mut process = EncodeProcess::spawn(tools, &job)?;
    // Frames at a constant rate: a gap in the video repeats the previous frame, so frame n is at time n / rate.
    let mut reader = match plan.chroma_key {
        Some(_) => None,
        None => Some(FrameReader::open(
            tools,
            &plan.video,
            &DecodeOptions {
                width: info.width,
                height: info.height,
                start_s: window.start_s,
                duration_s: Some(window.duration_s),
                constant_rate: Some(rate),
            },
        )?),
    };

    let layers = Layers::new(plan, timeline);
    reporter.stage(Stage::Video);
    let mut buffer: Vec<u8> = Vec::new();
    let mut index = 0u64;
    while index < total {
        if cancel.is_cancelled() {
            process.abort();
            if let Some(reader) = reader {
                reader.cancel();
            }
            return Err(RenderError::Cancelled);
        }
        let mut canvas = match (&mut reader, plan.chroma_key) {
            (Some(reader), _) => {
                if !reader.read_frame(&mut buffer)? {
                    break; // the video ended a frame early
                }
                RgbaImage::from_raw(info.width, info.height, std::mem::take(&mut buffer))
                    .expect("the reader gives whole frames")
            }
            (None, Some(color)) => chroma_background(info.width, info.height, color),
            (None, None) => unreachable!("a plan without a chroma key reads the video"),
        };
        layers.draw(&mut canvas, window.start_s + index as f64 / rate);

        process.write(canvas.as_raw())?;
        buffer = canvas.into_raw();
        index += 1;
        if index.is_multiple_of(PROGRESS_EVERY) || index == total {
            reporter.progress(Stage::Video, index, total);
        }
    }
    if let Some(reader) = reader {
        reader.cancel(); // the rest of the video is not needed
    }
    reporter.stage(Stage::Finishing);
    process.finish()?;
    Ok((plan.output.path.clone(), audio_stats))
}

/// Stops before the sound is made if the folder of the temporary sound file cannot hold it. A folder whose free
/// space cannot be read is not blocked.
fn check_free_space(dir: &Path, needed: u64) -> Result<(), RenderError> {
    match fs4::available_space(dir) {
        Ok(available) if !has_room(available, needed) => Err(RenderError::NotEnoughSpace {
            needed,
            available,
            dir: dir.to_path_buf(),
        }),
        _ => Ok(()),
    }
}

/// Whether `available` bytes hold `needed` bytes with a margin of 5 %.
fn has_room(available: u64, needed: u64) -> bool {
    available >= needed + needed / 20
}

/// Where the temporary sound file of a render goes: next to the output.
fn temp_wav_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_owned();
    name.push(".audio.tmp.wav");
    PathBuf::from(name)
}

/// Makes the sound of the window and writes it as a WAV file.
fn write_wav(
    path: &Path,
    settings: &SoundSettings,
    signals: &SignalSet,
    timeline: Timeline,
    window: ResolvedWindow,
    cancel: &CancelToken,
    reporter: &mut Reporter<'_>,
) -> Result<RenderStats, RenderError> {
    let mut engine = Engine::new(
        settings.clone(),
        signals.controls(),
        timeline,
        window.start_s,
    )?;
    let total = frames_for(window.duration_s, settings.audio.sample_rate) as u64;
    let channels = u64::from(settings.audio.channels);
    let mut file = BufWriter::new(File::create(path)?);
    write_wav_header(
        &mut file,
        settings.audio.sample_rate,
        settings.audio.channels,
        total * channels * 2,
    )?;
    reporter.stage(Stage::Audio);
    let mut done = 0;
    while done < total {
        if cancel.is_cancelled() {
            return Err(RenderError::Cancelled);
        }
        let block = BLOCK_FRAMES.min(total - done);
        file.write_all(&pcm16le(&engine.render_s16(block as usize)))?;
        done += block;
        reporter.progress(Stage::Audio, done, total);
    }
    file.flush()?;
    Ok(engine.stats())
}

/// Opens the output to see what it holds, and compares the sound and the picture.
fn check_output(
    tools: &Tools,
    path: &Path,
    window: ResolvedWindow,
) -> Result<(OutputCheck, Vec<String>), FfmpegError> {
    let info = probe(tools, path)?;
    let mut warnings = Vec::new();
    if let Some(audio) = info.audio_duration_s
        && (audio - info.duration_s).abs() > DURATION_TOLERANCE_S
    {
        warnings.push(format!(
            "the sound lasts {audio:.3} s and the picture {:.3} s in the output",
            info.duration_s
        ));
    }
    if (info.duration_s - window.duration_s).abs() > 0.5 {
        warnings.push(format!(
            "the output lasts {:.1} s, but {:.1} s were expected",
            info.duration_s, window.duration_s
        ));
    }
    let check = OutputCheck {
        codec: info.codec,
        video_duration_s: info.duration_s,
        audio_duration_s: info.audio_duration_s,
        frames: info.frame_count,
    };
    Ok((check, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_a_margin_over_the_size_of_the_file() {
        assert!(has_room(1_050, 1_000));
        assert!(!has_room(1_049, 1_000));
        assert!(has_room(0, 0));
    }

    #[test]
    fn the_free_space_of_a_real_folder_is_readable() {
        let dir = std::env::temp_dir();
        assert!(check_free_space(&dir, 1).is_ok());
        let error = check_free_space(&dir, u64::MAX / 4).unwrap_err();
        assert!(matches!(error, RenderError::NotEnoughSpace { .. }));
        assert!(error.to_string().contains("MB of free space"));
    }
}
