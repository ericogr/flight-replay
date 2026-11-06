// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Adapted from walksnail-osd-tool (the matching of the OSD and SRT files to a video, GPL-3.0-only, Copyright (C)
// Alexander van Saase) and from fpv-motorsound, internal/pipeline/discover.go (Apache-2.0, Copyright 2026 EricoGR).

//! Finds the files that go with a video: the `.osd` and the `.srt` the goggles recorded next to it.
//!
//! Only a few names are tried, on purpose: guessing more could pair a video with the telemetry of another flight.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Endings that a video can have after the name of its recording, such as the output of walksnail-osd-tool. The
/// `.osd` and `.srt` keep the plain name.
const VIDEO_SUFFIXES: [&str; 3] = ["_with_osd_compressed", "_with_osd", "_compressed"];

/// The files found next to a video.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Companions {
    /// The `.osd` file.
    pub osd: Option<PathBuf>,
    /// The `.srt` file.
    pub srt: Option<PathBuf>,
}

/// A video and what was found with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flight {
    /// The video.
    pub video: PathBuf,
    /// The files that go with it.
    pub companions: Companions,
}

/// Looks next to `video` for an `.osd` and an `.srt` with the same name. If the name of the video ends like the
/// output of walksnail-osd-tool (`_with_osd_compressed`), the name without that ending is tried as well.
#[must_use]
pub fn companions(video: &Path) -> Companions {
    let Some(stem) = video.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
        return Companions::default();
    };
    let dir = video.parent().unwrap_or_else(|| Path::new(""));
    let mut stems = vec![stem.clone()];
    for suffix in VIDEO_SUFFIXES {
        if let Some(prefix) = stem.strip_suffix(suffix).filter(|p| !p.is_empty()) {
            stems.push(prefix.to_owned());
        }
    }
    let find = |extension: &str| {
        stems.iter().find_map(|stem| {
            // Exact case first, then the upper-case extension some cards use.
            [extension.to_owned(), extension.to_uppercase()]
                .iter()
                .map(|ext| dir.join(format!("{stem}.{ext}")))
                .find(|path| path.is_file())
        })
    };
    Companions {
        osd: find("osd"),
        srt: find("srt"),
    }
}

/// The videos of a folder (`.mp4`, any case), sorted by name, each with what was found next to it.
pub fn flights_in(dir: &Path) -> io::Result<Vec<Flight>> {
    let mut videos: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("mp4"))
        })
        .collect();
    videos.sort();
    Ok(videos
        .into_iter()
        .map(|video| Flight {
            companions: companions(&video),
            video,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, b"x").unwrap();
        path
    }

    #[test]
    fn finds_the_files_with_the_same_name() {
        let dir = tempfile::tempdir().unwrap();
        let video = touch(dir.path(), "AvatarG0153.mp4");
        let osd = touch(dir.path(), "AvatarG0153.osd");
        let srt = touch(dir.path(), "AvatarG0153.srt");
        touch(dir.path(), "AvatarG0154.osd");
        assert_eq!(
            companions(&video),
            Companions {
                osd: Some(osd),
                srt: Some(srt)
            }
        );
    }

    #[test]
    fn a_rendered_video_finds_the_files_of_its_recording() {
        let dir = tempfile::tempdir().unwrap();
        let video = touch(dir.path(), "AvatarG0050_with_osd_compressed.mp4");
        let osd = touch(dir.path(), "AvatarG0050.osd");
        let found = companions(&video);
        assert_eq!(found.osd, Some(osd));
        assert_eq!(found.srt, None);
    }

    #[test]
    fn the_name_of_the_video_comes_first_and_other_names_are_not_guessed() {
        let dir = tempfile::tempdir().unwrap();
        let video = touch(dir.path(), "flight_compressed.mp4");
        let exact = touch(dir.path(), "flight_compressed.osd");
        touch(dir.path(), "flight.osd");
        touch(dir.path(), "flight2.osd");
        assert_eq!(companions(&video).osd, Some(exact));
        let alone = touch(dir.path(), "other.mp4");
        assert_eq!(companions(&alone), Companions::default());
    }

    #[test]
    fn finds_an_upper_case_extension() {
        let dir = tempfile::tempdir().unwrap();
        let video = touch(dir.path(), "clip.mp4");
        let osd = touch(dir.path(), "clip.OSD");
        if osd.exists() && dir.path().join("clip.osd").is_file() {
            return; // a case-insensitive file system cannot tell them apart
        }
        assert_eq!(companions(&video).osd, Some(osd));
    }

    #[test]
    fn lists_the_videos_of_a_folder_in_order() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "b.MP4");
        touch(dir.path(), "a.mp4");
        touch(dir.path(), "a.osd");
        touch(dir.path(), "notes.txt");
        let flights = flights_in(dir.path()).unwrap();
        let names: Vec<_> = flights
            .iter()
            .map(|f| f.video.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.mp4", "b.MP4"]);
        assert!(flights[0].companions.osd.is_some());
        assert!(flights[1].companions.osd.is_none());
    }
}
