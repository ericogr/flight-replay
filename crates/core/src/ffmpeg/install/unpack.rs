// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Taking the programs out of a downloaded package.
//!
//! Only the files named in the build are taken, and each is saved under a name from the build, never under the
//! name it has in the package. A package with a path such as `../../x` cannot write outside the folder.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use super::InstallError;
use super::builds::{Member, Package, Packing};
use crate::cancel::CancelToken;

/// The largest file taken out of a package. A real build of FFmpeg is under 200 MB.
const MAX_MEMBER_BYTES: u64 = 1 << 30;

/// Takes the files of `package.members` out of `archive` into `into`.
pub(super) fn unpack(
    package: &Package,
    archive: &Path,
    into: &Path,
    cancel: &CancelToken,
) -> Result<(), InstallError> {
    let mut taking = Taking::new(package.members, into, cancel);
    match package.packing {
        Packing::Zip => unpack_zip(archive, &mut taking)?,
        Packing::TarXz => unpack_tar_xz(archive, &mut taking)?,
    }
    taking.finish()
}

/// The files being taken out of an archive. Both kinds of archive are walked entry by entry and hand each file to
/// [`Taking::offer`], so what is wanted, where it is saved and when to stop are decided in one place.
struct Taking<'a> {
    members: &'a [Member],
    saved: Vec<bool>,
    into: &'a Path,
    cancel: &'a CancelToken,
}

impl<'a> Taking<'a> {
    fn new(members: &'a [Member], into: &'a Path, cancel: &'a CancelToken) -> Self {
        Self {
            members,
            saved: vec![false; members.len()],
            into,
            cancel,
        }
    }

    /// Saves the file at `path` of the archive if it is one that is wanted and not saved yet. `size` is the size the
    /// archive says. Gives `true` when every file has been saved, so the walk can stop.
    fn offer(
        &mut self,
        path: &str,
        size: u64,
        reader: &mut dyn Read,
    ) -> Result<bool, InstallError> {
        let wanted = self
            .members
            .iter()
            .zip(&self.saved)
            .position(|(member, saved)| !saved && matches(path, member.path_ends_with));
        if let Some(index) = wanted {
            let member = &self.members[index];
            save(
                reader,
                size,
                &self.into.join(member.save_as),
                member.executable,
                self.cancel,
            )?;
            self.saved[index] = true;
        }
        Ok(self.saved.iter().all(|saved| *saved))
    }

    /// Fails if a file that is required was not in the archive.
    fn finish(self) -> Result<(), InstallError> {
        match self
            .members
            .iter()
            .zip(&self.saved)
            .find(|(member, saved)| member.required && !**saved)
        {
            Some((member, _)) => Err(InstallError::Missing(member.save_as)),
            None => Ok(()),
        }
    }
}

/// Whether the path of a file in a package ends with `suffix`, on a whole part of the path.
fn matches(path: &str, suffix: &str) -> bool {
    let path = path.replace('\\', "/");
    path == suffix || path.ends_with(&format!("/{suffix}"))
}

