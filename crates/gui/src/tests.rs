// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Headless tests of the wizard.
//!
//! They run the real `update` and `view` of the application: the messages that the views produce are found with
//! iced's simulator (no window and no desktop are needed), and the tasks that `update` returns are run to the end.
//! The tests that need FFmpeg skip themselves, with a message, when it is not installed.
//!
//! `FLIGHT_REPLAY_SNAPSHOTS=<folder>` saves a picture of every step the tests go through, to look at the layout.

use std::path::{Path, PathBuf};

use flight_replay_core::ffmpeg::{DecodeOptions, FrameReader, Tools, probe};
use flight_replay_core::osd::Coverage;
use iced::Task;
use iced::futures::StreamExt;
use iced_runtime::Action;
use image::{Rgba, RgbaImage};
use tempfile::TempDir;

use crate::app::{FlightReplay, ToolsState};
use crate::message::{Message, OutputEdit};
use crate::preferences::ThemeChoice;
use crate::state::{FileKind, QueueStatus, RenderState};
use crate::tasks;
use crate::view;
use crate::wizard::Step;

/// The application, driven the way the user would.
struct Harness {
    app: FlightReplay,
    dir: TempDir,
}

impl Harness {
    /// A new application with its settings in a folder of its own, after the search for FFmpeg. `None` when FFmpeg
    /// is not installed.
    fn start() -> Option<Self> {
        if let Err(error) = Tools::locate(None) {
            eprintln!("skipping the GUI test: {error}");
            return None;
        }
        let dir = TempDir::new().expect("a temporary folder");
        let app = FlightReplay::new(Some(dir.path().join("settings.toml")));
        let mut harness = Self { app, dir };
        harness.run(tasks::locate_tools(None));
        assert!(
            matches!(harness.app.tools, ToolsState::Found(_)),
            "the tools are found"
        );
        Some(harness)
    }

    /// Runs a task to its end, feeding what it produces back into the application.
    fn run(&mut self, task: Task<Message>) {
        let mut pending = vec![task];
        while let Some(task) = pending.pop() {
            let Some(mut stream) = iced_runtime::task::into_stream(task) else {
                continue;
            };
            while let Some(action) = iced::futures::executor::block_on(stream.next()) {
                if let Action::Output(message) = action {
                    pending.push(self.app.update(message));
                }
            }
        }
    }

    /// Sends a message and runs what it starts.
    fn send(&mut self, message: Message) {
        let task = self.app.update(message);
        self.run(task);
    }

    /// The window as a simulator sees it. It is taller than the real one, so that a step that scrolls is all in view:
    /// the simulator finds only what is in view.
    fn tall_window(&self) -> iced_test::Simulator<'_, Message> {
        iced_test::Simulator::with_size(
            iced::Settings::default(),
            iced::Size::new(1100.0, 3000.0),
            view::view(&self.app),
        )
    }

    /// The messages that clicking the widget with this text produces. Empty for a button that is switched off.
    fn clicks(&self, label: &str) -> Vec<Message> {
        let mut ui = self.tall_window();
        ui.click(label)
            .unwrap_or_else(|_| panic!("a widget with the text {label:?} on {:?}", self.app.step));
        ui.into_messages().collect()
    }

    /// Clicks the widget with this text, and runs what the click starts.
    fn click(&mut self, label: &str) {
        for message in self.clicks(label) {
            self.send(message);
        }
    }

    /// Whether some widget shows this text.
    fn shows(&self, text: &str) -> bool {
        self.tall_window().find(text).is_ok()
    }

    /// Saves a picture of the window when `FLIGHT_REPLAY_SNAPSHOTS` names a folder.
    fn snapshot(&self, name: &str) {
        let Some(folder) = std::env::var_os("FLIGHT_REPLAY_SNAPSHOTS") else {
            return;
        };
        let folder = PathBuf::from(folder);
        std::fs::create_dir_all(&folder).expect("the snapshot folder");
        for entry in std::fs::read_dir(&folder).expect("the snapshot folder") {
            let path = entry.expect("an entry").path();
            if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(&format!("{name}-")))
            {
                std::fs::remove_file(path).expect("the old snapshot");
            }
        }
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            iced::Size::new(1100.0, 820.0),
            view::view(&self.app),
        );
        let snapshot = ui.snapshot(&self.app.theme()).expect("a snapshot");
        snapshot
            .matches_image(folder.join(format!("{name}.png")))
            .expect("the snapshot is saved");
    }

    /// A path inside the temporary folder.
    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}

