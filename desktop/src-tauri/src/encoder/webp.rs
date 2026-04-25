// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{decode::DecodedImage, EncodeError, EncodedFile, ImageExt};
use std::io::Write;
use tempfile::NamedTempFile;

pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    let q = quality.clamp(1, 100);
    let encoder = webp::Encoder::from_rgba(&decoded.rgba, decoded.width, decoded.height);

    let memory = if q >= 100 {
        encoder.encode_lossless()
    } else {
        encoder.encode(q as f32)
    };
    let bytes: &[u8] = &*memory;

    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(bytes)?;
    tmp.flush()?;
    let len = bytes.len() as u64;
    let (_file, path) = tmp.keep().map_err(|e| EncodeError::Io(e.error))?;

    Ok(EncodedFile { ext: ImageExt::Webp, tmp_path: path, bytes: len })
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
    fn encodes_lossy_webp() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let out = encode(&decoded, 80).unwrap();
        assert_eq!(out.ext, ImageExt::Webp);
        assert!(out.bytes > 0);
    }

    #[test]
    fn quality_100_uses_lossless_path() {
        let decoded = decode(&fixture("transparent.png"), ImageExt::Png).unwrap();
        let q100 = encode(&decoded, 100).unwrap();
        let q80 = encode(&decoded, 80).unwrap();
        assert!(q100.bytes >= q80.bytes);
    }
}
