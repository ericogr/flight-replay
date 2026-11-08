// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! The builds of FFmpeg that the application can install, one for each system it is published for.
//!
//! FFmpeg has no official binaries: its own download page links to builds made by other people. Each build here is
//! fixed with its address, its size and its SHA-256. What was checked about it (that it is GPL, is not built with
//! `--enable-nonfree`, and has `libx264` and `libx265`) holds for exactly that file, and a file that changed since
//! is refused. To use a newer FFmpeg, check the new file the same way, then change the numbers here.
//!
//! The addresses of the providers are not promised to last. When one goes away the install fails with a message,
//! and the user can still install FFmpeg by other means.

/// How a package is packed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packing {
    /// A `.zip` archive.
    Zip,
    /// A `.tar.xz` archive. It is only unpacked on Linux, where the only build that comes this way is used.
    TarXz,
}

/// A file that is taken out of a package.
#[derive(Debug, Clone, Copy)]
pub struct Member {
    /// The end of the path of the file inside the package, such as `bin/ffmpeg`. It matches whole parts of the
    /// path: `bin/ffmpeg` is not found in `sbin/ffmpeg`.
    pub path_ends_with: &'static str,
    /// The name the file is saved under, in the folder of the installation.
    pub save_as: &'static str,
    /// Whether the install fails without it. The license text is kept when the package has it, but is not required.
    pub required: bool,
    /// Whether it is a program, which is made executable.
    pub executable: bool,
}

/// A file to download.
#[derive(Debug, Clone, Copy)]
pub struct Package {
    /// Where to get it. Always `https`.
    pub url: &'static str,
    /// Its SHA-256, in lower case.
    pub sha256: &'static str,
    /// Its size in bytes. The download is stopped if it goes past this.
    pub bytes: u64,
    /// How it is packed.
    pub packing: Packing,
    /// What is taken out of it.
    pub members: &'static [Member],
}

/// A build of FFmpeg: one or more packages that together give `ffmpeg` and `ffprobe`.
#[derive(Debug, Clone, Copy)]
pub struct Build {
    /// The release of FFmpeg, such as `9.0.2`.
    pub version: &'static str,
    /// Who made the build.
    pub provider: &'static str,
    /// A page of the provider that describes the build and gives its sources.
    pub page: &'static str,
    /// The license of the build.
    pub license: &'static str,
    /// What to download.
    pub packages: &'static [Package],
    /// About how many bytes the files take once they are unpacked.
    pub unpacked_bytes: u64,
}

impl Build {
    /// The build for the system the program runs on, if there is one: Windows and Linux on x86-64, and macOS on
    /// Apple Silicon.
    #[must_use]
    pub fn for_this_system() -> Option<&'static Build> {
        if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some(&WINDOWS_X86_64)
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(&MACOS_AARCH64)
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(&LINUX_X86_64)
        } else {
            None
        }
    }

    /// Every build, whatever system this is.
    #[cfg(test)]
    fn all() -> [&'static Build; 3] {
        [&WINDOWS_X86_64, &MACOS_AARCH64, &LINUX_X86_64]
    }

    /// How many bytes are downloaded in all.
    #[must_use]
    pub fn download_bytes(&self) -> u64 {
        self.packages.iter().map(|p| p.bytes).sum()
    }
}

/// Windows, x86-64: the release "essentials" build of gyan.dev, which the FFmpeg site links to.
pub static WINDOWS_X86_64: Build = Build {
    version: "9.0.2",
    provider: "gyan.dev",
    page: "https://www.gyan.dev/ffmpeg/builds/",
    license: "GPL v3",
    packages: &[Package {
        url: "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip",
        sha256: "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba",
        bytes: 114_768_076,
        packing: Packing::Zip,
        members: &[
            Member {
                path_ends_with: "bin/ffmpeg.exe",
                save_as: "ffmpeg.exe",
                required: true,
                executable: true,
            },
            Member {
                path_ends_with: "bin/ffprobe.exe",
                save_as: "ffprobe.exe",
                required: true,
                executable: true,
            },
            Member {
                path_ends_with: "LICENSE",
                save_as: "LICENSE.txt",
                required: false,
                executable: false,
            },
            Member {
                path_ends_with: "README.txt",
                save_as: "BUILD-README.txt",
                required: false,
                executable: false,
            },
        ],
    }],
    unpacked_bytes: 210_700_000,
};

