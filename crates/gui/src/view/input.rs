// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use flight_replay_core::osd::Coverage;
use iced::Element;
use iced::widget::{column, row, text};

use super::widgets::{action, hint, page, path_text, problem, section};
use crate::app::FlightReplay;
use crate::message::Message;
use crate::state::FileKind;
use flight_replay_core::display::clock_tenths;

pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    page(vec![
        hint("Drop the files on the window, or use the buttons. The .osd and .srt next to the video are found by themselves."),
        section("Video", video(app)),
        section("OSD recording (.osd)", osd(app)),
        section("SRT data (.srt)", srt(app)),
        section("OSD font", font(app)),
    ])
    .into()
}

/// The buttons of a file row.
fn buttons<'a>(kind: FileKind, open: bool, busy: bool) -> Element<'a, Message> {
    let mut line = row![action(
        if open { "Change…" } else { "Choose…" },
        (!busy).then_some(Message::Browse(kind))
    )]
    .spacing(8);
    if open && kind != FileKind::Video {
        line = line.push(action("Remove", Some(Message::Clear(kind))));
    }
    line.into()
}

fn status(app: &FlightReplay, kind: FileKind, busy: bool) -> Element<'_, Message> {
    if busy {
        return hint("Reading…");
    }
    match app.file_error(kind) {
        Some(error) => problem(error.to_owned()),
        None => hint(""),
    }
}

fn video(app: &FlightReplay) -> Element<'_, Message> {
    let info = match &app.video {
        Some(video) => {
            let i = &video.info;
            let sound = if i.audio_streams > 0 {
                "has sound"
            } else {
                "no sound"
            };
            column![
                row![
                    path_text(&video.path),
                    buttons(FileKind::Video, true, app.busy.video)
                ]
                .spacing(12),
                hint(format!(
                    "{}x{}, {}, {:.2} fps (declared {:.0}), {}, {} frames, {sound}",
                    i.width,
                    i.height,
                    i.codec,
                    i.frame_rate,
                    i.nominal_frame_rate,
                    clock_tenths(i.duration_s),
                    i.frame_count
                )),
            ]
            .spacing(6)
        }
        None => column![
            row![
                text("No video yet."),
                buttons(FileKind::Video, false, app.busy.video)
            ]
            .spacing(12)
        ],
    };
    column![info, status(app, FileKind::Video, app.busy.video)]
        .spacing(6)
        .into()
}

fn osd(app: &FlightReplay) -> Element<'_, Message> {
    let busy = app.busy.osd;
    let body: Element<'_, Message> = match &app.osd {
        Some(osd) => {
            let f = &osd.file;
            let mut items = column![].spacing(6);
            if osd.paths.len() > 1 {
                for (index, (path, part)) in osd.paths.iter().zip(&osd.parts).enumerate() {
                    items = items.push(
                        row![
                            text(format!("{}.", index + 1)),
                            path_text(path),
                            hint(format!(
                                "{} to {}",
                                clock_tenths(part.start_s),
                                clock_tenths(part.end_s)
                            )),
                            action(
                                "Move up",
                                (index > 0 && !busy).then_some(Message::MoveOsdFileUp(index))
                            ),
                            action("Remove", (!busy).then_some(Message::RemoveOsdFile(index))),
                        ]
                        .spacing(12)
                        .align_y(iced::Alignment::Center),
                    );
                }
            } else {
                items = items.push(
                    row![path_text(osd.path()), buttons(FileKind::Osd, true, busy)].spacing(12),
                );
            }
            items = items.push(action(
                "Add the .osd of another recording…",
                (!busy).then_some(Message::AddOsd),
            ));
            items = items.push(hint(format!(
                "{}, {}x{} grid, {} screens, {}",
                f.firmware,
                f.columns,
                f.rows,
                f.frames.len(),
                clock_tenths(f.duration_s())
            )));
            items = items.push(hint(osd.profile.explanation()));
            if osd.paths.len() > 1 {
                items = items.push(hint(
                    "The video is made of several recordings: the files are played in this order, one after the other.",
                ));
                if let Some(video) = &app.video
                    && let Some(warning) =
                        Coverage::new(&osd.parts, video.info.duration_s).warning()
                {
                    items = items.push(problem(warning));
                }
            }
            items.into()
        }
        None => row![
            text("None found next to the video."),
            buttons(FileKind::Osd, false, busy)
        ]
        .spacing(12)
        .into(),
    };
    column![body, status(app, FileKind::Osd, busy)]
        .spacing(6)
        .into()
}

fn srt(app: &FlightReplay) -> Element<'_, Message> {
    let body: Element<'_, Message> = match &app.srt {
        Some(srt) => column![
            row![
                path_text(&srt.path),
                buttons(FileKind::Srt, true, app.busy.srt)
            ]
            .spacing(12),
            hint(format!(
                "{} subtitles, {}",
                srt.file.cues.len(),
                clock_tenths(srt.file.duration_s())
            )),
        ]
        .spacing(6)
        .into(),
        None => row![
            text("None found next to the video."),
            buttons(FileKind::Srt, false, app.busy.srt)
        ]
        .spacing(12)
        .into(),
    };
    column![body, status(app, FileKind::Srt, app.busy.srt)]
        .spacing(6)
        .into()
}

fn font(app: &FlightReplay) -> Element<'_, Message> {
    let body: Element<'_, Message> = match &app.font {
        Some(font) => {
            let layout = font.sheet.layout();
            let colours = if layout.pages == 4 { "four-colour" } else { "standard" };
            column![
                row![path_text(&font.path), buttons(FileKind::Font, true, app.busy.font)].spacing(12),
                hint(format!(
                    "{} cells ({}), {colours}, {} glyphs",
                    layout.size,
                    format_args!("{}x{}", layout.size.width(), layout.size.height()),
                    layout.glyph_count()
                )),
            ]
            .spacing(6)
            .into()
        }
        None => column![
            row![text("No font yet."), buttons(FileKind::Font, false, app.busy.font)].spacing(12),
            hint("The font is the PNG of your goggles, such as WS_INAV_Europa_24.png from the userfont folder. It is needed to draw the OSD."),
        ]
        .spacing(6)
        .into(),
    };
    column![body, status(app, FileKind::Font, app.busy.font)]
        .spacing(6)
        .into()
}
