// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! `cargo xtask dist`: the release archives and their checksums.
//!
//! It runs on the system it packages for, after `cargo build --release -p flight-replay`, and needs no tool besides
//! Cargo (and Git for the source archive), so the three systems are packaged by the same code:
//!
//! - Linux: a `.tar.gz` with the program, the desktop entry, the icons and `install.sh`.
//! - Windows: a `.zip` with `flight-replay.exe`.
//! - macOS: a `.zip` with `Flight Replay.app`.
//!
//! Each archive also holds the license, the notices, the README and the license texts of the OSD fonts compiled into
//! the program. `SHA256SUMS` lists every archive.

use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use flate2::Compression;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};

const HELP: &str = "\
Usage: cargo xtask dist [options]
       cargo xtask app-bundle [options]

Options:
  --out <folder>      Where the archives go (default: dist)
  --binary <file>     The release binary (default: target/release/flight-replay)
  --source            Also write the source archive (needs git)

`app-bundle` writes `Flight Replay.app` as a folder instead of an archive.";

/// The icon sizes that go in the Linux icon theme.
const ICON_SIZES: [u32; 7] = [16, 32, 48, 64, 128, 256, 512];

/// The files that go in every archive, from the root of the repository.
const DOCUMENTS: [&str; 5] = [
    "LICENSE",
    "NOTICE",
    "README.md",
    "CHANGELOG.md",
    "THIRD_PARTY_NOTICES.md",
];

/// The notices of the OSD fonts compiled into the program, under `assets/osd-fonts/`. They go in every archive, in
/// the same layout, so that the links of the README there work.
const FONT_NOTICES: [&str; 4] = [
    "README.md",
    "licenses/CC-BY-SA-4.0.txt",
    "licenses/Apache-2.0.txt",
    "licenses/BebasNeue-OFL.txt",
];

/// The system an archive is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Linux,
    Windows,
    Macos,
}

impl Target {
    /// The system this program runs on.
    fn current() -> Result<Self> {
        match std::env::consts::OS {
            "linux" => Ok(Self::Linux),
            "windows" => Ok(Self::Windows),
            "macos" => Ok(Self::Macos),
            other => bail!("there is no package for {other}"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Windows => "windows",
            Self::Macos => "macos",
        }
    }

    fn executable(self) -> &'static str {
        match self {
            Self::Windows => "flight-replay.exe",
            _ => "flight-replay",
        }
    }
}

/// One file of an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The path inside the archive, with `/` between the parts.
    pub path: String,
    pub data: Vec<u8>,
    /// Unix permissions.
    pub mode: u32,
}

impl Entry {
    fn file(path: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            data,
            mode: 0o644,
        }
    }

    fn program(path: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            data,
            mode: 0o755,
        }
    }
}

/// What the packaging needs to know.
pub struct Pack {
    /// The root of the repository.
    pub root: PathBuf,
    /// The release binary.
    pub binary: PathBuf,
    pub version: String,
    pub arch: String,
}

impl Pack {
    /// `flight-replay-<version>-<system>-<architecture>`
    fn base(&self, target: Target) -> String {
        format!(
            "flight-replay-{}-{}-{}",
            self.version,
            target.name(),
            self.arch
        )
    }

    fn read(&self, relative: &str) -> Result<Vec<u8>> {
        let path = self.root.join(relative);
        fs::read(&path).with_context(|| format!("could not read {}", path.display()))
    }
}

/// The files of the archive for `target`, with the paths they have inside it.
pub fn entries(ctx: &Pack, target: Target) -> Result<Vec<Entry>> {
    let base = ctx.base(target);
    let binary = fs::read(&ctx.binary).with_context(|| {
        format!(
            "could not read {}: build it first (make release)",
            ctx.binary.display()
        )
    })?;
    let mut entries = Vec::new();
    for document in DOCUMENTS {
        entries.push(Entry::file(
            format!("{base}/{document}"),
            ctx.read(document)?,
        ));
    }
    for notice in FONT_NOTICES {
        entries.push(Entry::file(
            format!("{base}/osd-fonts/{notice}"),
            ctx.read(&format!("assets/osd-fonts/{notice}"))?,
        ));
    }
    match target {
        Target::Linux => {
            entries.push(Entry::program(format!("{base}/flight-replay"), binary));
            entries.push(Entry::program(
                format!("{base}/install.sh"),
                ctx.read("packaging/linux/install.sh")?,
            ));
            entries.push(Entry::file(
                format!("{base}/share/applications/flight-replay.desktop"),
                ctx.read("packaging/linux/flight-replay.desktop")?,
            ));
            for size in ICON_SIZES {
                entries.push(Entry::file(
                    format!("{base}/share/icons/hicolor/{size}x{size}/apps/flight-replay.png"),
                    ctx.read(&format!("assets/icon/flight-replay-{size}.png"))?,
                ));
            }
            entries.push(Entry::file(
                format!("{base}/share/icons/hicolor/scalable/apps/flight-replay.svg"),
                ctx.read("assets/icon/flight-replay.svg")?,
            ));
        }
        Target::Windows => {
            entries.push(Entry::program(format!("{base}/flight-replay.exe"), binary));
        }
        Target::Macos => entries.extend(app_bundle(ctx, &format!("{base}/"), binary)?),
    }
    Ok(entries)
}

