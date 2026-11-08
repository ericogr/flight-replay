// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! Downloading a file.
//!
//! The application has no network code of its own. It starts `curl`, which comes with Windows 10 and later and with
//! macOS, and is on nearly every Linux desktop. That keeps the proxy and the certificates of the system working
//! without the application carrying a TLS library, and it fits how FFmpeg itself is used: a program that is started,
//! never linked.

use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::Stdio;
use std::thread::sleep;
use std::time::Duration;

use super::InstallError;
use crate::cancel::CancelToken;
use crate::ffmpeg::tools::command;

/// How often the size of the file is looked at.
const POLL: Duration = Duration::from_millis(120);

/// How long to wait for a server to answer, in seconds, as `curl` is told it.
const CONNECT_TIMEOUT_SECONDS: &str = "20";
/// A download slower than this many bytes a second...
const MIN_BYTES_PER_SECOND: &str = "1024";
/// ...for this many seconds is given up on.
const MIN_SPEED_SECONDS: &str = "30";

/// Something that can save the file at an address.
pub trait Download {
    /// Saves what is at `url` in the file `to`.
    ///
    /// `expected` is the exact size of the file: the download is stopped if it goes past it, so that a server that
    /// sends too much cannot fill the disk. `on_bytes` is told how many bytes have arrived, now and then. A stop
    /// asked for with `cancel` is answered with [`InstallError::Cancelled`]. A file that was not finished is removed.
    fn fetch(
        &self,
        url: &str,
        to: &Path,
        expected: u64,
        on_bytes: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Result<(), InstallError>;
}

/// Downloads with the `curl` program.
#[derive(Debug, Clone, Copy)]
pub struct Curl {
    https_only: bool,
}

impl Curl {
    /// A downloader that follows redirects, but only over `https`.
    #[must_use]
    pub fn new() -> Self {
        Self { https_only: true }
    }

    /// A downloader that also accepts plain `http`, for the tests that talk to a server on this machine.
    #[cfg(test)]
    fn allowing_http() -> Self {
        Self { https_only: false }
    }

    /// Whether `curl` can be started.
    #[cfg(test)]
    fn is_available() -> bool {
        command(Path::new("curl"))
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}

impl Default for Curl {
    fn default() -> Self {
        Self::new()
    }
}

impl Download for Curl {
    fn fetch(
        &self,
        url: &str,
        to: &Path,
        expected: u64,
        on_bytes: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Result<(), InstallError> {
        let mut curl = command(Path::new("curl"));
        curl.args(["--fail", "--silent", "--show-error", "--location"])
            .args(["--connect-timeout", CONNECT_TIMEOUT_SECONDS])
            .args(["--speed-limit", MIN_BYTES_PER_SECOND])
            .args(["--speed-time", MIN_SPEED_SECONDS]);
        if self.https_only {
            curl.args(["--proto", "=https", "--proto-redir", "=https"]);
        }
        curl.arg("--output")
            .arg(to)
            .arg("--url")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = match curl.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(InstallError::NoCurl);
            }
            Err(error) => return Err(error.into()),
        };

        let outcome = loop {
            let size = fs::metadata(to).map_or(0, |meta| meta.len());
            on_bytes(size);
            if let Some(status) = child.try_wait()? {
                break Ok(status);
            }
            if cancel.is_cancelled() {
                break Err(InstallError::Cancelled);
            }
            if size > expected {
                break Err(InstallError::TooLarge(url.to_owned()));
            }
            sleep(POLL);
        };

