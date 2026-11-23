// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use flight_replay_core::display::clock_tenths;
use flight_replay_core::ffmpeg::VideoInfo;
use flight_replay_core::font::{BUNDLED_FONTS, BundledFont, FontSheet};
use flight_replay_core::footage::Footage;
use flight_replay_core::osd::{Coverage, OsdFile};
use flight_replay_core::paths::file_name;
use flight_replay_core::srt::SrtFile;
use iced::widget::{Column, column, pick_list, row, text};
use iced::{Alignment, Element};

use super::widgets::{action, hint, labeled, page, path_text, problem, section};
use crate::app::FlightReplay;
use crate::message::Message;
use crate::state::{FileKind, FontSource, LoadedFont, LoadedOsd};

pub fn view(app: &FlightReplay) -> Element<'_, Message> {
    page(vec![
        hint("Drop the files on the window, or use the buttons. The .osd and .srt next to each video are found by themselves. When the goggles recorded a flight in several files, choose or drop all its videos together: they are joined into one video."),
        section(if app.clips.len() > 1 { "Videos" } else { "Video" }, video(app)),
        section("OSD recording (.osd)", osd(app)),
        section("SRT data (.srt)", srt(app)),
        section("OSD font", font(app)),
    ])
    .into()
}

/// Whether the buttons that open a dialog or start a read can be pressed: the file is being read, or a dialog is open,
/// and a second click would only get in the way of the first.
fn free(app: &FlightReplay, kind: FileKind) -> bool {
    !app.busy.of(kind) && !app.dialog.is_open()
}

/// The buttons of a file row.
fn buttons<'a>(app: &FlightReplay, kind: FileKind, open: bool) -> Element<'a, Message> {
    let mut line = row![action(
        if open { "Change…" } else { "Choose…" },
        free(app, kind).then_some(Message::Browse(kind))
    )]
    .spacing(8);
    if open && kind != FileKind::Video {
        line = line.push(action("Remove", Some(Message::Clear(kind))));
    }
    line.into()
}

fn status(app: &FlightReplay, kind: FileKind) -> Element<'_, Message> {
    if app.busy.of(kind) {
        return hint("Reading…");
    }
    match app.file_error(kind) {
        Some(error) => problem(error.to_owned()),
        None => hint(""),
    }
}

/// What a video is, in a line.
fn video_summary(i: &VideoInfo) -> String {
    let sound = if i.audio_streams > 0 {
        "has sound"
    } else {
        "no sound"
    };
    format!(
        "{}x{}, {}, {:.2} fps (declared {:.0}), {}, {} frames, {sound}",
        i.width,
        i.height,
        i.codec,
        i.frame_rate,
        i.nominal_frame_rate,
        clock_tenths(i.duration_s),
        i.frame_count
    )
}

fn video(app: &FlightReplay) -> Element<'_, Message> {
    let add = action(
        "Add the next video…",
        free(app, FileKind::Video).then_some(Message::AddVideos),
    );
    let body = match app.footage.as_deref() {
        None => {
            column![row![text("No video yet."), buttons(app, FileKind::Video, false)].spacing(12)]
        }
        Some(footage) => match footage.parts() {
            [video] => column![
                row![path_text(&video.path), buttons(app, FileKind::Video, true)].spacing(12),
                hint(video_summary(&video.info)),
                add,
            ]
            .spacing(6),
            _ => several_videos(app, footage, add),
        },
    };
    column![body, status(app, FileKind::Video)]
        .spacing(6)
        .into()
}

