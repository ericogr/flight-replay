// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 EricoGR
//
// Ported from fpv-motorsound, internal/media/wav.go (Apache-2.0, Copyright 2026 EricoGR).

use std::io::{self, Write};

/// Writes a canonical 44-byte PCM header for a stream of `data_bytes` bytes of interleaved 16-bit samples. The size
/// must be known up front, so the output can be a plain writer.
pub fn write_wav_header<W: Write>(
    w: &mut W,
    sample_rate: u32,
    channels: u16,
    data_bytes: u64,
) -> io::Result<()> {
    const BITS: u16 = 16;
    if data_bytes > u64::from(u32::MAX) - 36 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("a WAV of {data_bytes} bytes does not fit the format (4 GiB limit)"),
        ));
    }
    let data_bytes = data_bytes as u32;
    let byte_rate = sample_rate * u32::from(channels) * u32::from(BITS) / 8;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes()); // size of the fmt chunk
    header.extend_from_slice(&1u16.to_le_bytes()); // PCM
    header.extend_from_slice(&channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&(channels * BITS / 8).to_le_bytes()); // block align
    header.extend_from_slice(&BITS.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_bytes.to_le_bytes());
    w.write_all(&header)
}

/// Converts samples to little-endian bytes.
#[must_use]
pub fn pcm16le(samples: &[i16]) -> Vec<u8> {
    samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_canonical_header() {
        let mut bytes = Vec::new();
        write_wav_header(&mut bytes, 48_000, 2, 1000).unwrap();
        assert_eq!(bytes.len(), 44);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 1036);
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 2);
        assert_eq!(
            u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            48_000
        );
        assert_eq!(
            u32::from_le_bytes(bytes[28..32].try_into().unwrap()),
            192_000
        );
        assert_eq!(u16::from_le_bytes(bytes[32..34].try_into().unwrap()), 4);
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 1000);
    }

    #[test]
    fn refuses_a_file_that_does_not_fit() {
        assert!(write_wav_header(&mut Vec::new(), 48_000, 2, u64::from(u32::MAX)).is_err());
    }

    #[test]
    fn encodes_samples_little_endian() {
        assert_eq!(pcm16le(&[1, -2]), vec![1, 0, 0xFE, 0xFF]);
    }
}