/// macOS, Apple Silicon: the release build of ffmpeg.martin-riedl.de, which comes as one package for each program.
pub static MACOS_AARCH64: Build = Build {
    version: "9.0.2",
    provider: "ffmpeg.martin-riedl.de",
    page: "https://ffmpeg.martin-riedl.de/",
    license: "GPL v3",
    packages: &[
        Package {
            url: "https://ffmpeg.martin-riedl.de/download/macos/arm64/1789931890_9.0.2/ffmpeg.zip",
            sha256: "c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924",
            bytes: 28_395_699,
            packing: Packing::Zip,
            members: &[Member {
                path_ends_with: "ffmpeg",
                save_as: "ffmpeg",
                required: true,
                executable: true,
            }],
        },
        Package {
            url: "https://ffmpeg.martin-riedl.de/download/macos/arm64/1789931890_9.0.2/ffprobe.zip",
            sha256: "fcbe839537485eaee7a7a8bc5cbc0f90d53617e80943e8a5b2e31cb851197ea6",
            bytes: 28_317_701,
            packing: Packing::Zip,
            members: &[Member {
                path_ends_with: "ffprobe",
                save_as: "ffprobe",
                required: true,
                executable: true,
            }],
        },
    ],
    unpacked_bytes: 132_500_000,
};

/// Linux, x86-64: the GPL build of `BtbN`, from the automatic build of 2026-10-31. `BtbN` keeps the last build of each
/// month for two years, which is why a build of the end of a month is the one that is fixed here.
pub static LINUX_X86_64: Build = Build {
    version: "9.0.2",
    provider: "BtbN FFmpeg-Builds",
    page: "https://github.com/BtbN/FFmpeg-Builds",
    license: "GPL v3",
    packages: &[Package {
        url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-10-31-12-59/ffmpeg-n9.0.2-39-g7adfbe40dc-linux64-gpl-9.0.tar.xz",
        sha256: "73e29a56974ae5a651133a04584c7483b21afe941e17dc91d79814a758b608d3",
        bytes: 153_175_100,
        packing: Packing::TarXz,
        members: &[
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
                path_ends_with: "LICENSE.txt",
                save_as: "LICENSE.txt",
                required: false,
                executable: false,
            },
        ],
    }],
    unpacked_bytes: 350_000_000,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_build_is_fixed_and_comes_over_https() {
        for build in Build::all() {
            assert!(!build.version.is_empty() && !build.provider.is_empty());
            assert!(build.page.starts_with("https://"), "{}", build.page);
            assert!(build.unpacked_bytes > build.download_bytes() / 2);
            assert!(!build.packages.is_empty());
            for package in build.packages {
                assert!(package.url.starts_with("https://"), "{}", package.url);
                assert!(package.bytes > 1_000_000, "{}", package.url);
                assert_eq!(package.sha256.len(), 64, "{}", package.url);
                assert!(
                    package
                        .sha256
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                    "{} is not a lower case SHA-256",
                    package.url
                );
            }
        }
    }

    #[test]
    fn every_build_gives_both_programs() {
        for build in Build::all() {
            let programs: Vec<&str> = build
                .packages
                .iter()
                .flat_map(|package| package.members)
                .filter(|member| member.required && member.executable)
                .map(|member| member.save_as.trim_end_matches(".exe"))
                .collect();
            assert!(
                programs.contains(&"ffmpeg") && programs.contains(&"ffprobe"),
                "{}: {programs:?}",
                build.provider
            );
        }
    }
}