/// The files of `Flight Replay.app`, under `prefix`.
pub fn app_bundle(ctx: &Pack, prefix: &str, binary: Vec<u8>) -> Result<Vec<Entry>> {
    let plist = String::from_utf8(ctx.read("packaging/macos/Info.plist")?)?
        .replace("@VERSION@", &ctx.version);
    let app = format!("{prefix}Flight Replay.app/Contents");
    Ok(vec![
        Entry::file(format!("{app}/Info.plist"), plist.into_bytes()),
        Entry::program(format!("{app}/MacOS/flight-replay"), binary),
        Entry::file(format!("{app}/Resources/flight-replay.icns"), icns(ctx)?),
    ])
}

/// An Apple icon file. It holds PNG pictures: `ic07` is 128 px, `ic08` 256, `ic09` 512, and `ic11`, `ic12` and
/// `ic13` are the 16, 32 and 128 px pictures at twice the density (32, 64 and 256 px).
pub fn icns(ctx: &Pack) -> Result<Vec<u8>> {
    let pictures = [
        (*b"ic11", 32),
        (*b"ic12", 64),
        (*b"ic07", 128),
        (*b"ic13", 256),
        (*b"ic08", 256),
        (*b"ic09", 512),
    ];
    let mut body = Vec::new();
    for (kind, size) in pictures {
        let png = ctx.read(&format!("assets/icon/flight-replay-{size}.png"))?;
        body.extend_from_slice(&kind);
        body.extend_from_slice(&u32::try_from(png.len() + 8)?.to_be_bytes());
        body.extend_from_slice(&png);
    }
    let mut file = b"icns".to_vec();
    file.extend_from_slice(&u32::try_from(body.len() + 8)?.to_be_bytes());
    file.extend(body);
    Ok(file)
}

/// Writes the entries as a gzip-compressed tar archive.
pub fn write_tar_gz(entries: &[Entry], path: &Path) -> Result<()> {
    let file = fs::File::create(path)?;
    let mut tar = tar::Builder::new(GzEncoder::new(file, Compression::best()));
    for entry in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(entry.data.len() as u64);
        header.set_mode(entry.mode);
        header.set_mtime(0);
        header.set_cksum();
        tar.append_data(&mut header, &entry.path, entry.data.as_slice())?;
    }
    tar.into_inner()?.finish()?;
    Ok(())
}

/// Writes the entries as a zip archive.
///
/// The permissions are always written as Unix ones. Left alone, the `zip` crate writes the system it runs on, and on
/// Windows that is DOS, which has no executable bit: the program in `Flight Replay.app` would lose it.
pub fn write_zip(entries: &[Entry], path: &Path) -> Result<()> {
    let mut zip = zip::ZipWriter::new(fs::File::create(path)?);
    for entry in entries {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .system(zip::System::Unix)
            .unix_permissions(entry.mode);
        zip.start_file(&entry.path, options)?;
        zip.write_all(&entry.data)?;
    }
    zip.finish()?;
    Ok(())
}

/// Writes the entries as files in a folder.
fn write_folder(entries: &[Entry], folder: &Path) -> Result<()> {
    for entry in entries {
        let path = folder.join(&entry.path);
        fs::create_dir_all(path.parent().context("a file has a folder")?)?;
        fs::write(&path, &entry.data)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(entry.mode))?;
        }
    }
    Ok(())
}

/// Makes the archive of `target` in `out`, and returns its path.
pub fn archive(ctx: &Pack, target: Target, out: &Path) -> Result<PathBuf> {
    fs::create_dir_all(out)?;
    let entries = entries(ctx, target)?;
    let base = ctx.base(target);
    let path = match target {
        Target::Linux => out.join(format!("{base}.tar.gz")),
        Target::Windows | Target::Macos => out.join(format!("{base}.zip")),
    };
    match target {
        Target::Linux => write_tar_gz(&entries, &path)?,
        Target::Windows | Target::Macos => write_zip(&entries, &path)?,
    }
    Ok(path)
}

