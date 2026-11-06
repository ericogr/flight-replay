// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use flight_replay_core::font::{BUNDLED_FONTS, BundledFont, FontSheet};
use flight_replay_core::osd::Coverage;
use iced::Element;
use iced::widget::{column, pick_list, row, text};

use super::widgets::{action, hint, labeled, page, path_text, problem, section};
use crate::app::FlightReplay;
use crate::message::Message;
use crate::state::{FileKind, FontSource, LoadedFont};
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

/// What the font section says before a font is chosen.
pub const NO_FONT_HINT: &str = "The OSD is drawn with a font. Choose one that comes with the program, or the PNG of your goggles, such as WS_INAV_Europa_24.png from the userfont folder.";

fn font(app: &FlightReplay) -> Element<'_, Message> {
    let busy = app.busy.font;
    let open = app.font.as_ref().map(|font| &font.source);
    let bundled: Vec<&'static BundledFont> = BUNDLED_FONTS.iter().collect();
    let in_use = match open {
        Some(FontSource::Bundled(font)) => Some(*font),
        _ => None,
    };

    let is_file = matches!(open, Some(FontSource::File(_)));
    let built_in = labeled(
        "Built in",
        row![
            pick_list(bundled, in_use, Message::UseBundledFont).placeholder("Choose a font…"),
            action(
                "Remove",
                (in_use.is_some() && !busy).then_some(Message::Clear(FileKind::Font))
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    );
    let from_file = labeled(
        "Or a file",
        row![
            match open {
                Some(FontSource::File(path)) => path_text(path),
                _ => text("None chosen.").into(),
            },
            action(
                if is_file { "Change…" } else { "Choose…" },
                (!busy).then_some(Message::Browse(FileKind::Font))
            ),
            action(
                "Remove",
                (is_file && !busy).then_some(Message::Clear(FileKind::Font))
            ),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
    );

    let mut items = column![built_in, from_file].spacing(8);
    match &app.font {
        Some(font) => {
            items = items.push(hint(font_summary(font)));
            if let FontSource::Bundled(bundled) = &font.source {
                items = items.push(hint(bundled.credit));
                if let Some(osd) = &app.osd
                    && osd.file.firmware != bundled.firmware()
                {
                    items = items.push(problem(format!(
                        "This font is for {}, and the .osd file is from {}: the symbols would not match.",
                        bundled.firmware(),
                        osd.file.firmware
                    )));
                }
            }
        }
        None => {
            items = items.push(hint(NO_FONT_HINT));
        }
    }
    column![items, status(app, FileKind::Font, busy)]
        .spacing(6)
        .into()
}

/// The cells and glyphs of a font, in a line.
fn font_summary(font: &LoadedFont) -> String {
    let layouts: Vec<_> = font.font.sheets().map(FontSheet::layout).collect();
    let sizes: Vec<String> = layouts
        .iter()
        .map(|layout| {
            format!(
                "{} ({}x{})",
                layout.size,
                layout.size.width(),
                layout.size.height()
            )
        })
        .collect();
    let first = layouts.first().copied();
    let colours = if first.is_some_and(|layout| layout.pages == 4) {
        "four-colour"
    } else {
        "standard"
    };
    format!(
        "Cells {}, {colours}, {} glyphs",
        sizes.join(" and "),
        first.map_or(0, |layout| layout.glyph_count())
    )
}
