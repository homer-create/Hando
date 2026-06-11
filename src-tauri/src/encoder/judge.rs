// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The "judge": perceptual quality measurement for the optimization rubric
//! (docs/rubric.md). Scores a candidate against the baseline image
//! (基準圖 = decoded original with orientation applied, metadata stripped).

use super::{decode::DecodedImage, EncodeError};
use ssimulacra2::{compute_frame_ssimulacra2, ColorPrimaries, Rgb, TransferCharacteristic};

/// SSIMULACRA2 score of `distorted` vs `reference`, both as RGBA buffers.
///
/// Scale (per the ssimulacra2 spec): 100 = identical, 90+ ≈ visually
/// lossless, 70 ≈ high quality, 50 ≈ medium. Can go negative for wrecked
/// images.
///
/// Alpha is composited over white before scoring — the metric itself has no
/// alpha support, and both images get the same treatment so transparency
/// differences still lower the score.
pub fn ssimulacra2_score(
    reference: &DecodedImage,
    distorted: &DecodedImage,
) -> Result<f64, EncodeError> {
    if reference.width != distorted.width || reference.height != distorted.height {
        return Err(EncodeError::Encode(format!(
            "judge: dimension mismatch {}x{} vs {}x{}",
            reference.width, reference.height, distorted.width, distorted.height
        )));
    }
    let src = rgba_to_srgb_frame(&reference.rgba, reference.width, reference.height)?;
    let dst = rgba_to_srgb_frame(&distorted.rgba, distorted.width, distorted.height)?;
    compute_frame_ssimulacra2(src, dst)
        .map_err(|e| EncodeError::Encode(format!("ssimulacra2: {e}")))
}

/// True iff both buffers decode to exactly the same pixels (lossless gate of
/// rubric §2: diff count = 0 against the baseline image).
pub fn pixels_identical(a: &DecodedImage, b: &DecodedImage) -> bool {
    a.width == b.width && a.height == b.height && a.rgba == b.rgba
}

/// Bits per pixel of the on-disk file — the cheap §1 input-gate signal.
pub fn bits_per_pixel(file_bytes: u64, width: u32, height: u32) -> f64 {
    if width == 0 || height == 0 {
        return 0.0;
    }
    (file_bytes as f64 * 8.0) / (width as f64 * height as f64)
}

fn rgba_to_srgb_frame(rgba: &[u8], width: u32, height: u32) -> Result<Rgb, EncodeError> {
    let data: Vec<[f32; 3]> = rgba
        .chunks_exact(4)
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            // Composite over white in sRGB space (what viewers effectively show)
            [
                (p[0] as f32 / 255.0) * a + (1.0 - a),
                (p[1] as f32 / 255.0) * a + (1.0 - a),
                (p[2] as f32 / 255.0) * a + (1.0 - a),
            ]
        })
        .collect();
    Rgb::new(
        data,
        width as usize,
        height as usize,
        TransferCharacteristic::SRGB,
        ColorPrimaries::BT709,
    )
    .map_err(|e| EncodeError::Encode(format!("judge rgb frame: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::decode::decode;
    use crate::encoder::{jpeg, ImageExt};
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
    }

    #[test]
    fn identical_images_score_near_100() {
        let img = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let score = ssimulacra2_score(&img, &img).unwrap();
        assert!(score > 99.0, "identical should be ~100, got {score}");
    }

    #[test]
    fn lower_quality_scores_lower() {
        let img = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let hi = jpeg::encode(&img, 90, true).unwrap();
        let lo = jpeg::encode(&img, 20, true).unwrap();
        let hi_dec = decode(&hi.tmp_path, ImageExt::Jpeg).unwrap();
        let lo_dec = decode(&lo.tmp_path, ImageExt::Jpeg).unwrap();
        let hi_score = ssimulacra2_score(&img, &hi_dec).unwrap();
        let lo_score = ssimulacra2_score(&img, &lo_dec).unwrap();
        let _ = std::fs::remove_file(&hi.tmp_path);
        let _ = std::fs::remove_file(&lo.tmp_path);
        assert!(
            hi_score > lo_score,
            "q90 ({hi_score}) should outscore q20 ({lo_score})"
        );
        assert!(hi_score > 70.0, "q90 should be high quality, got {hi_score}");
    }

    #[test]
    fn dimension_mismatch_is_error() {
        let a = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let b = decode(&fixture("screenshot.png"), ImageExt::Png).unwrap();
        assert!(ssimulacra2_score(&a, &b).is_err());
    }

    #[test]
    fn pixels_identical_detects_equality_and_difference() {
        let img = decode(&fixture("screenshot.png"), ImageExt::Png).unwrap();
        assert!(pixels_identical(&img, &img));
        let mut tweaked_rgba = img.rgba.clone();
        tweaked_rgba[0] = tweaked_rgba[0].wrapping_add(1);
        let tweaked = DecodedImage {
            rgba: tweaked_rgba,
            width: img.width,
            height: img.height,
            icc_profile: None,
        };
        assert!(!pixels_identical(&img, &tweaked));
    }

    #[test]
    fn bpp_computes_expected_value() {
        // 1000 bytes over 100x100 px = 8000 bits / 10000 px = 0.8 bpp
        assert!((bits_per_pixel(1000, 100, 100) - 0.8).abs() < 1e-9);
    }
}