/// The source of the commit that is checked out, as the GPL asks to be offered with the program.
fn source_archive(ctx: &Pack, out: &Path) -> Result<PathBuf> {
    let path = out.join(format!("flight-replay-{}-source.tar.gz", ctx.version));
    let status = Command::new("git")
        .current_dir(&ctx.root)
        .args(["archive", "--format=tar.gz"])
        .arg(format!("--prefix=flight-replay-{}/", ctx.version))
        .arg("-o")
        .arg(&path)
        .arg("HEAD")
        .status()
        .context("could not run git")?;
    if !status.success() {
        bail!("git archive failed");
    }
    Ok(path)
}

/// The lines of `SHA256SUMS` for these files, in the format of `sha256sum`.
pub fn checksums(files: &[PathBuf]) -> Result<String> {
    let mut sorted = files.to_vec();
    sorted.sort();
    let mut text = String::new();
    for file in sorted {
        let digest = Sha256::digest(fs::read(&file)?);
        let name = file
            .file_name()
            .context("an archive has a name")?
            .to_string_lossy();
        writeln!(text, "{digest:x}  {name}")?;
    }
    Ok(text)
}

struct Options {
    out: PathBuf,
    binary: Option<PathBuf>,
    source: bool,
}

fn parse(args: Vec<String>) -> Result<Options> {
    let mut options = Options {
        out: PathBuf::from("dist"),
        binary: None,
        source: false,
    };
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--out" => options.out = args.next().context("--out needs a folder")?.into(),
            "--binary" => {
                options.binary = Some(args.next().context("--binary needs a file")?.into());
            }
            "--source" => options.source = true,
            "--help" | "-h" => {
                println!("{HELP}");
                std::process::exit(0);
            }
            other => bail!("unknown option {other:?}\n\n{HELP}"),
        }
    }
    Ok(options)
}

fn context(options: &Options, target: Target) -> Result<Pack> {
    let root = repository_root()?;
    Ok(Pack {
        binary: options
            .binary
            .clone()
            .unwrap_or_else(|| root.join("target/release").join(target.executable())),
        root,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
    })
}

/// The root of the repository: the parent of the folder of this package.
fn repository_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("the repository has a root")
}

/// `cargo xtask dist`
pub fn run(args: Vec<String>) -> Result<()> {
    let options = parse(args)?;
    let target = Target::current()?;
    let ctx = context(&options, target)?;
    let mut files = vec![archive(&ctx, target, &options.out)?];
    if options.source {
        files.push(source_archive(&ctx, &options.out)?);
    }
    let sums = options.out.join("SHA256SUMS");
    fs::write(&sums, checksums(&files)?)?;
    for file in files.iter().chain([&sums]) {
        println!("{}", file.display());
    }
    Ok(())
}

