// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::time::Duration;

use flight_replay_core::render::{Mode, Outcome};
use iced::widget::{column, progress_bar, row, text};
use iced::{Element, Font};

use super::widgets::{action, good, hint, page, problem, section};
use crate::app::FlightReplay;
use crate::message::Message;
use crate::state::{QueueStatus, RenderState, Running};
use crate::wizard::{self, Step};
use flight_replay_core::display::clock_tenths;
use flight_replay_core::paths::file_name;

pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    let mut items: Vec<Element<'_, Message>> = vec![section("Summary", summary(app))];
    match &app.render {
        RenderState::Idle => {
            items.push(section("Command", commands(app)));
            items.push(start(app));
        }
        RenderState::Running(running) => items.push(section("Rendering", running_view(running))),
        RenderState::Finished(result) => {
            items.push(match result {
                Ok(outcome) => section("Done", finished(outcome)),
                Err(error) => section(
                    "The render did not finish",
                    column![
                        problem(error.clone()),
                        action("Try again", Some(Message::StartRender))
                    ]
                    .spacing(8),
                ),
            });
        }
    }
    // With videos in the queue, the queue is what the user is working with, so it comes first.
    if app.queue.items.is_empty() {
        items.push(section("Queue", queue(app)));
    } else {
        items.insert(0, section("Queue", queue(app)));
    }
    page(items).into()
}

/// Several videos, rendered one after the other with the settings of the wizard.
fn queue(app: &FlightReplay) -> Element<'_, Message> {
    let queue = &app.queue;
    let rendering = app.is_rendering();
    let mut items = column![hint(
        "Render several videos one after the other with these settings. Add this video, or every video of a folder with the files found next to each. The speed of the OSD is chosen for each video."
    )]
    .spacing(8);
    items = items.push(
        row![
            action(
                "Add this video to the queue",
                (!rendering && app.build_plan().is_ok()).then_some(Message::QueueAdd)
            ),
            action(
                "Add the videos of a folder…",
                (!rendering && !queue.preparing && app.tools().is_some())
                    .then_some(Message::QueueAddFolder)
            ),
        ]
        .spacing(8),
    );
    if queue.preparing {
        items = items.push(hint("Preparing the videos…"));
    }
    for (index, item) in queue.items.iter().enumerate() {
        let status: Element<'_, Message> = match &item.status {
            QueueStatus::Waiting => hint("waiting"),
            QueueStatus::Running => text("rendering…").into(),
            QueueStatus::Done(what) => good(what.clone()),
            QueueStatus::Failed(why) => problem(why.clone()),
            QueueStatus::Cancelled => hint("stopped"),
        };
        items = items.push(
            row![
                text(format!("{}. {}", index + 1, item.title)),
                status,
                action(
                    "Remove",
                    (item.status != QueueStatus::Running).then_some(Message::QueueRemove(index))
                ),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        );
    }
    if !queue.items.is_empty() {
        let finished = queue
            .items
            .iter()
            .any(|i| !matches!(i.status, QueueStatus::Waiting | QueueStatus::Running));
        items = items.push(
            row![
                action(
                    "Start the queue",
                    (queue.waiting() > 0 && !rendering).then_some(Message::QueueStart)
                ),
                action(
                    "Clear the finished",
                    finished.then_some(Message::QueueClearFinished)
                ),
            ]
            .spacing(8),
        );
    }
    items.into()
}

fn summary(app: &FlightReplay) -> Element<'_, Message> {
    let mut lines = column![].spacing(4);
    if let Some(video) = &app.video {
        lines = lines.push(text(format!("Video: {}", file_name(&video.path))));
    }
    let mut added = Vec::new();
    if app.settings.draw_osd {
        added.push("the OSD");
    }
    if app.settings.draw_srt {
        added.push("the SRT data");
    }
    if app.settings.make_sound {
        added.push("the motor sound");
    }
    lines = lines.push(text(if added.is_empty() {
        "Nothing is added yet.".to_owned()
    } else {
        format!("Adds: {}", added.join(", "))
    }));
    let draws = app.settings.draw_osd || app.settings.draw_srt;
    lines = lines.push(text(if draws {
        format!(
            "Video: encoded again with {} at {} Mbps{}",
            app.settings.output.encoder,
            app.settings.output.bitrate_mbps,
            if app.settings.output.upscale_1440p {
                ", scaled to 1440p"
            } else {
                ""
            }
        )
    } else {
        "Video: copied as it is".to_owned()
    }));
    lines = lines.push(text(format!("Output: {}", app.output_path)));
    if app.stretch.enabled {
        lines = lines.push(text(format!(
            "Only a stretch: from {} s, for {} s",
            app.stretch.start.trim(),
            app.stretch.duration.trim()
        )));
    }
    lines.into()
}

