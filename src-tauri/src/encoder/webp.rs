// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{decode::DecodedImage, icc, EncodeError, EncodedFile, ImageExt};
use std::io::Write;
use tempfile::NamedTempFile;

pub fn encode(decoded: &DecodedImage, quality: u32, method: u8) -> Result<EncodedFile, EncodeError> {
    let q = quality.clamp(1, 100);
    let encoder = webp::Encoder::from_rgba(&decoded.rgba, decoded.width, decoded.height);

    let mut config = webp::WebPConfig::new()
        .map_err(|_| EncodeError::Encode("webp config init".into()))?;
    config.method = method.min(6) as i32;
    if q >= 100 {
        config.lossless = 1;
        config.quality = 75.0; // in lossless mode quality = effort, not fidelity
        // keep RGB values under fully-transparent pixels so the rubric's
        // pixel-identical gate (§2) holds on images with alpha
        config.exact = 1;
    } else {
        config.lossless = 0;
        config.quality = q as f32;
    }

    let memory = encoder.encode_advanced(&config)
        .map_err(|e| EncodeError::Encode(format!("webp encode: {e:?}")))?;
    let bytes: &[u8] = &*memory;

    // ICC passthrough: libwebp's one-shot encoder emits the simple container,
    // so the profile is spliced in as a VP8X + ICCP chunk afterwards. On a
    // malformed container this keeps the profile-less encode instead of
    // failing the file.
    let tagged = decoded.icc_profile.as_deref()
        .filter(|p| !p.is_empty())
        .and_then(|profile| icc::embed_webp_icc(bytes, profile, decoded.width, decoded.height));
    let bytes = tagged.as_deref().unwrap_or(bytes);

    // EXIF passthrough (opt-in): same container surgery, EXIF chunk at the end
    let exif_tagged = decoded.exif.as_deref()
        .filter(|e| !e.is_empty())
        .and_then(|exif| super::metadata::embed_webp_exif(bytes, exif, decoded.width, decoded.height));
    let bytes = exif_tagged.as_deref().unwrap_or(bytes);

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
        let out = encode(&decoded, 80, 4).unwrap();
        assert_eq!(out.ext, ImageExt::Webp);
        assert!(out.bytes > 0);
    }

    #[test]
    fn quality_100_is_pixel_identical_lossless() {
        // The real invariant for q100: decoded output matches the input
        // exactly (rubric §2 gate), including RGB under transparent pixels
        // thanks to config.exact = 1.
        let decoded = decode(&fixture("transparent.png"), ImageExt::Png).unwrap();
        let q100 = encode(&decoded, 100, 4).unwrap();
        let back = decode(&q100.tmp_path, ImageExt::Webp).unwrap();
        let _ = std::fs::remove_file(&q100.tmp_path);
        assert!(
            crate::encoder::judge::pixels_identical(&decoded, &back),
            "webp q100 must be lossless"
        );
    }

    #[test]
    fn icc_profile_roundtrips_lossy_and_lossless() {
        // transparent.png exercises both container shapes: lossy+alpha makes
        // libwebp emit VP8X+ALPH itself (existing-VP8X path), lossless VP8L
        // uses the simple container (fresh-VP8X path). Extraction goes through
        // image-webp, which is independent of our RIFF surgery.
        let mut decoded = decode(&fixture("transparent.png"), ImageExt::Png).unwrap();
        let profile = crate::encoder::icc::test_profile(3000);
        decoded.icc_profile = Some(profile.clone());
        for q in [80u32, 100] {
            let out = encode(&decoded, q, 4).unwrap();
            let back = decode(&out.tmp_path, ImageExt::Webp).unwrap();
            let _ = std::fs::remove_file(&out.tmp_path);
            assert_eq!(back.icc_profile.as_deref(), Some(profile.as_slice()), "q{q}");
        }
    }

    #[test]
    fn exif_roundtrips_with_and_without_icc() {
        // Both metadata chunks at once exercises the two-pass container
        // surgery (ICC then EXIF on the already-upgraded VP8X container)
        let mut decoded = decode(&fixture("transparent.png"), ImageExt::Png).unwrap();
        let exif = crate::encoder::metadata::test_exif(1, true);
        decoded.exif = Some(exif.clone());
        for with_icc in [false, true] {
            decoded.icc_profile = with_icc.then(|| crate::encoder::icc::test_profile(3000));
            for q in [80u32, 100] {
                let out = encode(&decoded, q, 4).unwrap();
                let back = decode(&out.tmp_path, ImageExt::Webp).unwrap();
                let _ = std::fs::remove_file(&out.tmp_path);
                assert_eq!(back.exif.as_deref(), Some(exif.as_slice()), "q{q} icc={with_icc}");
                if with_icc {
                    assert!(back.icc_profile.is_some(), "q{q}: ICC must survive the EXIF pass");
                }
            }
        }
    }

    #[test]
    fn higher_method_is_not_larger() {
        // method 6 (max effort) should compress at least as well as method 0
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let m0 = encode(&decoded, 80, 0).unwrap();
        let m6 = encode(&decoded, 80, 6).unwrap();
        let _ = std::fs::remove_file(&m0.tmp_path);
        let _ = std::fs::remove_file(&m6.tmp_path);
        assert!(m6.bytes <= m0.bytes, "m6 {} should be <= m0 {}", m6.bytes, m0.bytes);
    }
}