/// `cargo xtask app-bundle`
pub fn run_bundle(args: Vec<String>) -> Result<()> {
    let options = parse(args)?;
    let ctx = context(&options, Target::Macos)?;
    let binary = fs::read(&ctx.binary).with_context(|| {
        format!(
            "could not read {}: build it first (make release)",
            ctx.binary.display()
        )
    })?;
    let entries = app_bundle(&ctx, "", binary)?;
    write_folder(&entries, &options.out)?;
    println!("{}", options.out.join("Flight Replay.app").display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use super::*;

    /// A context with a fake binary, on the real files of the repository.
    fn context(dir: &Path) -> Pack {
        let binary = dir.join("flight-replay");
        fs::write(&binary, b"\x7fELF not really").unwrap();
        Pack {
            root: repository_root().unwrap(),
            binary,
            version: "1.2.3".into(),
            arch: "x86_64".into(),
        }
    }

    fn tar_names(path: &Path) -> Vec<(String, u32)> {
        let decoder = flate2::read::GzDecoder::new(fs::File::open(path).unwrap());
        tar::Archive::new(decoder)
            .entries()
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.path().unwrap().to_string_lossy().into_owned(),
                    e.header().mode().unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn the_linux_archive_has_the_program_the_desktop_entry_and_the_icons() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let path = archive(&ctx, Target::Linux, &dir.path().join("out")).unwrap();
        assert!(path.ends_with("flight-replay-1.2.3-linux-x86_64.tar.gz"));

        let names = tar_names(&path);
        let has = |name: &str| {
            names
                .iter()
                .any(|(n, _)| n == &format!("flight-replay-1.2.3-linux-x86_64/{name}"))
        };
        for name in [
            "flight-replay",
            "install.sh",
            "LICENSE",
            "NOTICE",
            "README.md",
            "share/applications/flight-replay.desktop",
            "share/icons/hicolor/256x256/apps/flight-replay.png",
            "share/icons/hicolor/scalable/apps/flight-replay.svg",
        ] {
            assert!(has(name), "{name} is in the archive: {names:?}");
        }
        let program = names
            .iter()
            .find(|(n, _)| n.ends_with("/flight-replay"))
            .unwrap();
        assert_eq!(program.1 & 0o111, 0o111, "the program is executable");
    }

    #[test]
    fn every_archive_carries_the_notices_of_the_fonts() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        for target in [Target::Linux, Target::Windows, Target::Macos] {
            let entries = entries(&ctx, target).unwrap();
            for notice in FONT_NOTICES {
                let path = format!("{}/osd-fonts/{notice}", ctx.base(target));
                let entry = entries
                    .iter()
                    .find(|e| e.path == path)
                    .unwrap_or_else(|| panic!("{path} is in the {} archive", target.name()));
                assert!(!entry.data.is_empty(), "{path} has content");
            }
        }
    }

    #[test]
    fn the_windows_archive_has_the_exe() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = context(dir.path());
        let exe = dir.path().join("flight-replay.exe");
        fs::write(&exe, b"MZ").unwrap();
        ctx.binary = exe;
        let path = archive(&ctx, Target::Windows, &dir.path().join("out")).unwrap();
        assert!(path.ends_with("flight-replay-1.2.3-windows-x86_64.zip"));

        let mut zip = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
            .collect();
        assert!(names.contains(&"flight-replay-1.2.3-windows-x86_64/flight-replay.exe".to_owned()));
        assert!(names.contains(&"flight-replay-1.2.3-windows-x86_64/LICENSE".to_owned()));
        let mut exe = Vec::new();
        zip.by_name("flight-replay-1.2.3-windows-x86_64/flight-replay.exe")
            .unwrap()
            .read_to_end(&mut exe)
            .unwrap();
        assert_eq!(exe, b"MZ");
    }

    #[test]
    fn the_macos_archive_has_a_bundle_with_the_version_and_the_icon() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let path = archive(&ctx, Target::Macos, &dir.path().join("out")).unwrap();

        let mut zip = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
        let app = "flight-replay-1.2.3-macos-x86_64/Flight Replay.app/Contents";
        let mut plist = String::new();
        zip.by_name(&format!("{app}/Info.plist"))
            .unwrap()
            .read_to_string(&mut plist)
            .unwrap();
        assert!(plist.contains("<string>1.2.3</string>"), "{plist}");
        assert!(plist.contains("io.github.ericogr.flight-replay"));
        assert!(!plist.contains("@VERSION@"));
        let mode = zip
            .by_name(&format!("{app}/MacOS/flight-replay"))
            .unwrap()
            .unix_mode()
            .unwrap();
        assert_eq!(mode & 0o111, 0o111, "the program is executable");
        assert!(
            zip.by_name(&format!("{app}/Resources/flight-replay.icns"))
                .is_ok()
        );
    }

    #[test]
    fn the_icon_file_has_the_apple_layout() {
        let dir = tempfile::tempdir().unwrap();
        let icon = icns(&context(dir.path())).unwrap();
        assert_eq!(&icon[..4], b"icns");
        assert_eq!(
            u32::from_be_bytes(icon[4..8].try_into().unwrap()) as usize,
            icon.len(),
            "the length in the header is the length of the file"
        );
        // Every picture is a PNG, and the pictures fill the file exactly.
        let mut at = 8;
        let mut kinds = Vec::new();
        while at < icon.len() {
            let length = u32::from_be_bytes(icon[at + 4..at + 8].try_into().unwrap()) as usize;
            assert_eq!(&icon[at + 8..at + 12], b"\x89PNG", "picture at {at}");
            kinds.push(String::from_utf8_lossy(&icon[at..at + 4]).into_owned());
            at += length;
        }
        assert_eq!(at, icon.len());
        assert_eq!(kinds, ["ic11", "ic12", "ic07", "ic13", "ic08", "ic09"]);
    }

    #[test]
    fn the_checksums_follow_the_format_of_sha256sum() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path().join("b.zip");
        let a = dir.path().join("a.tar.gz");
        fs::write(&b, b"two").unwrap();
        fs::write(&a, b"one").unwrap();
        let text = checksums(&[b, a]).unwrap();
        assert_eq!(
            text,
            "7692c3ad3540bb803c020b3aee66cd8887123234ea0c6e7143c0add73ff431ed  a.tar.gz\n\
             3fc4ccfe745870e2c0d99f71f30ff0656c8dedd41cc1d7d3d376b0dbe685e2f3  b.zip\n"
        );
    }
}