/// The videos of a flight of several, in the order they play.
fn several_videos<'a>(
    app: &FlightReplay,
    footage: &'a Footage,
    add: Element<'a, Message>,
) -> Column<'a, Message> {
    let free = free(app, FileKind::Video);
    let mut items = column![hint(format!(
        "The flight is {} videos, one after the other: {} in all. The first one names the video that is written.",
        footage.parts().len(),
        clock_tenths(footage.info().duration_s)
    ))]
    .spacing(6);
    for (index, video) in footage.parts().iter().enumerate() {
        items = items.push(
            row![
                text(format!("{}.", index + 1)),
                path_text(&video.path),
                action(
                    "Move up",
                    (index > 0 && free).then_some(Message::MoveVideoUp(index))
                ),
                action("Remove", free.then_some(Message::RemoveVideo(index))),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
        items = items.push(hint(video_summary(&video.info)));
    }
    items = items.push(
        row![
            add,
            action(
                "Choose other videos…",
                free.then_some(Message::Browse(FileKind::Video))
            )
        ]
        .spacing(8),
    );
    for difference in footage.differences() {
        items = items.push(hint(format!(
            "{difference}. Such videos are joined only when the OSD or the SRT data are drawn, which encodes the video again."
        )));
    }
    items
}

/// What a missing file leaves out of the render, as the settings ask for it: `None` when nothing that needs it is on.
fn left_out(app: &FlightReplay, kind: FileKind) -> Option<String> {
    let settings = &app.settings;
    let wanted: &[(bool, &str)] = match kind {
        FileKind::Osd => &[
            (settings.draw_osd, "the OSD"),
            (settings.make_sound, "the motor sound"),
        ],
        FileKind::Srt => &[(settings.draw_srt, "the SRT data")],
        FileKind::Video | FileKind::Font => &[],
    };
    let names: Vec<&str> = wanted
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, name)| *name)
        .collect();
    (!names.is_empty()).then(|| names.join(" and "))
}

/// The warning for the videos that lack an `.osd` or an `.srt` file, when what the user asked for needs it.
fn missing_warning(app: &FlightReplay, kind: FileKind) -> Option<Element<'_, Message>> {
    let names = app.videos_without(kind);
    if names.is_empty() {
        return None;
    }
    let what = left_out(app, kind)?;
    let file = if kind == FileKind::Osd {
        ".osd"
    } else {
        ".srt"
    };
    Some(problem(format!(
        "There is no {file} file for {}: {what} will be left out of the render. Choose the file here, or go on without it.",
        names.join(", ")
    )))
}

/// The line of one video of a flight of several, for its `.osd` or its `.srt`: the file and what to do with it, or
/// that there is none. `detail` says something about the file when there is one.
fn clip_file_row(
    app: &FlightReplay,
    kind: FileKind,
    index: usize,
    detail: Option<String>,
) -> Element<'_, Message> {
    let clip = &app.clips[index];
    let path = match kind {
        FileKind::Osd => clip.osd.first(),
        FileKind::Srt => clip.srt.as_ref(),
        FileKind::Video | FileKind::Font => None,
    };
    let free = free(app, kind);
    let number = text(format!("{}.", index + 1));
    match path {
        Some(path) => row![
            number,
            path_text(path),
            hint(detail.unwrap_or_default()),
            action(
                "Change…",
                free.then_some(Message::BrowseClipFile(kind, index))
            ),
            action(
                "Remove",
                free.then_some(Message::ClearClipFile(kind, index))
            ),
        ],
        None => row![
            number,
            text(format!(
                "None found next to {}.",
                file_name(&clip.video.path)
            )),
            action(
                "Choose…",
                free.then_some(Message::BrowseClipFile(kind, index))
            ),
        ],
    }
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn osd(app: &FlightReplay) -> Element<'_, Message> {
    let several = app.clips.len() > 1;
    let mut items = column![].spacing(6);
    if several {
        items = items.push(osd_of_each_video(app));
    }
    match &app.osd {
        Some(osd) => {
            if !several {
                items = items.push(recordings_of_one_video(app, osd));
            }
            items = items.push(hint(osd_summary(&osd.file)));
            items = items.push(hint(osd.profile.explanation()));
            if !several && osd.paths.len() > 1 {
                items = items.push(hint(
                    "The video is made of several recordings: the files are played in this order, one after the other.",
                ));
            }
            for warning in coverage_warnings(app, osd) {
                items = items.push(problem(warning));
            }
        }
        None if !several => {
            items = items.push(
                row![
                    text("None found next to the video."),
                    buttons(app, FileKind::Osd, false)
                ]
                .spacing(12),
            );
        }
        None => {}
    }
    if let Some(warning) = missing_warning(app, FileKind::Osd) {
        items = items.push(warning);
    }
    column![items, status(app, FileKind::Osd)].spacing(6).into()
}