fn unpack_zip(archive: &Path, taking: &mut Taking<'_>) -> Result<(), InstallError> {
    let unpack_error = |error: zip::result::ZipError| InstallError::Unpack(error.to_string());
    let mut zip = zip::ZipArchive::new(File::open(archive)?).map_err(unpack_error)?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(unpack_error)?;
        if entry.is_dir() {
            continue;
        }
        // A name that is not valid text cannot be one of the files that are looked for.
        let Ok(name) = entry.name().map(std::borrow::Cow::into_owned) else {
            continue;
        };
        let size = entry.size();
        if taking.offer(&name, size, &mut entry)? {
            break;
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn unpack_tar_xz(archive: &Path, taking: &mut Taking<'_>) -> Result<(), InstallError> {
    let file = std::io::BufReader::new(File::open(archive)?);
    let mut tar = tar::Archive::new(liblzma::read::XzDecoder::new(file));
    let unpack_error = |error: std::io::Error| InstallError::Unpack(error.to_string());
    for entry in tar.entries().map_err(unpack_error)? {
        let mut entry = entry.map_err(unpack_error)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(unpack_error)?
            .to_string_lossy()
            .into_owned();
        let size = entry.size();
        if taking.offer(&path, size, &mut entry)? {
            break;
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn unpack_tar_xz(_archive: &Path, _taking: &mut Taking<'_>) -> Result<(), InstallError> {
    Err(InstallError::Unpack(
        "a .tar.xz package is only unpacked on Linux".to_owned(),
    ))
}

/// Writes what `reader` gives to `to`, and makes it a program if `executable`. `declared` is the size the package
/// says.
fn save(
    reader: &mut dyn Read,
    declared: u64,
    to: &Path,
    executable: bool,
    cancel: &CancelToken,
) -> Result<(), InstallError> {
    if declared > MAX_MEMBER_BYTES {
        return Err(InstallError::Unpack(format!(
            "{} is larger than expected",
            to.display()
        )));
    }
    let mut out = File::create(to)?;
    let mut buffer = vec![0u8; 1 << 20];
    let mut total = 0u64;
    loop {
        if cancel.is_cancelled() {
            return Err(InstallError::Cancelled);
        }
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > MAX_MEMBER_BYTES {
            return Err(InstallError::Unpack(format!(
                "{} is larger than expected",
                to.display()
            )));
        }
        out.write_all(&buffer[..read])?;
    }
    out.flush()?;
    drop(out);
    if executable {
        make_executable(to)?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
#[allow(
    clippy::unnecessary_wraps,
    reason = "the same signature as the one for Unix, where setting the permissions can fail"
)]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::builds::Member;
    use super::super::testing::zip_with;
    use super::*;

    const FFMPEG: Member = Member {
        path_ends_with: "bin/ffmpeg",
        save_as: "ffmpeg",
        required: true,
        executable: true,
    };
    const LICENSE: Member = Member {
        path_ends_with: "LICENSE",
        save_as: "LICENSE.txt",
        required: false,
        executable: false,
    };

    fn package(packing: Packing, members: &'static [Member]) -> Package {
        Package {
            url: "https://example.invalid/p",
            sha256: "",
            bytes: 0,
            packing,
            members,
        }
    }

    #[test]
    fn matches_whole_parts_of_the_path() {
        assert!(matches("ffmpeg-9/bin/ffmpeg", "bin/ffmpeg"));
        assert!(matches("bin/ffmpeg", "bin/ffmpeg"));
        assert!(matches("ffmpeg-9\\bin\\ffmpeg", "bin/ffmpeg"));
        assert!(!matches("ffmpeg-9/sbin/ffmpeg", "bin/ffmpeg"));
        assert!(!matches("ffmpeg-9/bin/ffmpeg.exe", "bin/ffmpeg"));
    }

    #[test]
    fn takes_only_the_named_files_under_the_names_of_the_build() {
        let dir = tempfile::tempdir().expect("a folder");
        let archive = dir.path().join("p.zip");
        std::fs::write(
            &archive,
            zip_with(&[
                ("pkg/doc/readme.txt", b"not wanted"),
                ("pkg/bin/ffplay", b"not wanted"),
                ("pkg/bin/ffmpeg", b"the program"),
                ("pkg/LICENSE", b"the license"),
            ]),
        )
        .expect("the archive");
        let into = dir.path().join("out");
        std::fs::create_dir(&into).expect("a folder");

        unpack(
            &package(Packing::Zip, &[FFMPEG, LICENSE]),
            &archive,
            &into,
            &CancelToken::new(),
        )
        .expect("unpacks");

        assert_eq!(
            std::fs::read(into.join("ffmpeg")).expect("ffmpeg"),
            b"the program"
        );
        assert_eq!(
            std::fs::read(into.join("LICENSE.txt")).expect("license"),
            b"the license"
        );
        let mut names: Vec<_> = std::fs::read_dir(&into)
            .expect("the folder")
            .map(|e| {
                e.expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        assert_eq!(names, ["LICENSE.txt", "ffmpeg"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(into.join("ffmpeg"))
                .expect("meta")
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "a program can be run");
            let mode = std::fs::metadata(into.join("LICENSE.txt"))
                .expect("meta")
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0, "a text is not made a program");
        }
    }

    #[test]
    fn a_path_in_the_package_cannot_write_outside_the_folder() {
        let dir = tempfile::tempdir().expect("a folder");
        let archive = dir.path().join("p.zip");
        std::fs::write(&archive, zip_with(&[("../../bin/ffmpeg", b"x")])).expect("the archive");
        let into = dir.path().join("a").join("b");
        std::fs::create_dir_all(&into).expect("a folder");

        unpack(
            &package(Packing::Zip, &[FFMPEG]),
            &archive,
            &into,
            &CancelToken::new(),
        )
        .expect("unpacks");

        assert!(into.join("ffmpeg").is_file());
        assert!(!dir.path().join("bin").exists() && !dir.path().join("ffmpeg").exists());
    }

    #[test]
    fn a_package_without_a_required_file_is_refused() {
        let dir = tempfile::tempdir().expect("a folder");
        let archive = dir.path().join("p.zip");
        std::fs::write(&archive, zip_with(&[("pkg/LICENSE", b"only this")])).expect("the archive");

        let error = unpack(
            &package(Packing::Zip, &[FFMPEG, LICENSE]),
            &archive,
            dir.path(),
            &CancelToken::new(),
        )
        .expect_err("ffmpeg is missing");

        assert!(matches!(error, InstallError::Missing("ffmpeg")), "{error}");
    }

    #[test]
    fn a_file_that_is_not_an_archive_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().expect("a folder");
        let archive = dir.path().join("p.zip");
        std::fs::write(&archive, b"this is not a zip").expect("a file");

        let error = unpack(
            &package(Packing::Zip, &[FFMPEG]),
            &archive,
            dir.path(),
            &CancelToken::new(),
        )
        .expect_err("not a zip");

        assert!(matches!(error, InstallError::Unpack(_)), "{error}");
    }

    #[test]
    fn a_stop_asked_for_ends_the_unpacking() {
        let dir = tempfile::tempdir().expect("a folder");
        let archive = dir.path().join("p.zip");
        std::fs::write(&archive, zip_with(&[("pkg/bin/ffmpeg", b"x")])).expect("the archive");
        let cancel = CancelToken::new();
        cancel.cancel();

        let error = unpack(
            &package(Packing::Zip, &[FFMPEG]),
            &archive,
            dir.path(),
            &cancel,
        )
        .expect_err("cancelled");

        assert!(matches!(error, InstallError::Cancelled), "{error}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unpacks_a_tar_xz() {
        let dir = tempfile::tempdir().expect("a folder");
        let archive = dir.path().join("p.tar.xz");
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for (name, content) in [
                ("pkg/bin/ffplay", &b"not wanted"[..]),
                ("pkg/bin/ffmpeg", &b"the program"[..]),
                ("pkg/LICENSE", &b"the license"[..]),
            ] {
                let mut header = tar::Header::new_gnu();
                header.set_size(content.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append_data(&mut header, name, content)
                    .expect("an entry");
            }
            builder.finish().expect("the tar");
        }
        let mut encoder =
            liblzma::write::XzEncoder::new(File::create(&archive).expect("a file"), 1);
        encoder.write_all(&tar_bytes).expect("compress");
        encoder.finish().expect("finish");
        let into = dir.path().join("out");
        std::fs::create_dir(&into).expect("a folder");

        unpack(
            &package(Packing::TarXz, &[FFMPEG, LICENSE]),
            &archive,
            &into,
            &CancelToken::new(),
        )
        .expect("unpacks");

        assert_eq!(
            std::fs::read(into.join("ffmpeg")).expect("ffmpeg"),
            b"the program"
        );
        assert_eq!(
            std::fs::read(into.join("LICENSE.txt")).expect("license"),
            b"the license"
        );
    }
}