/// A file of the fixtures folder of the repository.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

/// A font sheet for the tests: 512 glyphs of 24x36 pixels, each one different.
fn write_font(path: &Path) {
    let (width, height) = (24, 36);
    let mut sheet = RgbaImage::new(width, height * 512);
    for code in 0..512u32 {
        for y in 0..height {
            for x in 0..width {
                let border = x < 2 || y < 2 || x + 2 >= width || y + 2 >= height;
                let pixel = if border {
                    Rgba([255, 255, 255, 255])
                } else {
                    let bit = (code >> ((y / 4) % 9)) & 1;
                    Rgba([255, (bit * 255) as u8, (code % 256) as u8, 210])
                };
                sheet.put_pixel(x, code * height + y, pixel);
            }
        }
    }
    sheet.save(path).expect("the font file");
}

#[test]
fn the_wizard_cannot_leave_the_input_step_without_a_video() {
    let Some(mut app) = Harness::start() else {
        return;
    };
    app.click("Next");
    assert_eq!(app.app.step, Step::Input);
    app.snapshot("input-empty");

    assert!(app.shows("Open a video."), "the reason is on the screen");
    assert!(
        app.clicks("Next").is_empty(),
        "the Next button is switched off"
    );
    assert!(
        app.clicks("5  Motor sound").is_empty(),
        "so are the steps beyond"
    );
}

#[test]
fn the_colour_scheme_is_saved_with_the_settings() {
    let Some(mut app) = Harness::start() else {
        return;
    };
    app.click("Light");
    assert_eq!(app.app.preferences.theme, ThemeChoice::Light);
    app.snapshot("welcome-light");

    let saved = std::fs::read_to_string(app.path("settings.toml")).expect("the settings are saved");
    assert!(saved.contains("light"), "{saved}");
}

#[test]
fn dropping_a_video_loads_the_files_that_go_with_it() {
    let Some(mut app) = Harness::start() else {
        return;
    };
    app.send(Message::FileDropped(fixture("clip-3s.mp4")));
    assert!(app.app.video.is_some(), "the video is loaded");
    assert!(app.app.osd.is_some(), "the .osd next to it is loaded");
    assert!(app.app.srt.is_some(), "the .srt next to it is loaded");
    assert!(app.app.file_errors.is_empty(), "{:?}", app.app.file_errors);

    app.send(Message::FileDropped(app.path("notes.txt")));
    assert!(
        app.app.notices.iter().any(|n| n.contains("notes.txt")),
        "a file of another kind is refused with a notice"
    );

    app.send(Message::Clear(FileKind::Video));
    assert!(app.app.video.is_none() && app.app.osd.is_none());
}

