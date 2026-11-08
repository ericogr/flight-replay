// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Installing FFmpeg on request.
//!
//! The application does not carry FFmpeg and never fetches it by itself. When the user asks, [`install_here`]
//! downloads the build for their system (see [`Build`]), checks it against the SHA-256 that is fixed in the code,
//! takes `ffmpeg` and `ffprobe` out of it, runs them once to see that they work, and puts them in
//! [`Tools::managed_dir`]. All of it happens in a temporary folder next to the final one, so a failure or a stop
//! leaves the previous installation, if any, exactly as it was.

mod builds;
mod fetch;
#[cfg(test)]
mod testing;
mod unpack;

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

pub use builds::{Build, Member, Package, Packing};
pub(crate) use fetch::{Curl, Download};

use super::{FfmpegError, Tools};
use crate::cancel::CancelToken;
use crate::display;
use crate::paths;

/// What went wrong with an install.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// There is no build for this system.
    #[error("there is no build of FFmpeg that Flight Replay can install on this system")]
    Unsupported,
    /// `curl` could not be started.
    #[error(
        "curl was not found, so FFmpeg cannot be downloaded from here. Install FFmpeg with your package manager, or choose the folder where it is"
    )]
    NoCurl,
    /// The folder is too full for the download and the files.
    #[error(
        "there is not enough free space in {folder}: about {} are needed and {} are free",
        display::size(*.needed),
        display::size(*.free)
    )]
    NotEnoughSpace {
        /// The folder.
        folder: String,
        /// Bytes needed.
        needed: u64,
        /// Bytes free.
        free: u64,
    },
    /// The download failed. Holds the reason `curl` gave.
    #[error("the download failed: {0}")]
    Download(String),
    /// The server sent more than the size the build says.
    #[error("the file at {0} is larger than expected, so the download was stopped")]
    TooLarge(String),
    /// The file is not the one that was checked.
    #[error(
        "the downloaded file is not the one that was expected (its checksum differs), so it was not installed"
    )]
    ChecksumMismatch,
    /// The package could not be unpacked.
    #[error("could not unpack the download: {0}")]
    Unpack(String),
    /// A file that is needed is not in the package.
    #[error("{0} is not in the download")]
    Missing(&'static str),
    /// The programs were installed but do not work.
    #[error("the downloaded FFmpeg does not work: {0}")]
    Unusable(#[from] FfmpegError),
    /// The copy that was there could not be replaced, for instance because a program of it is running.
    #[error("could not replace the copy of FFmpeg that is there (is it in use?): {0}")]
    Replace(io::Error),
    /// The user stopped it.
    #[error("the install was cancelled")]
    Cancelled,
    /// A file or folder could not be written.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Which part of the install is going on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Getting the file. `done` and `total` are bytes.
    Downloading,
    /// Checking the file against its SHA-256.
    Checking,
    /// Taking the programs out of it.
    Unpacking,
    /// Running the programs once.
    Testing,
}

/// How far an install has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// The part that is going on.
    pub stage: Stage,
    /// Bytes downloaded so far while downloading.
    pub done: u64,
    /// Bytes to download in all.
    pub total: u64,
}

/// Installs `build` in the folder of the application ([`Tools::managed_dir`]), downloading with `curl`, and gives
/// the programs that were installed.
pub fn install_here(
    build: &Build,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<Tools, InstallError> {
    let dest = Tools::managed_dir().ok_or_else(|| {
        InstallError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "the data folder of the user is not known",
        ))
    })?;
    install(
        build,
        &dest,
        &Curl::new(),
        &|dir| Tools::in_dir(dir).map(|_| ()),
        cancel,
        on_progress,
    )?;
    Ok(Tools::in_dir(&dest)?)
}

/// Removes the installation that [`install_here`] made. It is not an error if there is none.
pub fn remove_installed() -> io::Result<()> {
    match Tools::managed_dir() {
        Some(dir) if dir.exists() => fs::remove_dir_all(dir),
        _ => Ok(()),
    }
}

