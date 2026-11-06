// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Planning the videos of a folder, one after the other, with the same settings.
//!
//! A queue in an application, or a batch command, asks for the plan of each flight of a folder. The files of each
//! flight are found next to its video (see [`crate::discover`]); the settings are the same for all.

use std::io;
use std::path::{Path, PathBuf};

use super::{OutputPlan, PlanInputs, RenderPlan, build_plan};
use crate::config::Settings;
use crate::discover::{self, Flight};
use crate::ffmpeg::{EncoderInfo, FfmpegError, Tools, probe};
use crate::font::FontSheet;
use crate::osd::{OsdError, OsdFile};
use crate::paths::file_name;
use crate::srt::{SrtError, SrtFile};
use crate::timeline::SyncOptions;

/// What every video of a folder is rendered with: the settings and the choices, apart from the files.
#[derive(Clone)]
pub struct FlightDefaults {
    /// What to draw, and how.
    pub settings: Settings,
    /// The encoders FFmpeg has, to find the options of the chosen one.
    pub encoders: Vec<EncoderInfo>,
    /// The OSD font.
    pub font: Option<FontSheet>,
    /// Synchronisation of each video and its OSD. A manual scale belongs to one video, so it should not be given
    /// here.
    pub sync: SyncOptions,
    /// Replace an output that exists.
    pub overwrite: bool,
    /// Replace the sound of a video that already has some.
    pub replace_audio: bool,
}

/// Why a video of a folder was left out.
#[derive(Debug, thiserror::Error)]
pub enum FlightError {
    /// Another plan writes the same file.
    #[error("its output file is already planned")]
    AlreadyPlanned,
    /// The output exists, and replacing it was not asked for.
    #[error("its output file exists already")]
    OutputExists,
    /// The OSD or the sound needs an `.osd` file, and none is next to the video.
    #[error("there is no .osd file next to it")]
    NoOsd,
    /// The SRT data are wanted, and no `.srt` file is next to the video.
    #[error("there is no .srt file next to it")]
    NoSrt,
    /// The video could not be read.
    #[error(transparent)]
    Video(#[from] FfmpegError),
    /// The `.osd` file could not be read.
    #[error(transparent)]
    Osd(#[from] OsdError),
    /// The `.srt` file could not be read.
    #[error(transparent)]
    Srt(#[from] SrtError),
    /// The plan is not complete.
    #[error("{}", .0.join(" "))]
    Plan(Vec<String>),
}

/// A video that was left out, and why.
#[derive(Debug)]
pub struct Skipped {
    /// The video.
    pub video: PathBuf,
    /// Why it was left out.
    pub reason: FlightError,
}

impl Skipped {
    /// The name of the video and the reason, in one line for the user.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("{}: {}", file_name(&self.video), self.reason)
    }
}

/// The plans of the videos of a folder, and the videos that could not be planned.
#[derive(Debug, Default)]
pub struct FolderPlans {
    /// The plans, in the order of the names of the videos.
    pub plans: Vec<RenderPlan>,
    /// The videos left out.
    pub skipped: Vec<Skipped>,
}

/// Plans one flight. `planned` are the outputs that other plans already write, so two videos never write one file.
pub fn plan_flight(
    tools: &Tools,
    flight: &Flight,
    defaults: &FlightDefaults,
    planned: &[PathBuf],
) -> Result<RenderPlan, FlightError> {
    let settings = &defaults.settings;
    let output = OutputPlan::default_path(&flight.video, false);
    if planned.contains(&output) {
        return Err(FlightError::AlreadyPlanned);
    }
    if !defaults.overwrite && output.exists() {
        return Err(FlightError::OutputExists);
    }
    let info = probe(tools, &flight.video)?;
    let osd = if settings.draw_osd || settings.make_sound {
        let path = flight.companions.osd.as_ref().ok_or(FlightError::NoOsd)?;
        Some(OsdFile::open(path)?)
    } else {
        None
    };
    let srt = if settings.draw_srt {
        let path = flight.companions.srt.as_ref().ok_or(FlightError::NoSrt)?;
        Some(SrtFile::open(path)?)
    } else {
        None
    };
    build_plan(&PlanInputs {
        video: flight.video.clone(),
        info,
        osd: osd.as_ref(),
        osd_path: flight.companions.osd.clone(),
        srt: srt.as_ref(),
        font: defaults.font.as_ref(),
        settings,
        encoders: &defaults.encoders,
        sync: defaults.sync,
        window: None,
        output,
        overwrite: defaults.overwrite,
        replace_audio: defaults.replace_audio,
    })
    .map_err(FlightError::Plan)
}

/// Plans every video of a folder (see [`discover::flights_in`]). A video that cannot be planned is left out, with the
/// reason, and the others go on. `planned` are the outputs that other plans already write.
pub fn plan_folder(
    tools: &Tools,
    folder: &Path,
    defaults: &FlightDefaults,
    planned: &[PathBuf],
) -> io::Result<FolderPlans> {
    let mut result = FolderPlans::default();
    let mut planned = planned.to_vec();
    for flight in discover::flights_in(folder)? {
        match plan_flight(tools, &flight, defaults, &planned) {
            Ok(plan) => {
                planned.push(plan.output.path.clone());
                result.plans.push(plan);
            }
            Err(reason) => result.skipped.push(Skipped {
                video: flight.video,
                reason,
            }),
        }
    }
    Ok(result)
}
