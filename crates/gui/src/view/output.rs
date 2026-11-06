// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::fmt;

use iced::widget::{checkbox, column, pick_list, row, text_input};
use iced::{Element, Length};

use super::widgets::{action, hint, labeled, page, problem, section, slider_row};
use crate::app::FlightReplay;
use crate::message::{Message, OutputEdit};

/// An encoder as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EncoderOption {
    name: String,
    label: String,
}

impl fmt::Display for EncoderOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();
    if app.draw_osd_on() || app.draw_srt_on() {
        items.push(section("Video", video(app)));
    } else if app.sound_on() {
        items.push(hint(
            "Only the sound is added, so the video is copied as it is, without encoding it again. There are no video settings.",
        ));
    } else {
        items.push(hint(
            "Nothing is added to the video yet, so there are no video settings.",
        ));
    }
    if app.sound_on() {
        items.push(section("Sound", audio(app)));
    }
    items.push(section("File", file(app)));
    items.push(section("Only a stretch", stretch(app)));
    page(items).into()
}

fn video(app: &FlightReplay) -> Element<'_, Message> {
    let output = &app.settings.output;
    let encoder: Element<'_, Message> = match &app.encoders {
        None => hint("Checking which encoders work…"),
        Some(list) => {
            let options: Vec<EncoderOption> = list
                .iter()
                .filter(|e| e.works || output.show_undetected_encoders)
                .map(|e| EncoderOption {
                    name: e.name.clone(),
                    label: format!(
                        "{} - {} - {}{}",
                        e.name,
                        e.codec,
                        if e.hardware { "hardware" } else { "software" },
                        if e.works { "" } else { " (not detected)" }
                    ),
                })
                .collect();
            let selected = options.iter().find(|o| o.name == output.encoder).cloned();
            pick_list(options, selected, |o| {
                Message::Output(OutputEdit::Encoder(o.name))
            })
            .into()
        }
    };
    column![
        labeled("Encoder", encoder),
        checkbox(output.show_undetected_encoders)
            .label("Also offer encoders that were not detected (use at your own risk)")
            .on_toggle(|v| Message::Output(OutputEdit::ShowUndetected(v))),
        slider_row(
            "Bitrate",
            "Mbps",
            f64::from(output.bitrate_mbps),
            1.0..=150.0,
            1.0,
            "Target bitrate of the video.",
            |v| { Message::Output(OutputEdit::Bitrate(v as u32)) }
        ),
        checkbox(output.upscale_1440p)
            .label("Scale the video to 1440p (better compression on YouTube)")
            .on_toggle(|v| Message::Output(OutputEdit::Upscale(v))),
        checkbox(output.chroma_key)
            .label("Draw on a green background instead of the video (to composite in an editor)")
            .on_toggle(|v| Message::Output(OutputEdit::ChromaKey(v))),
    ]
    .spacing(8)
    .into()
}

fn audio(app: &FlightReplay) -> Element<'_, Message> {
    let output = &app.settings.output;
    column![
        labeled(
            "Audio bitrate (kbps)",
            pick_list(
                [128u32, 192, 256, 320],
                Some(output.audio_bitrate_kbps),
                |v| Message::Output(OutputEdit::AudioBitrate(v))
            ),
        ),
        checkbox(output.faststart)
            .label("Move the index to the start of the file (faster to start playing online)")
            .on_toggle(|v| Message::Output(OutputEdit::Faststart(v))),
    ]
    .spacing(8)
    .into()
}

fn file(app: &FlightReplay) -> Element<'_, Message> {
    let mut items = column![
        labeled(
            "Save as",
            row![
                text_input("Where to save the video", &app.output_path)
                    .on_input(|t| Message::Output(OutputEdit::Path(t)))
                    .width(Length::Fill),
                action("Choose…", Some(Message::BrowseOutput)),
            ]
            .spacing(8),
        ),
        checkbox(app.preferences.overwrite)
            .label("Replace the file if it exists")
            .on_toggle(|v| Message::Output(OutputEdit::Overwrite(v))),
    ]
    .spacing(8);
    if app.sound_on() && app.video.as_ref().is_some_and(|v| v.info.audio_streams > 0) {
        items = items.push(problem("The video already has sound."));
        items = items.push(
            checkbox(app.preferences.replace_audio)
                .label("Replace it with the new sound")
                .on_toggle(|v| Message::Output(OutputEdit::ReplaceAudio(v))),
        );
    }
    items.into()
}

fn stretch(app: &FlightReplay) -> Element<'_, Message> {
    let mut items = column![
        checkbox(app.stretch_on)
            .label("Render only a stretch of the video (a quick test of the whole render)")
            .on_toggle(|v| Message::Output(OutputEdit::Stretch(v))),
    ]
    .spacing(8);
    if app.stretch_on {
        items = items.push(labeled(
            "Starts at (seconds)",
            text_input("0", &app.preferences.stretch.start)
                .on_input(|t| Message::Output(OutputEdit::StretchStart(t)))
                .width(Length::Fixed(120.0)),
        ));
        items = items.push(labeled(
            "Lasts (seconds)",
            text_input("30", &app.preferences.stretch.duration)
                .on_input(|t| Message::Output(OutputEdit::StretchDuration(t)))
                .width(Length::Fixed(120.0)),
        ));
        items = items.push(hint("A video made from the stretch is not cut exactly when the video is copied: it starts at the keyframe before the start."));
    }
    items.into()
}
