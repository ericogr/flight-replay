// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Planning the videos of a folder, one after the other, with the same settings.
//!
//! A queue in an application, or a batch command, asks for the plan of each flight of a folder. The files of each
//! flight are found next to its video (see [`crate::discover`]); the settings are the same for all.

use std::io;
use std::path::{Path, PathBuf};

use super::{Availability, Omission, OutputPlan, PlanInputs, RenderPlan, build_plan};
use crate::config::Settings;
use crate::discover::{self, Flight};
use crate::ffmpeg::{EncoderInfo, FfmpegError, Tools, probe};
use crate::font::FontSheet;
use crate::osd::{OsdError, OsdFile};
use crate::paths::file_name;
use crate::srt::{SrtError, SrtFile};
use crate::synth::{SignalSet, SoundSettings};
use crate::telemetry::{LoadOptions, Recording};
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
    /// Nothing the settings ask for can be made with the files next to the video.
    #[error("{}", nothing_to_add(.0))]
    NothingToAdd(Vec<Omission>),
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

/// A video that was planned without some of what the settings ask for, and why.
#[derive(Debug)]
pub struct LeftOut {
    /// The video.
    pub video: PathBuf,
    /// What is left out of its render.
    pub parts: Vec<Omission>,
}

impl LeftOut {
    /// The name of the video and what is left out of it, in one line for the user.
    #[must_use]
    pub fn describe(&self) -> String {
        let parts: Vec<String> = self.parts.iter().map(ToString::to_string).collect();
        format!("{}: {}", file_name(&self.video), parts.join(" "))
    }
}

/// A flight that was planned: the plan, and what the files could not give.
#[derive(Debug)]
pub struct Planned {
    /// The plan.
    pub plan: RenderPlan,
    /// What the settings ask for and the files next to the video cannot give. It is not in the plan.
    pub left_out: Vec<Omission>,
}

/// The plans of the videos of a folder, and the videos that could not be planned.
#[derive(Debug, Default)]
pub struct FolderPlans {
    /// The plans, in the order of the names of the videos.
    pub plans: Vec<RenderPlan>,
    /// The videos planned without some of what was asked for.
    pub left_out: Vec<LeftOut>,
    /// The videos left out.
    pub skipped: Vec<Skipped>,
}

/// Plans one flight. `planned` are the outputs that other plans already write, so two videos never write one file.
///
/// What the files next to the video cannot give is left out of the plan, with the reason (see [`Availability`]): a
/// video with no `.osd` file still gets the SRT data, and one whose OSD does not show the throttle still gets the
/// OSD. The flight is left out only when nothing is left to add.
pub fn plan_flight(
    tools: &Tools,
    flight: &Flight,
    defaults: &FlightDefaults,
    planned: &[PathBuf],
) -> Result<Planned, FlightError> {
    let settings = &defaults.settings;
    let output = OutputPlan::default_path(&flight.video, false);
    if planned.contains(&output) {
        return Err(FlightError::AlreadyPlanned);
    }
    if !defaults.overwrite && output.exists() {
        return Err(FlightError::OutputExists);
    }
    let info = probe(tools, &flight.video)?;
    let osd = match &flight.companions.osd {
        Some(path) if settings.draw_osd || settings.make_sound => Some(OsdFile::open(path)?),
        _ => None,
    };
    let srt = match &flight.companions.srt {
        Some(path) if settings.draw_srt => Some(SrtFile::open(path)?),
        _ => None,
    };
    let mut availability = Availability::new(osd.as_ref(), defaults.font.is_some(), srt.is_some());
    if settings.make_sound
        && let Some(osd) = &osd
    {
        let name = flight
            .companions
            .osd
            .as_deref()
            .map_or_else(String::new, file_name);
        availability = availability.with_telemetry(telemetry_problem(osd, &settings.sound, &name));
    }
    let left_out = availability.omissions(settings);
    let settings = availability.restrict(settings);
    if !settings.draw_osd && !settings.draw_srt && !settings.make_sound {
        return Err(FlightError::NothingToAdd(left_out));
    }
    let plan = build_plan(&PlanInputs {
        video: flight.video.clone(),
        info,
        osd: osd.as_ref(),
        osd_path: flight.companions.osd.clone(),
        srt: srt.as_ref(),
        font: defaults.font.as_ref(),
        settings: &settings,
        encoders: &defaults.encoders,
        sync: defaults.sync,
        window: None,
        output,
        overwrite: defaults.overwrite,
        replace_audio: defaults.replace_audio,
    })
    .map_err(FlightError::Plan)?;
    Ok(Planned { plan, left_out })
}

/// Reads the telemetry of the recording and says why the sound cannot follow it, if it cannot: the recording does
/// not show the throttle, say.
fn telemetry_problem(osd: &OsdFile, sound: &SoundSettings, name: &str) -> Option<String> {
    let recording = match Recording::decode(osd, LoadOptions::default()) {
        Ok(recording) => recording,
        Err(error) => return Some(format!("The telemetry could not be read: {error}.")),
    };
    SignalSet::collect(&recording.table, sound, name, recording.frames)
        .err()
        .map(|error| format!("{error}."))
}

fn nothing_to_add(left_out: &[Omission]) -> String {
    if left_out.is_empty() {
        "nothing is turned on to add".to_owned()
    } else {
        let parts: Vec<String> = left_out.iter().map(ToString::to_string).collect();
        format!("nothing can be added. {}", parts.join(" "))
    }
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
            Ok(Planned { plan, left_out }) => {
                planned.push(plan.output.path.clone());
                if !left_out.is_empty() {
                    result.left_out.push(LeftOut {
                        video: flight.video,
                        parts: left_out,
                    });
                }
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