/// Installs `build` in the folder `dest`, which is replaced if it exists.
///
/// `downloader` gets the files, and `works` is asked whether the programs in a folder run, before they are put in
/// place. Both are parameters so that the steps in between can be tested without a network or a real FFmpeg.
pub(crate) fn install(
    build: &Build,
    dest: &Path,
    downloader: &dyn Download,
    works: &dyn Fn(&Path) -> Result<(), FfmpegError>,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<(), InstallError> {
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    fs::create_dir_all(&parent)?;
    check_space(&parent, build)?;
    let staging = Staging::create(&parent)?;

    let total = build.download_bytes();
    let at = |stage, done| Progress { stage, done, total };
    let mut before = 0u64;
    for (index, package) in build.packages.iter().enumerate() {
        let file = staging.path().join(format!("package-{index}"));
        let mut report = |arrived: u64| {
            on_progress(at(Stage::Downloading, before + arrived.min(package.bytes)));
        };
        report(0);
        downloader.fetch(package.url, &file, package.bytes, &mut report, cancel)?;

        let through = before + package.bytes;
        on_progress(at(Stage::Checking, through));
        if sha256_of(&file, cancel)? != package.sha256 {
            return Err(InstallError::ChecksumMismatch);
        }

        on_progress(at(Stage::Unpacking, through));
        unpack::unpack(package, &file, staging.path(), cancel)?;
        fs::remove_file(&file)?;
        before = through;
    }
    write_about(build, staging.path())?;

    on_progress(at(Stage::Testing, total));
    works(staging.path())?;
    if cancel.is_cancelled() {
        return Err(InstallError::Cancelled);
    }
    staging.put_in_place(dest).map_err(InstallError::Replace)?;
    Ok(())
}

/// Fails when `folder` has less room than the install needs: the biggest download and the files it holds. When the
/// free space cannot be read, the install goes on.
fn check_space(folder: &Path, build: &Build) -> Result<(), InstallError> {
    let biggest = build.packages.iter().map(|p| p.bytes).max().unwrap_or(0);
    let needed = biggest + build.unpacked_bytes;
    match paths::free_space(folder) {
        Some(free) if !paths::has_room(free, needed) => Err(InstallError::NotEnoughSpace {
            folder: folder.display().to_string(),
            needed,
            free,
        }),
        _ => Ok(()),
    }
}

/// The SHA-256 of a file, in lower case hexadecimal.
fn sha256_of(path: &Path, cancel: &CancelToken) -> Result<String, InstallError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        if cancel.is_cancelled() {
            return Err(InstallError::Cancelled);
        }
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Writes `ABOUT.txt` next to the programs, so that anyone who finds the folder knows what it is and where it came
/// from.
fn write_about(build: &Build, folder: &Path) -> io::Result<()> {
    let mut text = format!(
        "FFmpeg {version}, installed by Flight Replay\n\
         \n\
         Flight Replay downloaded these programs because you asked it to. They are not part of Flight Replay: FFmpeg\n\
         is a separate program with its own license ({license}; the text is in LICENSE.txt when the build came with\n\
         it). The build was made by {provider}: {page}\n\
         The source code of FFmpeg is at https://ffmpeg.org/download.html\n\
         \n\
         What was downloaded:\n",
        version = build.version,
        license = build.license,
        provider = build.provider,
        page = build.page,
    );
    for package in build.packages {
        let _ = writeln!(text, "  {}\n    SHA-256 {}", package.url, package.sha256);
    }
    text.push_str(
        "\nTo remove it, use \"Remove the FFmpeg that Flight Replay installed\" in the settings of Flight Replay, or\n\
         delete this folder.\n",
    );
    fs::write(folder.join("ABOUT.txt"), text)
}

/// A temporary folder next to the final one. It is removed when it goes out of use, unless it was put in place.
struct Staging {
    path: PathBuf,
    keep: bool,
}

impl Staging {
    fn create(parent: &Path) -> io::Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = parent.join(format!(
            ".ffmpeg-install-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path)?;
        Ok(Self { path, keep: false })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    /// Replaces `dest` with this folder. The old copy is moved aside first and put back if the new one cannot take
    /// its place, so a failure never leaves the user with none.
    fn put_in_place(mut self, dest: &Path) -> io::Result<()> {
        let mut aside = self.path.clone().into_os_string();
        aside.push("-old");
        let aside = PathBuf::from(aside);
        let had_old = dest.exists();
        if had_old {
            fs::rename(dest, &aside)?;
        }
        if let Err(error) = fs::rename(&self.path, dest) {
            if had_old {
                let _ = fs::rename(&aside, dest);
            }
            return Err(error);
        }
        self.keep = true;
        if had_old {
            let _ = fs::remove_dir_all(&aside);
        }
        Ok(())
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[cfg(unix)]
    use super::testing::SPAWNING;
    use super::testing::zip_with;
    use super::*;

    /// A downloader that serves files from memory.
    struct Memory {
        files: HashMap<&'static str, Vec<u8>>,
        /// Stop the install as soon as the first file has been served.
        cancel_after_first: Option<CancelToken>,
    }

    impl Download for Memory {
        fn fetch(
            &self,
            url: &str,
            to: &Path,
            _expected: u64,
            on_bytes: &mut dyn FnMut(u64),
            _cancel: &CancelToken,
        ) -> Result<(), InstallError> {
            let Some(bytes) = self.files.get(url) else {
                return Err(InstallError::Download(
                    "The requested URL returned error: 404".to_owned(),
                ));
            };
            fs::write(to, bytes)?;
            on_bytes(bytes.len() as u64);
            if let Some(token) = &self.cancel_after_first {
                token.cancel();
            }
            Ok(())
        }
    }

    fn leak<T>(value: T) -> &'static T {
        Box::leak(Box::new(value))
    }

    fn hash_of(bytes: &[u8]) -> String {
        let dir = tempfile::tempdir().expect("a folder");
        let file = dir.path().join("f");
        fs::write(&file, bytes).expect("a file");
        sha256_of(&file, &CancelToken::new()).expect("a hash")
    }

    const MEMBERS: &[Member] = &[
        Member {
            path_ends_with: "bin/ffmpeg",
            save_as: "ffmpeg",
            required: true,
            executable: true,
        },
        Member {
            path_ends_with: "bin/ffprobe",
            save_as: "ffprobe",
            required: true,
            executable: true,
        },
        Member {
            path_ends_with: "LICENSE",
            save_as: "LICENSE.txt",
            required: false,
            executable: false,
        },
    ];

    /// A build of one zip, and the downloader that serves it.
    fn fixture(unpacked_bytes: u64) -> (Build, Memory) {
        let archive = zip_with(&[
            ("pkg/bin/ffmpeg", b"ffmpeg program"),
            ("pkg/bin/ffprobe", b"ffprobe program"),
            ("pkg/bin/ffplay", b"not wanted"),
            ("pkg/LICENSE", b"license text"),
        ]);
        let package = leak(Package {
            url: "https://example.invalid/ffmpeg.zip",
            sha256: Box::leak(hash_of(&archive).into_boxed_str()),
            bytes: archive.len() as u64,
            packing: Packing::Zip,
            members: MEMBERS,
        });
        let build = Build {
            version: "7.1",
            provider: "a test",
            page: "https://example.invalid/",
            license: "GPL v3",
            packages: std::slice::from_ref(package),
            unpacked_bytes,
        };
        let memory = Memory {
            files: HashMap::from([(package.url, archive)]),
            cancel_after_first: None,
        };
        (build, memory)
    }

    fn assert_no_leftovers(parent: &Path) {
        assert_eq!(
            leftovers(parent),
            Vec::<String>::new(),
            "work folders were left behind"
        );
    }

    fn leftovers(parent: &Path) -> Vec<String> {
        fs::read_dir(parent)
            .expect("the folder")
            .map(|e| {
                e.expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| name.starts_with(".ffmpeg-install"))
            .collect()
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "it stands in for the check of the programs, which can fail"
    )]
    fn works(_: &Path) -> Result<(), FfmpegError> {
        Ok(())
    }

    #[test]
    fn installs_from_the_package_and_replaces_the_old_copy() {
        let (build, memory) = fixture(1000);
        let root = tempfile::tempdir().expect("a folder");
        let dest = root.path().join("data").join("ffmpeg");
        fs::create_dir_all(&dest).expect("an old copy");
        fs::write(dest.join("old-file"), "old").expect("a file");
        let mut stages = Vec::new();

        install(
            &build,
            &dest,
            &memory,
            &works,
            &CancelToken::new(),
            &mut |p| stages.push(p.stage),
        )
        .expect("installs");

        assert_eq!(
            fs::read(dest.join("ffmpeg")).expect("ffmpeg"),
            b"ffmpeg program"
        );
        assert_eq!(
            fs::read(dest.join("ffprobe")).expect("ffprobe"),
            b"ffprobe program"
        );
        assert_eq!(
            fs::read(dest.join("LICENSE.txt")).expect("license"),
            b"license text"
        );
        assert!(!dest.join("old-file").exists(), "the old copy was replaced");
        assert!(
            !dest.join("ffplay").exists(),
            "what is not named is not kept"
        );
        let about = fs::read_to_string(dest.join("ABOUT.txt")).expect("about");
        assert!(
            about.contains("FFmpeg 7.1") && about.contains("https://example.invalid/ffmpeg.zip")
        );
        assert!(about.contains(build.packages[0].sha256));
        assert_no_leftovers(dest.parent().expect("parent"));
        // The steps come in order, and every one of them is reported.
        let first = |s: Stage| {
            stages
                .iter()
                .position(|x| *x == s)
                .expect("the stage was reported")
        };
        assert!(
            first(Stage::Downloading) < first(Stage::Checking)
                && first(Stage::Checking) < first(Stage::Unpacking)
                && first(Stage::Unpacking) < first(Stage::Testing)
        );
    }

    #[test]
    fn a_file_that_does_not_match_its_checksum_is_refused_and_the_old_copy_stays() {
        let (build, mut memory) = fixture(1000);
        let url = build.packages[0].url;
        let mut tampered = memory.files[url].clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        memory.files.insert(url, tampered);
        let root = tempfile::tempdir().expect("a folder");
        let dest = root.path().join("ffmpeg");
        fs::create_dir_all(&dest).expect("an old copy");
        fs::write(dest.join("ffmpeg"), "old").expect("a file");

        let error = install(
            &build,
            &dest,
            &memory,
            &works,
            &CancelToken::new(),
            &mut |_| {},
        )
        .expect_err("refused");

        assert!(matches!(error, InstallError::ChecksumMismatch), "{error}");
        assert_eq!(fs::read(dest.join("ffmpeg")).expect("kept"), b"old");
        assert_no_leftovers(root.path());
    }

    #[test]
    fn an_address_that_fails_leaves_nothing_behind() {
        let (build, mut memory) = fixture(1000);
        memory.files.clear();
        let root = tempfile::tempdir().expect("a folder");
        let dest = root.path().join("ffmpeg");

        let error = install(
            &build,
            &dest,
            &memory,
            &works,
            &CancelToken::new(),
            &mut |_| {},
        )
        .expect_err("404");

        assert!(
            matches!(&error, InstallError::Download(reason) if reason.contains("404")),
            "{error}"
        );
        assert!(!dest.exists());
        assert_no_leftovers(root.path());
    }

    #[test]
    fn a_stop_asked_for_keeps_the_old_copy_and_removes_the_work() {
        let (build, mut memory) = fixture(1000);
        let cancel = CancelToken::new();
        memory.cancel_after_first = Some(cancel.clone());
        let root = tempfile::tempdir().expect("a folder");
        let dest = root.path().join("ffmpeg");
        fs::create_dir_all(&dest).expect("an old copy");
        fs::write(dest.join("ffmpeg"), "old").expect("a file");

        let error =
            install(&build, &dest, &memory, &works, &cancel, &mut |_| {}).expect_err("cancelled");

        assert!(matches!(error, InstallError::Cancelled), "{error}");
        assert_eq!(fs::read(dest.join("ffmpeg")).expect("kept"), b"old");
        assert_no_leftovers(root.path());
    }

    #[test]
    fn programs_that_do_not_work_are_not_put_in_place() {
        let (build, memory) = fixture(1000);
        let root = tempfile::tempdir().expect("a folder");
        let dest = root.path().join("ffmpeg");
        fs::create_dir_all(&dest).expect("an old copy");
        fs::write(dest.join("ffmpeg"), "old").expect("a file");
        let too_old = |_: &Path| {
            Err(FfmpegError::TooOld {
                tool: "ffmpeg",
                version: "4.4.2".to_owned(),
            })
        };

        let error = install(
            &build,
            &dest,
            &memory,
            &too_old,
            &CancelToken::new(),
            &mut |_| {},
        )
        .expect_err("does not work");

        assert!(matches!(error, InstallError::Unusable(_)), "{error}");
        assert_eq!(fs::read(dest.join("ffmpeg")).expect("kept"), b"old");
        assert_no_leftovers(root.path());
    }

    #[test]
    fn a_swap_that_fails_puts_the_old_copy_back() {
        let root = tempfile::tempdir().expect("a folder");
        let dest = root.path().join("ffmpeg");
        fs::create_dir_all(&dest).expect("an old copy");
        fs::write(dest.join("ffmpeg"), "old").expect("a file");
        let staging = Staging::create(root.path()).expect("a work folder");
        // The work folder is gone before the swap, so the new copy cannot take the place of the old one.
        fs::remove_dir_all(staging.path()).expect("remove the work folder");

        staging
            .put_in_place(&dest)
            .expect_err("nothing to put in place");

        assert_eq!(fs::read(dest.join("ffmpeg")).expect("kept"), b"old");
        assert_no_leftovers(root.path());
    }

    #[test]
    fn a_disk_with_no_room_is_told_before_anything_is_downloaded() {
        let (build, memory) = fixture(u64::MAX / 4);
        let root = tempfile::tempdir().expect("a folder");
        let dest = root.path().join("ffmpeg");

        let error = install(
            &build,
            &dest,
            &memory,
            &works,
            &CancelToken::new(),
            &mut |_| {},
        )
        .expect_err("no room");

        assert!(
            matches!(error, InstallError::NotEnoughSpace { .. }),
            "{error}"
        );
        assert!(error.to_string().contains("are needed"), "{error}");
        assert_no_leftovers(root.path());
    }

    /// A build whose `ffmpeg` and `ffprobe` are scripts that report this version, and the downloader that serves it.
    #[cfg(unix)]
    fn script_fixture(version: &str) -> (Build, Memory) {
        let script = |name: &str| {
            format!(
                "#!/bin/sh\necho '{name} version {version} Copyright (c) the FFmpeg developers'\n"
            )
            .into_bytes()
        };
        let archive = zip_with(&[
            ("pkg/bin/ffmpeg", &script("ffmpeg")),
            ("pkg/bin/ffprobe", &script("ffprobe")),
        ]);
        let package = leak(Package {
            url: "https://example.invalid/p.zip",
            sha256: Box::leak(hash_of(&archive).into_boxed_str()),
            bytes: archive.len() as u64,
            packing: Packing::Zip,
            members: &MEMBERS[..2],
        });
        let build = Build {
            version: "test",
            provider: "a test",
            page: "https://example.invalid/",
            license: "GPL v3",
            packages: std::slice::from_ref(package),
            unpacked_bytes: 1000,
        };
        let memory = Memory {
            files: HashMap::from([(package.url, archive)]),
            cancel_after_first: None,
        };
        (build, memory)
    }

    /// The whole path with real programs: the extracted files are run, as `Tools::in_dir` does.
    #[cfg(unix)]
    #[test]
    fn the_installed_programs_are_run_and_their_version_is_read() {
        let _turn = SPAWNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (build, memory) = script_fixture("7.1");
        let root = tempfile::tempdir().expect("a folder");
        let dest = root.path().join("ffmpeg");

        install(
            &build,
            &dest,
            &memory,
            &|dir| Tools::in_dir(dir).map(|_| ()),
            &CancelToken::new(),
            &mut |_| {},
        )
        .expect("installs");

        let tools = Tools::in_dir(&dest).expect("the installed pair");
        assert_eq!(tools.ffmpeg_version(), Some("7.1"));
        assert_eq!(tools.ffmpeg(), dest.join("ffmpeg"));
    }

    #[cfg(unix)]
    #[test]
    fn programs_that_report_an_old_version_are_not_installed() {
        let _turn = SPAWNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (build, memory) = script_fixture("4.4.2");
        let root = tempfile::tempdir().expect("a folder");
        let dest = root.path().join("ffmpeg");

        let error = install(
            &build,
            &dest,
            &memory,
            &|dir| Tools::in_dir(dir).map(|_| ()),
            &CancelToken::new(),
            &mut |_| {},
        )
        .expect_err("too old");

        assert!(
            matches!(&error, InstallError::Unusable(FfmpegError::TooOld { .. })),
            "{error}"
        );
        assert!(!dest.exists());
        assert_no_leftovers(root.path());
    }
}
