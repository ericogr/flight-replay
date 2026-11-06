// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use flight_replay_core::synth::{MOTOR_PRESETS, motor_preset_in_use};
use flight_replay_core::telemetry::{DEFAULT_PROBE_COLUMNS, probe_at};
use iced::Element;
use iced::Length;
use iced::widget::{checkbox, column, pick_list, row, text, text_input};

use super::preview_view::{preview_block, sync_block};
use super::widgets::{action, good, hint, labeled, page, problem, section, slider_row};
use crate::app::FlightReplay;
use crate::message::{Message, SoundEdit};
use crate::sound_params::{self, Group};
use flight_replay_core::display::clock_tenths;

pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    let unavailable = app.sound_unavailable();
    let can_make = app.osd.is_some() && unavailable.is_none();
    let toggle = checkbox(app.settings.make_sound)
        .label("Make the motor sound")
        .on_toggle_maybe(can_make.then_some(Message::MakeSound));

    let mut items = vec![toggle.into()];
    if app.osd.is_none() {
        items.push(hint(
            "There is no .osd file. The sound follows the telemetry stored in it.",
        ));
    }
    if let Some(reason) = unavailable {
        items.push(problem(reason));
    }
    if app.osd.is_some() {
        items.push(section("What the recording gives the sound", coverage(app)));
        items.push(section("Preview of the sound", listen(app)));
        items.push(section("Presets", presets(app)));
        items.push(section("Audio format", format(app)));
        for group in Group::ALL {
            items.push(section(group.title(), parameters(app, group)));
        }
        items.push(sync_block(app));
    }
    page(items).into()
}

fn coverage(app: &FlightReplay) -> Element<'_, Message> {
    if app.busy.telemetry {
        return text("Reading the telemetry…").into();
    }
    let Some(telemetry) = &app.telemetry else {
        return hint("No telemetry has been read.");
    };
    match &telemetry.signals {
        Ok(signals) => {
            let mut lines = column![good(signals.describe())].spacing(4);
            for warning in &signals.warnings {
                lines = lines.push(hint(format!("• {warning}")));
            }
            lines.into()
        }
        Err(error) if error.is_empty() => text("Checking the signals…").into(),
        Err(error) => problem(error.clone()),
    }
}

fn listen(app: &FlightReplay) -> Element<'_, Message> {
    let can_play = app.signals().is_some() && !app.busy.audio && !app.is_playing();
    let controls = row![
        action(
            "Play 10 seconds from here",
            can_play.then_some(Message::PlayAudio)
        ),
        action("Stop", app.is_playing().then_some(Message::StopAudio)),
        text(if app.busy.audio {
            "Making the sound…"
        } else if app.is_playing() {
            "Playing…"
        } else {
            ""
        }),
    ]
    .spacing(10);

    let mut items = column![
        hint("Choose the moment on the slider, then listen. While the sound plays, the picture and the OSD follow it, a few times a second."),
        preview_block(app, false),
        controls,
    ]
    .spacing(10);
    if let Some(error) = &app.audio_error {
        items = items.push(problem(error.clone()));
    }
    if let (Some(telemetry), Some(timeline)) = (&app.telemetry, app.timeline()) {
        let report = probe_at(
            &telemetry.recording.table,
            &timeline,
            app.preview.time_s,
            &DEFAULT_PROBE_COLUMNS,
        );
        items = items.push(hint(format!(
            "The OSD at {} says: {}. Compare it with the picture to check the timing.",
            clock_tenths(app.preview.time_s),
            report.values_text()
        )));
    }
    items.into()
}

/// A kind of motor and propeller in the list, by its place in [`MOTOR_PRESETS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MotorChoice(usize);

impl std::fmt::Display for MotorChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(MOTOR_PRESETS[self.0].name)
    }
}

fn presets(app: &FlightReplay) -> Element<'_, Message> {
    column![motor_presets(app), saved_presets(app)]
        .spacing(16)
        .into()
}

/// What the sound step says when the values no longer are those of a kind of motor.
pub const CUSTOM_SOUND_HINT: &str = "Custom: some values were changed by hand. Choosing a kind of motor sets the pitch, the inertia, the tone and the propeller noise, and keeps the rest.";

/// The sounds that come with the program, one for each kind of motor and propeller. What is chosen is read back from
/// the settings, so it is right after a restart, and it says "Custom" as soon as a value is changed by hand.
fn motor_presets(app: &FlightReplay) -> Element<'_, Message> {
    let in_use = motor_preset_in_use(&app.settings.sound);
    let choices: Vec<MotorChoice> = (0..MOTOR_PRESETS.len()).map(MotorChoice).collect();
    let explanation = match in_use {
        Some(index) => MOTOR_PRESETS[index].description,
        None => CUSTOM_SOUND_HINT,
    };
    column![
        labeled(
            "Motor and propeller",
            pick_list(choices, in_use.map(MotorChoice), |choice| {
                Message::Sound(SoundEdit::MotorPreset(choice.0))
            })
            .placeholder("Custom"),
        ),
        hint(explanation),
    ]
    .spacing(6)
    .into()
}

/// The settings the user saved under a name.
fn saved_presets(app: &FlightReplay) -> Element<'_, Message> {
    let names: Vec<String> = app.settings.presets.keys().cloned().collect();
    let has_saved = !names.is_empty();
    column![
        labeled(
            "Saved presets",
            pick_list(names, app.selected_preset.clone(), |name| Message::Sound(
                SoundEdit::Preset(name)
            ))
            .placeholder(if has_saved {
                "Choose a preset"
            } else {
                "No preset saved yet"
            }),
        ),
        labeled(
            "Name",
            row![
                text_input("A name for these settings", &app.preset_name)
                    .on_input(|t| Message::Sound(SoundEdit::PresetName(t)))
                    .width(Length::Fixed(260.0)),
                action(
                    "Save",
                    (!app.preset_name.trim().is_empty())
                        .then_some(Message::Sound(SoundEdit::SavePreset))
                ),
                action(
                    "Delete",
                    app.selected_preset
                        .is_some()
                        .then_some(Message::Sound(SoundEdit::DeletePreset))
                ),
            ]
            .spacing(8),
        ),
        action(
            "Back to the default settings",
            Some(Message::Sound(SoundEdit::ResetToDefaults))
        ),
    ]
    .spacing(8)
    .into()
}

fn format(app: &FlightReplay) -> Element<'_, Message> {
    let audio = &app.settings.sound.audio;
    column![
        labeled(
            "Sample rate",
            pick_list([44_100u32, 48_000], Some(audio.sample_rate), |r| {
                Message::Sound(SoundEdit::SampleRate(r))
            }),
        ),
        checkbox(audio.channels == 2)
            .label("Stereo")
            .on_toggle(|on| Message::Sound(SoundEdit::Stereo(on))),
    ]
    .spacing(8)
    .into()
}

fn parameters(app: &FlightReplay, group: Group) -> Element<'_, Message> {
    let mut lines = column![].spacing(6);
    for spec in sound_params::SPECS.iter().filter(|s| s.group == group) {
        let param = spec.param;
        lines = lines.push(slider_row(
            spec.label,
            spec.unit,
            sound_params::get(&app.settings.sound, param),
            spec.min..=spec.max,
            spec.step,
            spec.help,
            move |v| Message::Sound(SoundEdit::Param(param, v)),
        ));
    }
    lines.into()
}
