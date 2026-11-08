// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::FfmpegError;

/// The oldest major version of FFmpeg that the application is written for.
pub(super) const MIN_MAJOR_VERSION: u32 = 6;

/// Paths of the `ffmpeg` and `ffprobe` programs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
    /// The release number `ffmpeg` reported, when it could be read.
    ffmpeg_version: Option<String>,
}

impl Tools {
    /// Creates the pair from explicit paths, without checking them.
    #[must_use]
    pub fn new(ffmpeg: PathBuf, ffprobe: PathBuf) -> Self {
        Self {
            ffmpeg,
            ffprobe,
            ffmpeg_version: None,
        }
    }

    /// Finds `ffmpeg` and `ffprobe`, and checks that both run and are not older than version 6.
    ///
    /// `preferred` is a folder or a program path given by the user. The search continues with the
    /// `FLIGHT_REPLAY_FFMPEG` environment variable (same format), `PATH`, the usual install folders of each
    /// system, the folder of the running executable and, last, [`Tools::managed_dir`]. A copy that is too old is
    /// skipped, and the search goes on: a good one further down is used. Only when there is none is the error
    /// [`FfmpegError::TooOld`] given, so that the user is told why the old one was not taken.
    pub fn locate(preferred: Option<&Path>) -> Result<Self, FfmpegError> {
        let mut hints: Vec<PathBuf> = preferred.map(Path::to_path_buf).into_iter().collect();
        hints.extend(env::var_os("FLIGHT_REPLAY_FFMPEG").map(PathBuf::from));

        Self::in_dirs(&candidate_dirs(&hints))
    }

    /// The pair that is in this one folder, checked like [`Tools::locate`] does. It is how a copy that was just
    /// installed is taken, whatever else is on the system.
    pub fn in_dir(dir: &Path) -> Result<Self, FfmpegError> {
        Self::in_dirs(&[dir.to_path_buf()])
    }

    fn in_dirs(dirs: &[PathBuf]) -> Result<Self, FfmpegError> {
        let ffmpeg = pick("ffmpeg", dirs, check)?;
        let ffprobe = pick("ffprobe", dirs, check)?;
        Ok(Self {
            ffmpeg: ffmpeg.path,
            ffprobe: ffprobe.path,
            ffmpeg_version: ffmpeg.version,
        })
    }

    /// The folder where Flight Replay keeps a copy of FFmpeg that it installed itself, in the data folder of the
    /// user. It is searched after every other place, so an FFmpeg that the user has put there on purpose always
    /// wins over it.
    #[must_use]
    pub fn managed_dir() -> Option<PathBuf> {
        crate::config::project_dirs().map(|dirs| dirs.data_local_dir().join("ffmpeg"))
    }

    /// Whether this is the copy that Flight Replay installed itself (see [`Tools::managed_dir`]).
    #[must_use]
    pub fn is_managed(&self) -> bool {
        Self::managed_dir().is_some_and(|dir| self.ffmpeg.starts_with(dir))
    }

    /// The release number `ffmpeg` reported, such as `7.1`. `None` for a build whose version cannot be read, such
    /// as one made from git.
    #[must_use]
    pub fn ffmpeg_version(&self) -> Option<&str> {
        self.ffmpeg_version.as_deref()
    }

    /// Path of `ffmpeg`.
    #[must_use]
    pub fn ffmpeg(&self) -> &Path {
        &self.ffmpeg
    }

    /// Path of `ffprobe`.
    #[must_use]
    pub fn ffprobe(&self) -> &Path {
        &self.ffprobe
    }
}

/// Builds a command for an external program.
#[cfg(windows)]
pub(crate) fn command(program: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Builds a command for an external program.
#[cfg(not(windows))]
pub(crate) fn command(program: &Path) -> Command {
    Command::new(program)
}

/// What the check of a program found.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// It runs, and its version is new enough or cannot be told. Holds the version when it could be read.
    Usable(Option<String>),
    /// It runs, but reports a version older than [`MIN_MAJOR_VERSION`]. Holds the version.
    TooOld(String),
    /// It does not run.
    Broken,
}

/// A program that was found and passed the check.
#[derive(Debug)]
struct Found {
    path: PathBuf,
    version: Option<String>,
}