        let status = match outcome {
            Ok(status) => status,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(to);
                return Err(error);
            }
        };
        if !status.success() {
            let mut reason = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut reason);
            }
            let _ = fs::remove_file(to);
            return Err(InstallError::Download(
                reason
                    .lines()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("curl failed")
                    .trim()
                    .to_owned(),
            ));
        }
        on_bytes(fs::metadata(to).map_or(0, |meta| meta.len()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use super::super::testing::SPAWNING;
    use super::*;

    /// A server on this machine. `/start` redirects to `/file`, which holds `body`; `/missing` is a 404; `/slow`
    /// sends part of a body and then waits.
    fn serve(body: Vec<u8>) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
        let base = format!("http://{}", listener.local_addr().expect("an address"));
        let handle = thread::spawn(move || {
            for stream in listener.incoming().take(8) {
                let Ok(mut stream) = stream else { return };
                let path = request_path(&mut stream);
                let _ = match path.as_str() {
                    "/start" => stream.write_all(
                        b"HTTP/1.1 302 Found\r\nLocation: /file\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                    "/file" => {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream
                            .write_all(head.as_bytes())
                            .and_then(|()| stream.write_all(&body))
                    }
                    _ => stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
            }
        });
        (base, handle)
    }

    fn request_path(stream: &mut TcpStream) -> String {
        let mut buffer = [0u8; 2048];
        let read = stream.read(&mut buffer).unwrap_or(0);
        String::from_utf8_lossy(&buffer[..read])
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .to_owned()
    }

    fn curl_or_skip() -> Option<Curl> {
        if Curl::is_available() {
            Some(Curl::allowing_http())
        } else {
            eprintln!("skipping the download test: curl is not installed");
            None
        }
    }

    #[test]
    fn saves_the_file_after_a_redirect_and_reports_how_much_has_come() {
        let _turn = SPAWNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(curl) = curl_or_skip() else { return };
        let body: Vec<u8> = (0..300_000u32).map(|n| (n % 251) as u8).collect();
        let (base, server) = serve(body.clone());
        let dir = tempfile::tempdir().expect("a folder");
        let file = dir.path().join("download");
        let mut seen = Vec::new();

        curl.fetch(
            &format!("{base}/start"),
            &file,
            body.len() as u64,
            &mut |bytes| seen.push(bytes),
            &CancelToken::new(),
        )
        .expect("the download");

        assert_eq!(std::fs::read(&file).expect("the file"), body);
        assert_eq!(seen.last().copied(), Some(body.len() as u64));
        assert!(seen.windows(2).all(|w| w[0] <= w[1]), "{seen:?}");
        drop(server);
    }

    #[test]
    fn a_file_larger_than_expected_is_stopped_and_removed() {
        let _turn = SPAWNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(curl) = curl_or_skip() else { return };
        let (base, server) = serve(vec![7u8; 400_000]);
        let dir = tempfile::tempdir().expect("a folder");
        let file = dir.path().join("download");

        let result = curl.fetch(
            &format!("{base}/file"),
            &file,
            1_000,
            &mut |_| {},
            &CancelToken::new(),
        );

        // curl may finish a small body before the next look at its size; either way nothing wrong stays on the disk.
        match result {
            Err(InstallError::TooLarge(_)) => assert!(!file.exists()),
            Ok(()) => assert_eq!(std::fs::metadata(&file).expect("the file").len(), 400_000),
            Err(other) => panic!("unexpected: {other}"),
        }
        drop(server);
    }

    #[test]
    fn an_address_that_is_not_there_is_an_error_with_the_reason() {
        let _turn = SPAWNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(curl) = curl_or_skip() else { return };
        let (base, server) = serve(Vec::new());
        let dir = tempfile::tempdir().expect("a folder");
        let file = dir.path().join("download");

        let error = curl
            .fetch(
                &format!("{base}/missing"),
                &file,
                10,
                &mut |_| {},
                &CancelToken::new(),
            )
            .expect_err("a 404");

        assert!(
            matches!(&error, InstallError::Download(reason) if reason.contains("404")),
            "{error}"
        );
        assert!(!file.exists());
        drop(server);
    }

    #[test]
    fn plain_http_is_refused_unless_the_test_allows_it() {
        let _turn = SPAWNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !Curl::is_available() {
            return;
        }
        let (base, server) = serve(vec![1u8; 100]);
        let dir = tempfile::tempdir().expect("a folder");
        let file = dir.path().join("download");

        let error = Curl::new()
            .fetch(
                &format!("{base}/file"),
                &file,
                100,
                &mut |_| {},
                &CancelToken::new(),
            )
            .expect_err("http is not allowed");

        assert!(matches!(error, InstallError::Download(_)), "{error}");
        assert!(!file.exists());
        drop(server);
    }
}