/// The `.osd` file of each video of a flight of several, with where its recording sits once they are read.
fn osd_of_each_video(app: &FlightReplay) -> Column<'_, Message> {
    let parts = app
        .osd
        .as_ref()
        .map(|osd| osd.parts.as_slice())
        .filter(|parts| parts.len() == app.clips.len());
    let mut rows = column![].spacing(6);
    for index in 0..app.clips.len() {
        let span = parts.map(|parts| {
            format!(
                "{} to {}",
                clock_tenths(parts[index].start_s),
                clock_tenths(parts[index].end_s)
            )
        });
        rows = rows.push(clip_file_row(app, FileKind::Osd, index, span));
    }
    rows
}

/// The `.osd` files of a flight of one video: one, or several when it was recorded in several files, and the button
/// to add another.
fn recordings_of_one_video<'a>(app: &FlightReplay, osd: &'a LoadedOsd) -> Column<'a, Message> {
    let busy = app.busy.osd;
    let mut rows = column![].spacing(6);
    if osd.paths.len() > 1 {
        for (index, (path, part)) in osd.paths.iter().zip(&osd.parts).enumerate() {
            rows = rows.push(
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
                .align_y(Alignment::Center),
            );
        }
    } else {
        rows =
            rows.push(row![path_text(osd.path()), buttons(app, FileKind::Osd, true)].spacing(12));
    }
    rows.push(action(
        "Add the .osd of another recording…",
        (!busy && !app.dialog.is_open()).then_some(Message::AddOsd),
    ))
}

/// What the screens of an `.osd` file are, in a line.
fn osd_summary(file: &OsdFile) -> String {
    format!(
        "{}, {}x{} grid, {} screens, {}",
        file.firmware,
        file.columns,
        file.rows,
        file.frames.len(),
        clock_tenths(file.duration_s())
    )
}

/// What is worth telling the user about how the `.osd` files cover the videos: each video is compared with its own
/// recording, and a video made of several recordings with all of them.
fn coverage_warnings(app: &FlightReplay, osd: &LoadedOsd) -> Vec<String> {
    if app.clips.len() > 1 {
        if osd.parts.len() != app.clips.len() {
            return Vec::new();
        }
        return app
            .clips
            .iter()
            .zip(&osd.parts)
            .filter_map(|(clip, part)| {
                Coverage {
                    video_s: clip.video.info.duration_s,
                    osd_s: part.end_s - part.start_s,
                }
                .warning()
                .map(|warning| format!("{}: {warning}", file_name(&clip.video.path)))
            })
            .collect();
    }
    if osd.paths.len() > 1
        && let Some(footage) = &app.footage
        && let Some(warning) = Coverage::new(&osd.parts, footage.info().duration_s).warning()
    {
        return vec![warning];
    }
    Vec::new()
}

fn srt(app: &FlightReplay) -> Element<'_, Message> {
    let mut items = column![].spacing(6);
    if app.clips.len() > 1 {
        for index in 0..app.clips.len() {
            items = items.push(clip_file_row(app, FileKind::Srt, index, None));
        }
    } else if let Some(srt) = &app.srt {
        items = items
            .push(row![path_text(&srt.paths[0]), buttons(app, FileKind::Srt, true)].spacing(12));
    } else {
        items = items.push(
            row![
                text("None found next to the video."),
                buttons(app, FileKind::Srt, false)
            ]
            .spacing(12),
        );
    }
    if let Some(srt) = &app.srt {
        items = items.push(hint(srt_summary(&srt.file)));
    }
    if let Some(warning) = missing_warning(app, FileKind::Srt) {
        items = items.push(warning);
    }
    column![items, status(app, FileKind::Srt)].spacing(6).into()
}

/// What the subtitles of an `.srt` file are, in a line.
fn srt_summary(file: &SrtFile) -> String {
    format!(
        "{} subtitles, {}",
        file.cues.len(),
        clock_tenths(file.duration_s())
    )
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
        pick_list(bundled, in_use, Message::UseBundledFont).placeholder("Choose a font…"),
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
                (!busy && !app.dialog.is_open()).then_some(Message::Browse(FileKind::Font))
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
    column![items, status(app, FileKind::Font)]
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
