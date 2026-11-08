// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Small helpers for paths, written once so every place treats the odd cases the same way.

use std::path::{Path, PathBuf};

/// The name of a file, for a message. A path with no name (such as `/`) is written whole.
#[must_use]
pub fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The folder a file is in. A bare file name (`clip.mp4`) has none: its folder is the current one, which is not a
/// path that can be created or opened by name, so this gives `None` for it.
#[must_use]
pub fn parent_folder(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|folder| !folder.as_os_str().is_empty())
}

/// The folder a file is in, or the current folder for a bare file name.
#[must_use]
pub fn folder_or_current(path: &Path) -> &Path {
    parent_folder(path).unwrap_or_else(|| Path::new("."))
}

/// The free space, in bytes, on the disk that holds `folder`. A folder that does not exist yet is measured by the
/// closest one above it that does. `None` when the system cannot tell.
#[must_use]
pub fn free_space(folder: &Path) -> Option<u64> {
    let existing = folder.ancestors().find(|ancestor| ancestor.exists())?;
    fs4::available_space(existing).ok()
}

/// Whether `free` bytes hold `needed` bytes with a margin of 5 %, so that a disk is not filled to the last byte.
#[must_use]
pub fn has_room(free: u64, needed: u64) -> bool {
    free >= needed + needed / 20
}

/// The home folder of the user.
#[must_use]
pub fn home_folder() -> Option<PathBuf> {
    directories::UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_needs_a_margin_over_its_size() {
        assert!(has_room(1_050, 1_000));
        assert!(!has_room(1_049, 1_000));
        assert!(has_room(0, 0));
    }

    #[test]
    fn the_free_space_of_a_folder_that_is_not_there_yet_is_that_of_the_one_above() {
        let dir = tempfile::tempdir().expect("a folder");
        let here = free_space(dir.path()).expect("the free space");
        assert!(here > 0);
        let below = dir.path().join("not").join("there").join("yet");
        assert!(free_space(&below).is_some());
    }

    #[test]
    fn a_file_is_named_by_its_last_part() {
        assert_eq!(
            file_name(Path::new("/flights/AvatarG0153.mp4")),
            "AvatarG0153.mp4"
        );
        assert_eq!(file_name(Path::new("clip.osd")), "clip.osd");
        assert_eq!(file_name(Path::new("/")), "/");
    }

    #[test]
    fn a_bare_file_name_has_no_folder_of_its_own() {
        assert_eq!(
            parent_folder(Path::new("/flights/a.mp4")),
            Some(Path::new("/flights"))
        );
        assert_eq!(parent_folder(Path::new("a.mp4")), None);
        assert_eq!(folder_or_current(Path::new("a.mp4")), Path::new("."));
        assert_eq!(folder_or_current(Path::new("out/a.mp4")), Path::new("out"));
    }
}