/// Takes the first usable program called `name` in `dirs`, in order. A copy that is too old does not stop the
/// search, but it is remembered to explain the failure when nothing better turns up.
fn pick(
    name: &'static str,
    dirs: &[PathBuf],
    check: impl Fn(&Path) -> Verdict,
) -> Result<Found, FfmpegError> {
    let file = format!("{name}{}", env::consts::EXE_SUFFIX);
    let mut too_old = None;
    for path in dirs.iter().map(|dir| dir.join(&file)) {
        if !path.is_file() {
            continue;
        }
        match check(&path) {
            Verdict::Usable(version) => return Ok(Found { path, version }),
            Verdict::TooOld(version) => {
                too_old.get_or_insert(version);
            }
            Verdict::Broken => {}
        }
    }
    Err(match too_old {
        Some(version) => FfmpegError::TooOld {
            tool: name,
            version,
        },
        None => FfmpegError::NotFound { tool: name },
    })
}

/// Runs a program with `-version` and judges what it answers.
fn check(program: &Path) -> Verdict {
    let output = command(program)
        .arg("-version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(output) => judge(
            output.status.success(),
            &String::from_utf8_lossy(&output.stdout),
        ),
        Err(_) => Verdict::Broken,
    }
}

/// Judges the answer of `-version`. A build whose version cannot be read (those made from git, such as
/// `N-118000-g1a2b3c4` or `git-2024-05-01`) is accepted: only a version that is clearly too old is refused.
fn judge(success: bool, stdout: &str) -> Verdict {
    if !success {
        return Verdict::Broken;
    }
    let release = stdout
        .lines()
        .next()
        .and_then(version_word)
        .and_then(release);
    match release {
        Some(release) if major(release).is_some_and(|major| major < MIN_MAJOR_VERSION) => {
            Verdict::TooOld(release.to_owned())
        }
        _ => Verdict::Usable(release.map(str::to_owned)),
    }
}

/// The version in the first line of `-version`: `ffmpeg version 6.1.1-3ubuntu5 Copyright …` gives
/// `6.1.1-3ubuntu5`.
fn version_word(line: &str) -> Option<&str> {
    line.split_once(" version ")?.1.split_whitespace().next()
}

/// The release number at the start of a version: `6.1.1-3ubuntu5` gives `6.1.1`, and `n7.1` gives `7.1`. Builds
/// from git have none.
fn release(word: &str) -> Option<&str> {
    let word = word.strip_prefix('n').unwrap_or(word);
    let end = word
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(word.len());
    let number = word.get(..end)?.trim_end_matches('.');
    number
        .starts_with(|c: char| c.is_ascii_digit())
        .then_some(number)
}

fn major(release: &str) -> Option<u32> {
    release.split('.').next()?.parse().ok()
}

fn candidate_dirs(hints: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = hints
        .iter()
        .map(|hint| {
            if hint.is_dir() {
                hint.clone()
            } else {
                hint.parent().map(Path::to_path_buf).unwrap_or_default()
            }
        })
        .collect();

    if let Some(path) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&path));
    }

    dirs.extend(WELL_KNOWN_DIRS.iter().map(PathBuf::from));

    #[cfg(windows)]
    if let Some(local) = env::var_os("LOCALAPPDATA") {
        // Folder where `winget install ffmpeg` puts its links.
        dirs.push(
            PathBuf::from(local)
                .join("Microsoft")
                .join("WinGet")
                .join("Links"),
        );
    }

    // A program that was copied next to Flight Replay is found as well.
    if let Some(parent) = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        dirs.push(parent);
    }

    // The copy that Flight Replay installed itself comes last: an FFmpeg of the user always wins over it.
    dirs.extend(Tools::managed_dir());
    dirs
}

#[cfg(target_os = "macos")]
const WELL_KNOWN_DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin"];

#[cfg(target_os = "linux")]
const WELL_KNOWN_DIRS: &[&str] = &["/usr/local/bin", "/usr/bin", "/snap/bin"];

