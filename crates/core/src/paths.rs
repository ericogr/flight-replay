// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Small helpers for paths, written once so every place treats the odd cases the same way.

use std::path::Path;

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

#[cfg(test)]
mod tests {
    use super::*;

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
