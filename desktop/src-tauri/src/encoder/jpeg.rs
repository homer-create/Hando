// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{decode::DecodedImage, EncodeError, EncodedFile, ImageExt};
use std::io::Write;
use tempfile::NamedTempFile;

pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    let q = quality.clamp(1, 100) as f32;

    let mut compress = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    compress.set_size(decoded.width as usize, decoded.height as usize);
    compress.set_quality(q);
    compress.set_progressive_mode();
    compress.set_optimize_scans(true);

    let mut comp = compress.start_compress(Vec::new())
        .map_err(|e| EncodeError::Encode(format!("mozjpeg start: {e}")))?;

    // JPEG doesn't support alpha — convert RGBA → RGB by dropping alpha
    let rgb: Vec<u8> = decoded.rgba.chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();

    comp.write_scanlines(&rgb)
        .map_err(|e| EncodeError::Encode(format!("mozjpeg scanlines: {e}")))?;
    let bytes = comp.finish()
        .map_err(|e| EncodeError::Encode(format!("mozjpeg finish: {e}")))?;

    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(&bytes)?;
    tmp.flush()?;
    let len = bytes.len() as u64;
    let (_file, path) = tmp.keep().map_err(|e| EncodeError::Io(e.error))?;

    Ok(EncodedFile { ext: ImageExt::Jpeg, tmp_path: path, bytes: len })
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
    fn encodes_jpeg_smaller_than_input_at_q80() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let src_bytes = std::fs::metadata(fixture("landscape.jpg")).unwrap().len();
        let out = encode(&decoded, 80).unwrap();
        assert!(out.bytes < src_bytes, "out {} should be < src {}", out.bytes, src_bytes);
        assert!(out.tmp_path.exists());
    }

    #[test]
    fn higher_quality_produces_larger_file() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let q40 = encode(&decoded, 40).unwrap();
        let q90 = encode(&decoded, 90).unwrap();
        assert!(q90.bytes > q40.bytes);
    }

    #[test]
    fn output_extension_is_jpeg() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let out = encode(&decoded, 80).unwrap();
        assert_eq!(out.ext, ImageExt::Jpeg);
    }
}
