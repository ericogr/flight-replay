// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use iced::Element;
use iced::widget::{column, row, text};

use super::widgets::{action, clock, hint, page, path_text, problem, section};
use crate::app::FlightReplay;
use crate::message::Message;
use crate::state::FileKind;

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
                    clock(i.duration_s),
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
    let body: Element<'_, Message> = match &app.osd {
        Some(osd) => {
            let f = &osd.file;
            column![
                row![
                    path_text(&osd.path),
                    buttons(FileKind::Osd, true, app.busy.osd)
                ]
                .spacing(12),
                hint(format!(
                    "{}, {}x{} grid, {} screens, {}",
                    f.firmware,
                    f.columns,
                    f.rows,
                    f.frames.len(),
                    clock(f.duration_s())
                )),
                hint(osd.profile.explanation()),
            ]
            .spacing(6)
            .into()
        }
        None => row![
            text("None found next to the video."),
            buttons(FileKind::Osd, false, app.busy.osd)
        ]
        .spacing(12)
        .into(),
    };
    column![body, status(app, FileKind::Osd, app.busy.osd)]
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
                clock(srt.file.duration_s())
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