#[test]
fn every_step_is_walked_and_the_video_is_rendered() {
    let Some(mut app) = Harness::start() else {
        return;
    };
    let font = app.path("font.png");
    write_font(&font);

    app.snapshot("1-welcome");
    app.click("Next");
    assert_eq!(app.app.step, Step::Input);

    app.send(Message::FileDropped(fixture("clip-3s.mp4")));
    app.send(Message::FileDropped(font));
    assert!(app.app.font.is_some(), "the font is loaded");
    assert!(app.app.telemetry.is_some(), "the telemetry is decoded");
    app.snapshot("2-input");
    app.click("Next");
    assert_eq!(app.app.step, Step::Overlay);
    app.snapshot("3-osd");
    app.click("Next");
    assert_eq!(app.app.step, Step::Srt);
    app.snapshot("4-srt");
    app.click("Draw the SRT data on the video");
    assert!(app.app.settings.draw_srt, "the SRT data is switched on");
    app.snapshot("4-srt-on");
    app.click("Next");
    assert_eq!(app.app.step, Step::Sound);
    app.snapshot("5-sound");
    app.click("Next");
    assert_eq!(app.app.step, Step::Output);

    let output = app.path("out.mp4");
    app.send(Message::Output(OutputEdit::Path(
        output.display().to_string(),
    )));
    app.snapshot("6-output");
    app.click("Next");
    assert_eq!(app.app.step, Step::Render);
    app.snapshot("7-render");

    app.click("Start the render");
    let RenderState::Finished(result) = &app.app.render else {
        panic!("the render is over");
    };
    let outcome = result.as_ref().expect("the render succeeded");
    assert_eq!(outcome.output, output);
    app.snapshot("8-done");

    let tools = Tools::locate(None).expect("the tools");
    let info = probe(&tools, &output).expect("the output is a video");
    let source = probe(&tools, &fixture("clip-3s.mp4")).expect("the clip");
    assert_eq!(
        (info.width, info.height),
        (source.width, source.height),
        "the size is kept"
    );
    assert!(info.audio_streams > 0, "the sound is in the output");

    // The picture has something drawn on it.
    let mut reader = FrameReader::open(
        &tools,
        &output,
        &DecodeOptions {
            start_s: 1.0,
            duration_s: Some(0.1),
            ..DecodeOptions::whole(info.width, info.height)
        },
    )
    .expect("the frames of the output");
    let mut frame = Vec::new();
    assert!(reader.read_frame(&mut frame).expect("a frame"));
    reader.cancel();
    let original = {
        let mut reader = FrameReader::open(
            &tools,
            &fixture("clip-3s.mp4"),
            &DecodeOptions {
                start_s: 1.0,
                duration_s: Some(0.1),
                ..DecodeOptions::whole(info.width, info.height)
            },
        )
        .expect("the frames of the clip");
        let mut original = Vec::new();
        assert!(reader.read_frame(&mut original).expect("a frame"));
        reader.cancel();
        original
    };
    let changed = frame
        .as_chunks::<4>()
        .0
        .iter()
        .zip(original.as_chunks::<4>().0)
        .filter(|(a, b)| a[..3].iter().zip(&b[..3]).any(|(x, y)| x.abs_diff(*y) > 40))
        .count();
    assert!(changed > 2_000, "the OSD is drawn: {changed} pixels differ");
}

