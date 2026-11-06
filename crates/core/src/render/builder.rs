// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Making a [`RenderPlan`] from what the user chose.
//!
//! The wizard, the queue and a future command-line tool all choose the same things (the files, the settings and the
//! output), so this is the one place where they become a plan.

use std::path::PathBuf;

use super::{
    EncoderChoice, OsdLayer, OutputPlan, Overwrite, RenderPlan, SrtLayer, VideoOutput, Window,
};
use crate::config::{OutputSettings, Settings};
use crate::ffmpeg::{EncoderInfo, VideoInfo};
use crate::font::FontSheet;
use crate::osd::OsdFile;
use crate::srt::SrtFile;
use crate::timeline::SyncOptions;

/// What a plan is made from.
pub struct PlanInputs<'a> {
    /// The video.
    pub video: PathBuf,
    /// What is known about it.
    pub info: VideoInfo,
    /// The OSD recording (the joined one, when the video has several).
    pub osd: Option<&'a OsdFile>,
    /// The path of the `.osd` file, for messages.
    pub osd_path: Option<PathBuf>,
    /// The subtitles.
    pub srt: Option<&'a SrtFile>,
    /// The OSD font.
    pub font: Option<&'a FontSheet>,
    /// What to draw, and how.
    pub settings: &'a Settings,
    /// The encoders FFmpeg has, to find the options of the chosen one.
    pub encoders: &'a [EncoderInfo],
    /// Synchronisation of the video and the OSD.
    pub sync: SyncOptions,
    /// Render only a stretch of the video.
    pub window: Option<Window>,
    /// The output file.
    pub output: PathBuf,
    /// Replace the output if it exists.
    pub overwrite: bool,
    /// Replace the sound of a video that already has some.
    pub replace_audio: bool,
}

/// The encoder the settings name, with the options FFmpeg needs for it. `None` when none is chosen.
#[must_use]
pub fn encoder_choice(output: &OutputSettings, encoders: &[EncoderInfo]) -> Option<EncoderChoice> {
    if output.encoder.is_empty() {
        return None;
    }
    let extra_args = encoders
        .iter()
        .find(|e| e.name == output.encoder)
        .map(|e| e.extra_args.clone())
        .unwrap_or_default();
    Some(EncoderChoice {
        name: output.encoder.clone(),
        extra_args,
        bitrate_mbps: output.bitrate_mbps,
        upscale_1440p: output.upscale_1440p,
    })
}

/// Builds the plan, or says everything that is wrong with it, in words for the user.
pub fn build_plan(inputs: &PlanInputs<'_>) -> Result<RenderPlan, Vec<String>> {
    let settings = inputs.settings;
    let output = &settings.output;
    let mut problems = Vec::new();
    let draws = settings.draw_osd || settings.draw_srt;

    let video_output = if draws {
        if let Some(choice) = encoder_choice(output, inputs.encoders) {
            VideoOutput::Encode(choice)
        } else {
            problems.push("Choose a video encoder.".to_owned());
            VideoOutput::Copy
        }
    } else {
        VideoOutput::Copy
    };

    let osd_layer = match (inputs.font, settings.draw_osd) {
        (Some(font), true) => Some(OsdLayer {
            font: font.clone(),
            options: settings.osd.clone(),
        }),
        (None, true) => {
            problems.push("Choose the OSD font file.".to_owned());
            None
        }
        _ => None,
    };
    let srt_layer = match (inputs.srt, settings.draw_srt) {
        (Some(file), true) => Some(SrtLayer {
            file: file.clone(),
            options: settings.srt.clone(),
        }),
        (None, true) => {
            problems.push("The SRT data cannot be drawn without an .srt file.".to_owned());
            None
        }
        _ => None,
    };

    let needs_osd = settings.draw_osd || settings.make_sound;
    let plan = RenderPlan {
        video: inputs.video.clone(),
        info: inputs.info.clone(),
        osd: needs_osd.then(|| inputs.osd.cloned()).flatten(),
        osd_path: inputs.osd_path.clone(),
        osd_layer,
        srt_layer,
        chroma_key: (output.chroma_key && draws).then_some(output.chroma_key_color),
        sound: settings.make_sound.then(|| settings.sound.clone()),
        sync: inputs.sync,
        window: inputs.window,
        output: OutputPlan {
            path: inputs.output.clone(),
            video: video_output,
            audio_bitrate_kbps: output.audio_bitrate_kbps,
            faststart: output.faststart,
            overwrite: if inputs.overwrite {
                Overwrite::Replace
            } else {
                Overwrite::Refuse
            },
            replace_audio: inputs.replace_audio,
        },
    };
    match plan.validate() {
        Ok(()) if problems.is_empty() => Ok(plan),
        Ok(()) => Err(problems),
        Err(more) => {
            problems.extend(more);
            Err(problems)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg::Codec;

    fn info() -> VideoInfo {
        VideoInfo {
            codec: "h264".into(),
            width: 1280,
            height: 720,
            frame_rate: 60.0,
            nominal_frame_rate: 60.0,
            duration_s: 3.0,
            frame_count: 180,
            audio_streams: 0,
            audio_duration_s: None,
        }
    }

    fn encoders() -> Vec<EncoderInfo> {
        vec![EncoderInfo {
            name: "libx264".into(),
            codec: Codec::H264,
            hardware: false,
            extra_args: vec!["-preset".into(), "fast".into()],
            works: true,
        }]
    }

    fn inputs<'a>(settings: &'a Settings, encoders: &'a [EncoderInfo]) -> PlanInputs<'a> {
        PlanInputs {
            video: "a.mp4".into(),
            info: info(),
            osd: None,
            osd_path: None,
            srt: None,
            font: None,
            settings,
            encoders,
            sync: SyncOptions::default(),
            window: None,
            output: "out.mp4".into(),
            overwrite: false,
            replace_audio: false,
        }
    }

    #[test]
    fn the_encoder_comes_with_its_own_options() {
        let settings = Settings::default();
        let choice = encoder_choice(&settings.output, &encoders()).unwrap();
        assert_eq!(choice.name, "libx264");
        assert_eq!(choice.extra_args, ["-preset", "fast"]);

        let mut none = settings.output.clone();
        none.encoder.clear();
        assert_eq!(encoder_choice(&none, &encoders()), None);
    }

    #[test]
    fn a_plan_that_draws_needs_a_font_and_the_files_it_draws() {
        let encoders = encoders();
        let settings = Settings {
            draw_osd: true,
            draw_srt: true,
            make_sound: false,
            ..Settings::default()
        };
        let Err(problems) = build_plan(&inputs(&settings, &encoders)) else {
            panic!("nothing to draw with");
        };
        assert!(problems.iter().any(|p| p.contains("font")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains(".srt")), "{problems:?}");
    }

    #[test]
    fn a_plan_that_draws_needs_an_encoder() {
        let mut settings = Settings {
            draw_srt: true,
            draw_osd: false,
            make_sound: false,
            ..Settings::default()
        };
        settings.output.encoder.clear();
        let problems = build_plan(&inputs(&settings, &[]))
            .err()
            .expect("no encoder");
        assert!(
            problems.iter().any(|p| p.contains("encoder")),
            "{problems:?}"
        );
    }
}