#[cfg(windows)]
const WELL_KNOWN_DIRS: &[&str] = &["C:\\ffmpeg\\bin"];

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
const WELL_KNOWN_DIRS: &[&str] = &[];

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str =
        "ffmpeg version 4.4.2-0ubuntu0.22.04.1 Copyright (c) 2000-2021 the FFmpeg developers\n";

    #[test]
    fn reads_the_release_of_the_builds_that_exist() {
        let cases = [
            (
                "ffmpeg version 6.1.1-3ubuntu5 Copyright (c) 2000-2023 the FFmpeg developers",
                Some("6.1.1"),
            ),
            (
                "ffmpeg version 4.4.2-0ubuntu0.22.04.1 Copyright (c) 2000-2021",
                Some("4.4.2"),
            ),
            (
                "ffmpeg version n7.1-2-gabcdef Copyright (c) 2000-2024",
                Some("7.1"),
            ),
            (
                "ffmpeg version 7.1-essentials_build-www.gyan.dev Copyright (c) 2000-2024",
                Some("7.1"),
            ),
            (
                "ffprobe version 8.0.1 Copyright (c) 2007-2025",
                Some("8.0.1"),
            ),
            (
                "ffmpeg version N-118000-g1a2b3c4d5e Copyright (c) 2000-2024",
                None,
            ),
            (
                "ffmpeg version git-2024-05-01-abcdef Copyright (c) 2000-2024",
                None,
            ),
            ("something else entirely", None),
        ];
        for (line, expected) in cases {
            assert_eq!(version_word(line).and_then(release), expected, "{line}");
        }
    }

    #[test]
    fn refuses_only_a_version_that_is_clearly_too_old() {
        assert_eq!(judge(true, OLD), Verdict::TooOld("4.4.2".into()));
        assert_eq!(
            judge(true, "ffmpeg version 5.1.6-0+deb12u1 Copyright"),
            Verdict::TooOld("5.1.6".into())
        );
        assert_eq!(
            judge(true, "ffmpeg version 6.0 Copyright"),
            Verdict::Usable(Some("6.0".into()))
        );
        assert_eq!(
            judge(true, "ffmpeg version 8.0.1 Copyright"),
            Verdict::Usable(Some("8.0.1".into()))
        );
        // A build from git has no version to compare: it is taken, not refused.
        assert_eq!(
            judge(true, "ffmpeg version N-118000-g1a2b3c4 Copyright"),
            Verdict::Usable(None)
        );
        assert_eq!(judge(true, ""), Verdict::Usable(None));
        // A program that fails is broken, whatever it printed.
        assert_eq!(judge(false, OLD), Verdict::Broken);
    }

    /// Makes one folder per name, each with an (empty) `ffmpeg` file, and tells what the check says about each.
    fn folders(names: &[&str]) -> (tempfile::TempDir, Vec<PathBuf>) {
        let root = tempfile::tempdir().expect("a temporary folder");
        let dirs = names
            .iter()
            .map(|name| {
                let dir = root.path().join(name);
                std::fs::create_dir(&dir).expect("a folder");
                std::fs::write(dir.join(format!("ffmpeg{}", env::consts::EXE_SUFFIX)), "")
                    .expect("a file");
                dir
            })
            .collect();
        (root, dirs)
    }

    fn by_folder(path: &Path) -> Verdict {
        match path
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
        {
            Some("old") => Verdict::TooOld("4.4.2".into()),
            Some("broken") => Verdict::Broken,
            _ => Verdict::Usable(Some("7.1".into())),
        }
    }

    #[test]
    fn skips_a_copy_that_is_too_old_and_takes_a_good_one_further_down() {
        let (_root, dirs) = folders(&["old", "good"]);
        let found = pick("ffmpeg", &dirs, by_folder).unwrap();
        assert_eq!(
            found.path,
            dirs[1].join(format!("ffmpeg{}", env::consts::EXE_SUFFIX))
        );
        assert_eq!(found.version.as_deref(), Some("7.1"));
    }

    #[test]
    fn keeps_the_first_good_copy_even_when_an_old_one_follows() {
        let (_root, dirs) = folders(&["good", "old"]);
        assert_eq!(
            pick("ffmpeg", &dirs, by_folder).unwrap().path,
            dirs[0].join(format!("ffmpeg{}", env::consts::EXE_SUFFIX))
        );
    }

    #[test]
    fn says_why_when_the_only_copies_are_too_old() {
        let (_root, dirs) = folders(&["broken", "old"]);
        let error = pick("ffmpeg", &dirs, by_folder).unwrap_err();
        assert!(
            matches!(&error, FfmpegError::TooOld { tool: "ffmpeg", version } if version == "4.4.2"),
            "{error:?}"
        );
        assert!(error.to_string().contains("FFmpeg 6 or newer"), "{error}");
    }

    #[test]
    fn says_not_found_when_there_is_nothing_to_use() {
        let (_root, dirs) = folders(&["broken"]);
        assert!(matches!(
            pick("ffmpeg", &dirs, by_folder),
            Err(FfmpegError::NotFound { tool: "ffmpeg" })
        ));
        assert!(matches!(
            pick("ffmpeg", &[], by_folder),
            Err(FfmpegError::NotFound { .. })
        ));
    }

    #[test]
    fn looks_in_the_folder_that_the_application_manages_last() {
        let chosen = tempfile::tempdir().expect("a temporary folder");
        let dirs = candidate_dirs(&[chosen.path().to_path_buf()]);
        // What the user chose comes first and the folder of the application last, so a FFmpeg of the user wins.
        assert_eq!(dirs.first().map(PathBuf::as_path), Some(chosen.path()));
        assert_eq!(dirs.last(), Tools::managed_dir().as_ref());
    }
}