fn commands(app: &FlightReplay) -> Element<'_, Message> {
    if app.command_lines.is_empty() {
        return hint("The command is shown here when everything is set.");
    }
    let mut lines = column![hint(
        "FFmpeg runs these commands. The sound is made by Flight Replay itself."
    )]
    .spacing(6);
    for line in &app.command_lines {
        lines = lines.push(text(line.clone()).font(Font::MONOSPACE).size(12));
    }
    lines
        .push(action("Copy", Some(Message::CopyCommand)))
        .into()
}

fn start(app: &FlightReplay) -> Element<'_, Message> {
    let mut problems = wizard::problems(Step::Render, &app.context());
    if problems.is_empty()
        && let Err(more) = app.build_plan()
    {
        problems = more;
    }
    let mut items = column![].spacing(8);
    for problem_text in &problems {
        items = items.push(problem(problem_text.clone()));
    }
    items
        .push(action(
            "Start the render",
            problems.is_empty().then_some(Message::StartRender),
        ))
        .into()
}

fn fraction((done, total): (u64, u64)) -> f32 {
    if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    }
}

fn running_view(running: &Running) -> Element<'_, Message> {
    let elapsed = running.started.elapsed();
    let overall = if running.video.1 > 0 {
        fraction(running.video)
    } else {
        fraction(running.audio)
    };
    let eta = if overall > 0.02 {
        let total = elapsed.as_secs_f32() / overall;
        format!(
            "about {} left",
            clock_tenths(f64::from((total - elapsed.as_secs_f32()).max(0.0)))
        )
    } else {
        "working out the time left…".to_owned()
    };
    let mut items = column![text(if running.cancelling {
        "Stopping…"
    } else {
        running.stage.label()
    })]
    .spacing(8);
    if running.audio.1 > 0 {
        items = items.push(
            column![
                text("Sound").size(14),
                progress_bar(0.0..=1.0, fraction(running.audio))
            ]
            .spacing(2),
        );
    }
    if running.video.1 > 0 {
        items = items.push(
            column![
                text(format!(
                    "Video: frame {} of {}",
                    running.video.0, running.video.1
                ))
                .size(14),
                progress_bar(0.0..=1.0, fraction(running.video)),
            ]
            .spacing(2),
        );
    }
    items = items.push(hint(format!(
        "{} elapsed, {eta}",
        clock_tenths(elapsed.as_secs_f64())
    )));
    for warning in &running.warnings {
        items = items.push(hint(format!("• {warning}")));
    }
    items
        .push(action(
            "Cancel",
            (!running.cancelling).then_some(Message::CancelRender),
        ))
        .into()
}

fn finished(outcome: &Outcome) -> Element<'_, Message> {
    let mut items = column![good(format!("Saved {}", outcome.output.display()))].spacing(6);
    items = items.push(hint(format!(
        "{} s of video in {} ({})",
        format_args!("{:.1}", outcome.window.duration_s),
        clock_tenths(Duration::as_secs_f64(&outcome.elapsed)),
        match outcome.mode {
            Mode::SoundOnly => "video copied",
            Mode::Overlay => "video encoded again",
        }
    )));
    if let Some(check) = &outcome.check {
        let audio = check
            .audio_duration_s
            .map_or_else(|| "no sound".to_owned(), |a| format!("sound {a:.3} s"));
        items = items.push(hint(format!(
            "Checked: {} frames, picture {:.3} s, {audio}",
            check.frames, check.video_duration_s
        )));
    }
    if let Some(stats) = &outcome.audio_stats {
        items = items.push(hint(format!(
            "Sound: peak {:.1} dBFS, level {:.1} dBFS",
            20.0 * stats.peak.max(1e-9).log10(),
            20.0 * stats.rms.max(1e-9).log10()
        )));
    }
    for warning in &outcome.warnings {
        items = items.push(hint(format!("• {warning}")));
    }
    items
        .push(
            row![
                action("Open the folder", Some(Message::OpenOutputFolder)),
                action("Render again", Some(Message::StartRender))
            ]
            .spacing(8),
        )
        .into()
}