/// The clip played twice, as one video of about six seconds, in the folder of the harness.
fn joined_video(harness: &Harness) -> PathBuf {
    let tools = Tools::locate(None).expect("the tools");
    let clip = std::fs::canonicalize(fixture("clip-3s.mp4")).expect("the clip");
    let list = harness.path("list.txt");
    std::fs::write(&list, format!("file '{0}'\nfile '{0}'\n", clip.display())).expect("the list");
    let output = harness.path("joined.mp4");
    let status = std::process::Command::new(tools.ffmpeg())
        .args(["-v", "error", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list)
        .args(["-c", "copy"])
        .arg(&output)
        .status()
        .expect("FFmpeg runs");
    assert!(status.success(), "FFmpeg joined the clip");
    output
}

#[test]
fn a_video_made_of_two_recordings_takes_two_osd_files() {
    let Some(mut app) = Harness::start() else {
        return;
    };
    let video = joined_video(&app);
    let first = app.path("first.osd");
    let second = app.path("second.osd");
    std::fs::copy(fixture("clip-3s.osd"), &first).expect("a copy");
    std::fs::copy(fixture("clip-3s.osd"), &second).expect("a copy");

    app.send(Message::FileDropped(video));
    assert!(
        app.app.osd.is_none(),
        "there is no .osd next to the joined video"
    );
    app.send(Message::FileChosen(FileKind::Osd, Some(first.clone())));
    assert_eq!(app.app.osd.as_ref().expect("the first .osd").paths.len(), 1);

    app.send(Message::Next);
    app.send(Message::OsdAdded(Some(second.clone())));
    let joined = app.app.osd.as_ref().expect("the joined .osd");
    assert_eq!(joined.paths, [first.clone(), second.clone()]);
    let single = flight_replay_core::osd::OsdFile::open(&first).expect("the first .osd");
    assert!(
        (joined.file.duration_s() - 2.0 * single.duration_s()).abs() < 0.002,
        "the durations add up"
    );
    let video_s = app.app.video.as_ref().expect("the video").info.duration_s;
    assert_eq!(Coverage::new(&joined.parts, video_s).warning(), None);
    app.snapshot("2-input-two-osd");

    app.send(Message::OsdAdded(Some(second.clone())));
    assert!(
        app.app.notices.iter().any(|n| n.contains("already")),
        "the same file is not added twice"
    );

    app.send(Message::MoveOsdFileUp(1));
    assert_eq!(
        app.app.osd.as_ref().expect("the .osd").paths,
        [second, first.clone()]
    );
    app.send(Message::RemoveOsdFile(0));
    assert_eq!(app.app.osd.as_ref().expect("the .osd").paths, [first]);
}

#[test]
fn the_screen_says_when_the_osd_files_do_not_cover_the_video() {
    let Some(mut app) = Harness::start() else {
        return;
    };
    // The clip is three seconds long, and two files of three seconds are six.
    let second = app.path("second.osd");
    std::fs::copy(fixture("clip-3s.osd"), &second).expect("a copy");
    app.send(Message::FileDropped(fixture("clip-3s.mp4")));
    app.send(Message::Next);
    app.send(Message::OsdAdded(Some(second)));

    let osd = app.app.osd.as_ref().expect("the joined .osd");
    let video_s = app.app.video.as_ref().expect("the video").info.duration_s;
    let warning = Coverage::new(&osd.parts, video_s)
        .warning()
        .expect("the files run past the video");
    assert!(
        app.shows(&warning),
        "the warning is on the screen: {warning}"
    );
    app.snapshot("2-input-too-many-osd");
}

#[test]
fn a_queue_renders_the_videos_of_a_folder_one_after_the_other() {
    let Some(mut app) = Harness::start() else {
        return;
    };
    let folder = app.path("flights");
    std::fs::create_dir(&folder).expect("the folder");
    for name in ["a", "b"] {
        for extension in ["mp4", "osd", "srt"] {
            std::fs::copy(
                fixture(&format!("clip-3s.{extension}")),
                folder.join(format!("{name}.{extension}")),
            )
            .expect("a copy");
        }
    }
    // A video without its .osd cannot be rendered with the OSD and the sound.
    std::fs::copy(fixture("clip-3s.mp4"), folder.join("c.mp4")).expect("a copy");
    let font = app.path("font.png");
    write_font(&font);

    app.send(Message::FileDropped(folder.join("a.mp4")));
    app.send(Message::FileDropped(font));
    app.send(Message::GoTo(Step::Render));
    assert_eq!(app.app.step, Step::Render);
    assert!(!app.shows("Start the queue"), "there is nothing to start");

    app.click("Add this video to the queue");
    assert_eq!(app.app.queue.items.len(), 1);
    app.send(Message::QueueFolderChosen(Some(folder.clone())));
    assert_eq!(
        app.app
            .queue
            .items
            .iter()
            .map(|i| i.title.as_str())
            .collect::<Vec<_>>(),
        ["a.mp4", "b.mp4"],
        "a is in the queue already, and c has no .osd"
    );
    let left_out: Vec<_> = app
        .app
        .notices
        .iter()
        .filter(|n| n.starts_with("Left out"))
        .collect();
    assert_eq!(left_out.len(), 2, "{left_out:?}");
    assert!(
        left_out
            .iter()
            .any(|n| n.contains("a.mp4") && n.contains("already planned"))
    );
    assert!(
        left_out
            .iter()
            .any(|n| n.contains("c.mp4") && n.contains(".osd"))
    );
    app.snapshot("7-queue");

    app.click("Start the queue");
    assert!(!app.app.queue.running, "the queue is over");
    for item in &app.app.queue.items {
        assert!(
            matches!(item.status, QueueStatus::Done(_)),
            "{}: {:?}",
            item.title,
            item.status
        );
    }
    for name in ["a_flight-replay.mp4", "b_flight-replay.mp4"] {
        assert!(folder.join(name).is_file(), "{name} was rendered");
    }
    app.snapshot("8-queue-done");

    app.click("Clear the finished");
    assert!(app.app.queue.items.is_empty());
}
