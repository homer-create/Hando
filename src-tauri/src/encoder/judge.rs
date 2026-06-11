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

/// Blockiness ratios at/above this flag a lossless container as hiding JPEG
/// history (rubric §1 second line of defense — "PNG that is really a JPEG").
///
/// Calibrated against the fixture set (see `blockiness_calibration_dump`):
/// clean sources measure ≤ 1.14, JPEG round-trips ≥ 1.39 (q60–q92), so 1.25
/// sits between with margin on both sides. Known limit: a camera-grade
/// (q≈95+) JPEG re-saved as PNG can stay under the bar — its artifacts are
/// near-invisible, so treating it as clean costs little.
pub const JPEG_BLOCKINESS_THRESHOLD: f64 = 1.25;

/// JPEG 8×8 grid fingerprint strength of the decoded pixels.
///
/// Returns a ratio: ~1.0 for clean sources (no phase of the 8-pixel lattice
/// stands out), rising the more the luma gradients concentrate on one lattice
/// phase — the signature JPEG block boundaries leave behind. Phase-agnostic,
/// so it survives cropping (the grid offset just moves to another phase).
pub fn jpeg_blockiness(img: &DecodedImage) -> f64 {
    let (w, h) = (img.width as usize, img.height as usize);
    // Need a few full lattice periods in both directions for the phase
    // statistics to mean anything.
    if w < 24 || h < 24 {
        return 1.0;
    }
    let luma: Vec<f32> = img
        .rgba
        .chunks_exact(4)
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            let r = p[0] as f32 * a + 255.0 * (1.0 - a);
            let g = p[1] as f32 * a + 255.0 * (1.0 - a);
            let b = p[2] as f32 * a + 255.0 * (1.0 - a);
            0.299 * r + 0.587 * g + 0.114 * b
        })
        .collect();

    // Only low-amplitude gradients enter the statistics: JPEG block-boundary
    // discontinuities are small steps in smooth regions, while real content
    // edges (text, UI borders — often themselves aligned to an 8px grid) are
    // large steps that would otherwise drown the signal in false positives.
    const AMPLITUDE_CAP: f32 = 12.0;
    let mut h_sum = [0f64; 8];
    let mut v_sum = [0f64; 8];
    let mut h_cnt = [0u64; 8];
    let mut v_cnt = [0u64; 8];
    for y in 1..h {
        let row = &luma[y * w..(y + 1) * w];
        let above = &luma[(y - 1) * w..y * w];
        for x in 1..w {
            let dh = (row[x] - row[x - 1]).abs();
            if dh < AMPLITUDE_CAP {
                h_sum[x % 8] += dh as f64;
                h_cnt[x % 8] += 1;
            }
        }
        let py = y % 8;
        for x in 0..w {
            let dv = (row[x] - above[x]).abs();
            if dv < AMPLITUDE_CAP {
                v_sum[py] += dv as f64;
                v_cnt[py] += 1;
            }
        }
    }
    phase_pop(&h_sum, &h_cnt).max(phase_pop(&v_sum, &v_cnt))
}

/// How much the strongest lattice phase stands out over the mean of the
/// other seven. The epsilon keeps flat images (zero gradients) at ~1.0.
fn phase_pop(sum: &[f64; 8], cnt: &[u64; 8]) -> f64 {
    const EPS: f64 = 0.05;
    let avgs: Vec<f64> = sum
        .iter()
        .zip(cnt)
        .map(|(s, c)| if *c == 0 { 0.0 } else { s / *c as f64 })
        .collect();
    let max = avgs.iter().cloned().fold(0.0f64, f64::max);
    let rest = (avgs.iter().sum::<f64>() - max) / 7.0;
    (max + EPS) / (rest + EPS)
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

    /// Round-trip a clean image through JPEG at `quality` — the pixels a
    /// "JPEG saved as PNG" would carry.
    fn jpeg_roundtrip(clean: &DecodedImage, quality: u32) -> DecodedImage {
        let enc = jpeg::encode(clean, quality, true).unwrap();
        let dec = decode(&enc.tmp_path, ImageExt::Jpeg).unwrap();
        let _ = std::fs::remove_file(&enc.tmp_path);
        dec
    }

    #[test]
    fn blockiness_flags_jpeg_history_and_passes_clean_sources() {
        for name in ["screenshot.png", "landscape.jpg"] {
            let ext = if name.ends_with(".png") { ImageExt::Png } else { ImageExt::Jpeg };
            let clean = decode(&fixture(name), ext).unwrap();
            let disguised = jpeg_roundtrip(&clean, 75);
            let b_clean = jpeg_blockiness(&clean);
            let b_disguised = jpeg_blockiness(&disguised);
            assert!(
                b_disguised >= JPEG_BLOCKINESS_THRESHOLD,
                "{name}: q75 jpeg history must be flagged, got {b_disguised:.3}"
            );
            // landscape.jpg itself carries jpeg history, so only assert the
            // truly clean source stays under the threshold
            if ext == ImageExt::Png {
                assert!(
                    b_clean < JPEG_BLOCKINESS_THRESHOLD,
                    "{name}: clean source must not be flagged, got {b_clean:.3}"
                );
            }
        }
    }

    #[test]
    fn blockiness_survives_cropping() {
        // Vendors crop images: the 8×8 grid shifts phase but must still be caught.
        let clean = decode(&fixture("screenshot.png"), ImageExt::Png).unwrap();
        let disguised = jpeg_roundtrip(&clean, 75);
        let (dx, dy) = (3u32, 5u32);
        let (w, h) = (disguised.width - dx, disguised.height - dy);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let row_start = (((y + dy) * disguised.width + dx) * 4) as usize;
            rgba.extend_from_slice(&disguised.rgba[row_start..row_start + (w * 4) as usize]);
        }
        let cropped = DecodedImage { rgba, width: w, height: h, icc_profile: None };
        let b = jpeg_blockiness(&cropped);
        assert!(
            b >= JPEG_BLOCKINESS_THRESHOLD,
            "cropped jpeg history must still be flagged, got {b:.3}"
        );
    }

    #[test]
    #[ignore = "calibration helper, run manually with --nocapture"]
    fn blockiness_calibration_dump() {
        for name in ["screenshot.png", "realphoto.png", "transparent.png"] {
            let img = decode(&fixture(name), ImageExt::Png).unwrap();
            println!("{name}: clean={:.3}", jpeg_blockiness(&img));
            for q in [60, 75, 85, 92] {
                let rt = jpeg_roundtrip(&img, q);
                println!("{name}: q{q}={:.3}", jpeg_blockiness(&rt));
            }
        }
        for name in ["landscape.jpg", "landscape2.jpg", "large_photo.jpg"] {
            let img = decode(&fixture(name), ImageExt::Jpeg).unwrap();
            println!("{name}: as-decoded={:.3}", jpeg_blockiness(&img));
        }
    }

    #[test]
    fn blockiness_is_neutral_on_degenerate_inputs() {
        // Too small for any 8×8 statistics — must not panic, must not flag.
        let tiny = decode(&fixture("tiny.png"), ImageExt::Png).unwrap();
        assert!(jpeg_blockiness(&tiny) < JPEG_BLOCKINESS_THRESHOLD);
        // Flat image: zero gradients everywhere, ratio must stay neutral.
        let flat = DecodedImage {
            rgba: vec![128; 64 * 64 * 4],
            width: 64,
            height: 64,
            icc_profile: None,
        };
        assert!(jpeg_blockiness(&flat) < JPEG_BLOCKINESS_THRESHOLD);
    }
}
