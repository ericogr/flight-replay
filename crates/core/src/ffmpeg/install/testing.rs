// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR

//! What the tests of the install share.

use std::io::{Cursor, Write};
use std::sync::Mutex;

/// Tests that start programs take turns: on Linux a program that has just been written may refuse to start while
/// another thread is in the middle of starting one.
pub(super) static SPAWNING: Mutex<()> = Mutex::new(());

/// A `.zip` archive in memory with these files.
pub(super) fn zip_with(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in files {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer.start_file(*name, options).expect("a file");
        writer.write_all(content).expect("its content");
    }
    writer.finish().expect("the archive").into_inner()
}
