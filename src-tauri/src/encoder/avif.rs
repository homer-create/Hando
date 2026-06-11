// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{decode::DecodedImage, EncodeError, EncodedFile, ImageExt};
use std::io::Write;
use tempfile::NamedTempFile;
use num_cpus;

/// Threads dedicated to a single AVIF encode: half the logical CPUs, clamped to [2, 4].
/// The outer spawn_blocking semaphore already limits file-level concurrency, so giving
/// each AVIF task more threads doesn't cause runaway contention on typical machines.
fn avif_thread_count() -> usize {
    (num_cpus::get() / 2).clamp(2, 4)
}

pub fn encode(decoded: &DecodedImage, quality: u32, speed: u8) -> Result<EncodedFile, EncodeError> {
    // Known limitation: ravif/avif-serialize only write nclx `colr` boxes, so
    // `decoded.icc_profile` cannot be re-embedded here — AVIF output drops the
    // ICC profile (docs/rubric.md §0.5 metadata row).
    let q = quality.clamp(1, 100) as f32;

    let img = ravif::Img::new(
        rgba_as_ravif_pixels(&decoded.rgba),
        decoded.width as usize,
        decoded.height as usize,
    );

    let result = ravif::Encoder::new()
        .with_quality(q)
        .with_speed(speed.clamp(1, 10))
        .with_num_threads(Some(avif_thread_count()))
        .encode_rgba(img)
        .map_err(|e| EncodeError::Encode(format!("ravif: {e}")))?;

    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(&result.avif_file)?;
    tmp.flush()?;
    let len = result.avif_file.len() as u64;
    let (_file, path) = tmp.keep().map_err(|e| EncodeError::Io(e.error))?;

    Ok(EncodedFile { ext: ImageExt::Avif, tmp_path: path, bytes: len })
}

fn rgba_as_ravif_pixels(rgba: &[u8]) -> &[rgb::RGBA<u8>] {
    assert_eq!(rgba.len() % 4, 0);
    // SAFETY: rgb::RGBA<u8> is #[repr(C)] with 4 u8 fields; alignment is 1.
    unsafe {
        std::slice::from_raw_parts(
            rgba.as_ptr() as *const rgb::RGBA<u8>,
            rgba.len() / 4,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::decode::decode;
    use crate::encoder::ImageExt;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
    }

    #[test]
    fn encodes_avif_and_produces_file() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let out = encode(&decoded, 60, 8).unwrap();
        assert_eq!(out.ext, ImageExt::Avif);
        assert!(out.bytes > 0);
        assert!(out.tmp_path.exists());
    }

    #[test]
    fn avif_is_smaller_than_jpeg_source() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let src_bytes = std::fs::metadata(fixture("landscape.jpg")).unwrap().len();
        let out = encode(&decoded, 60, 8).unwrap();
        assert!(out.bytes < src_bytes, "AVIF q60 {} should be < JPEG src {}", out.bytes, src_bytes);
    }
}
