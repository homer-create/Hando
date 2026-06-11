// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Auto quality mode — quality-targeted encoding (rubric §8.6).
//!
//! Instead of a fixed quality number, binary-search the smallest quality whose
//! ssimulacra2 score against the baseline image still clears the target `S`.
//! Lossless and lossy candidates compete; smallest passing file wins (rubric §5).

use super::{
    avif,
    decode::{self, DecodedImage},
    jpeg, judge, png, webp, EncodeError, EncodeOpts, EncodedFile, ImageExt,
};
use std::path::Path;

/// Generation-loss bound when re-encoding an already-lossy, heavily-compressed
/// source: the output must be visually identical to the input (rubric §4).
const B_CLASS_MIN_TARGET: f64 = 90.0;

/// At/above this bits-per-pixel a lossy source is treated as near-clean (B1):
/// camera JPEGs sit around 2–5 bpp, web-grade re-compressions below ~1.
const B1_BPP_THRESHOLD: f64 = 1.0;

/// At/above this bits-per-pixel a lossy re-encode is futile (2026-06-11 goal
/// loop, large_photo bpp≈10): ssimulacra2 punishes grain removal so hard that
/// no quality clears any gate (q85 scored 16.1 vs target 80) while each probe
/// on such poorly-compressed pixels costs seconds. Camera JPEGs sit at
/// 2–5 bpp, so they stay on the normal B1 path.
const LOSSY_FUTILE_BPP: f64 = 8.0;

/// Pixel count at/above which the large-image knob guards kick in
/// (2MP ≈ 1080p, docs/goal-learnings.md).
const LARGE_IMAGE_PIXELS: u64 = 2_000_000;

fn is_large(decoded: &DecodedImage) -> bool {
    decoded.width as u64 * decoded.height as u64 >= LARGE_IMAGE_PIXELS
}

/// AVIF speed for auto-mode searches: 2MP+ sources start at speed 9 — half
/// the encode time for ~6% size (realphoto avif@85: 3063ms → 1579ms). Never
/// slows down a faster user-chosen speed.
fn auto_avif_speed(decoded: &DecodedImage, opts: &EncodeOpts) -> u8 {
    if is_large(decoded) { opts.avif_speed.max(9) } else { opts.avif_speed }
}

/// oxipng level for auto-mode candidates: photo-grade 2MP+ PNGs take >3s per
/// probe at level 4 (realphoto: 3694ms) and the quality search multiplies
/// that — cap at 2. Never raises a lighter user-chosen level.
fn auto_oxipng_level(decoded: &DecodedImage, opts: &EncodeOpts) -> u8 {
    if is_large(decoded) { opts.png_oxipng_level.min(2) } else { opts.png_oxipng_level }
}

/// Quality search granularity. Searching on a lattice of step 4 keeps the
/// binary search to ~5 encode+judge rounds; differences under 4 quality
/// points are visually negligible.
const QUALITY_STEP: u32 = 4;

/// Effective quality gate for a candidate derived from this source
/// (rubric §1 input gate + §4 generation-loss rules).
pub fn effective_gate(
    src_ext: ImageExt,
    src_path: &Path,
    src_bytes: u64,
    decoded: &DecodedImage,
    target: f64,
) -> f64 {
    let lossy_source = match src_ext {
        ImageExt::Png => false,
        ImageExt::Jpeg | ImageExt::Avif => true,
        ImageExt::Webp => !webp_is_lossless(src_path).unwrap_or(false),
    };
    if !lossy_source {
        // Second line of defense (rubric §1): a lossless container whose
        // pixels carry the JPEG 8×8 grid fingerprint is a re-saved lossy
        // image — its "clean baseline" is already dirty, so bound generation
        // loss exactly like a B-class source.
        if judge::jpeg_blockiness(decoded) >= judge::JPEG_BLOCKINESS_THRESHOLD {
            return target.max(B_CLASS_MIN_TARGET);
        }
        return target;
    }
    let bpp = judge::bits_per_pixel(src_bytes, decoded.width, decoded.height);
    if bpp >= B1_BPP_THRESHOLD {
        target // B1: near-clean source, the preset target applies as-is
    } else {
        target.max(B_CLASS_MIN_TARGET) // B2: bound generation loss
    }
}

/// Sniff the WebP container for the VP8L (lossless) chunk.
fn webp_is_lossless(src: &Path) -> Option<bool> {
    let mut head = [0u8; 64];
    use std::io::Read;
    let mut f = std::fs::File::open(src).ok()?;
    let n = f.read(&mut head).ok()?;
    let head = &head[..n];
    if head.len() < 16 || &head[0..4] != b"RIFF" || &head[8..12] != b"WEBP" {
        return None;
    }
    Some(head.windows(4).any(|w| w == b"VP8L"))
}

/// Auto-mode main output. Returns `None` when no candidate clears the gates —
/// the caller maps that to `SkippedNoGain` (nothing could be improved within
/// the quality budget).
pub fn encode_auto(
    src_path: &Path,
    ext: ImageExt,
    decoded: &DecodedImage,
    opts: &EncodeOpts,
    src_bytes: u64,
) -> Result<Option<EncodedFile>, EncodeError> {
    let target = opts.target_quality;
    let gate = effective_gate(ext, src_path, src_bytes, decoded, target);

    match ext {
        ImageExt::Png => {
            // A-class: lossless and quantized candidates compete (rubric §5)
            let level = auto_oxipng_level(decoded, opts);
            let lossless = png::encode(decoded, 100, level).ok();
            let quantized = search_min_quality(decoded, gate, ImageExt::Png, |q| {
                png::encode(decoded, q, level)
            });
            Ok(pick_smaller(lossless, quantized))
        }
        ImageExt::Webp => {
            let lossless = webp::encode(decoded, 100, opts.webp_method).ok();
            let lossy = search_min_quality(decoded, gate, ImageExt::Webp, |q| {
                webp::encode(decoded, q, opts.webp_method)
            });
            Ok(pick_smaller(lossless, lossy))
        }
        ImageExt::Jpeg => {
            let bytes = std::fs::read(src_path)?;
            let upright = decode::read_exif_orientation(&bytes).unwrap_or(1) == 1;
            let transcoded = if upright { jpeg::optimize_lossless(&bytes).ok() } else { None };

            let bpp = judge::bits_per_pixel(src_bytes, decoded.width, decoded.height);
            let lossy = if bpp >= LOSSY_FUTILE_BPP {
                // Grain-heavy, barely-compressed source: every lossy probe
                // fails the gate and burns seconds — transcode only. For a
                // rotated source this leaves no candidate at all; the caller
                // reports SkippedNoGain and the original is preserved.
                None
            } else if bpp >= B1_BPP_THRESHOLD || !upright {
                // B1 (near-clean) — or rotated B2 where the transcode is
                // unavailable and a tightly-gated re-encode is the only option
                search_min_quality(decoded, gate, ImageExt::Jpeg, |q| {
                    jpeg::encode(decoded, q, opts.jpeg_progressive)
                })
            } else {
                // B2 upright: lossless transcode only (rubric §4 rule 1) —
                // re-encoding an already heavily-compressed JPEG stacks
                // generation loss for little gain
                None
            };
            Ok(pick_smaller(transcoded, lossy))
        }
        ImageExt::Avif => {
            // No lossless transcode path exists for AVIF; tightly-gated re-encode
            let speed = auto_avif_speed(decoded, opts);
            Ok(search_min_quality(decoded, gate, ImageExt::Avif, |q| {
                avif::encode(decoded, q, speed)
            }))
        }
    }
}

/// Auto-mode companion output (WebP/AVIF alongside the main file), gated the
/// same way as the main candidate.
pub fn encode_companion_auto(
    decoded: &DecodedImage,
    companion_ext: ImageExt,
    gate: f64,
    opts: &EncodeOpts,
) -> Result<EncodedFile, EncodeError> {
    let found = match companion_ext {
        ImageExt::Webp => search_min_quality(decoded, gate, ImageExt::Webp, |q| {
            webp::encode(decoded, q, opts.webp_method)
        }),
        ImageExt::Avif => {
            let speed = auto_avif_speed(decoded, opts);
            search_min_quality(decoded, gate, ImageExt::Avif, |q| {
                avif::encode(decoded, q, speed)
            })
        }
        other => {
            return Err(EncodeError::Encode(format!(
                "unsupported auto companion format {other:?}"
            )))
        }
    };
    found.ok_or_else(|| {
        EncodeError::Encode(format!(
            "no {companion_ext:?} quality clears the target {gate:.0}"
        ))
    })
}

/// Binary-search the lowest quality (on a step-{QUALITY_STEP} lattice in
/// 1..=99) whose decoded output scores ≥ `target` vs the baseline.
/// Returns the smallest passing candidate, or `None` if even the top quality
/// fails the gate. All non-winning temp files are cleaned up.
fn search_min_quality(
    baseline: &DecodedImage,
    target: f64,
    ext: ImageExt,
    encode_at: impl Fn(u32) -> Result<EncodedFile, EncodeError>,
) -> Option<EncodedFile> {
    let ladder: Vec<u32> = (1..=99).step_by(QUALITY_STEP as usize).collect();
    let (mut lo, mut hi) = (0usize, ladder.len() - 1);
    let mut best: Option<EncodedFile> = None;

    while lo <= hi {
        let mid = (lo + hi) / 2;
        let q = ladder[mid];
        let Ok(out) = encode_at(q) else {
            // encoder error at this quality — treat as fail, search upward
            if mid == ladder.len() - 1 { break; }
            lo = mid + 1;
            continue;
        };
        let passes = decode::decode(&out.tmp_path, ext)
            .ok()
            .map(|dec| {
                judge::pixels_identical(baseline, &dec)
                    || judge::ssimulacra2_score(baseline, &dec).unwrap_or(f64::NEG_INFINITY)
                        >= target
            })
            .unwrap_or(false);

        if passes {
            if let Some(old) = best.replace(out) {
                let _ = std::fs::remove_file(&old.tmp_path);
            }
            if mid == 0 { break; }
            hi = mid - 1;
        } else {
            let _ = std::fs::remove_file(&out.tmp_path);
            if mid == ladder.len() - 1 { break; }
            lo = mid + 1;
        }
    }
    best
}

/// Keep the smaller candidate, delete the loser's temp file.
fn pick_smaller(a: Option<EncodedFile>, b: Option<EncodedFile>) -> Option<EncodedFile> {
    match (a, b) {
        (Some(x), Some(y)) => {
            let (winner, loser) = if x.bytes <= y.bytes { (x, y) } else { (y, x) };
            let _ = std::fs::remove_file(&loser.tmp_path);
            Some(winner)
        }
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::decode::decode;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
    }

    fn opts() -> EncodeOpts {
        EncodeOpts::default()
    }

    #[test]
    fn auto_png_result_clears_target() {
        let path = fixture("screenshot.png");
        let baseline = decode(&path, ImageExt::Png).unwrap();
        let src_bytes = std::fs::metadata(&path).unwrap().len();
        let o = EncodeOpts { target_quality: 80.0, ..opts() };

        let out = encode_auto(&path, ImageExt::Png, &baseline, &o, src_bytes)
            .unwrap()
            .expect("screenshot should have a passing candidate");
        let dec = decode(&out.tmp_path, ImageExt::Png).unwrap();
        let ok = judge::pixels_identical(&baseline, &dec)
            || judge::ssimulacra2_score(&baseline, &dec).unwrap() >= 80.0;
        let _ = std::fs::remove_file(&out.tmp_path);
        assert!(ok, "winning candidate must clear the gate");
    }

    #[test]
    fn auto_low_bpp_jpeg_takes_lossless_transcode_only() {
        // landscape.jpg is synthetic and very low bpp → B2: the only candidate
        // is the lossless transcode, so pixels must be identical to baseline.
        let path = fixture("landscape.jpg");
        let baseline = decode(&path, ImageExt::Jpeg).unwrap();
        let src_bytes = std::fs::metadata(&path).unwrap().len();
        let bpp = judge::bits_per_pixel(src_bytes, baseline.width, baseline.height);
        assert!(bpp < B1_BPP_THRESHOLD, "fixture assumption: low bpp, got {bpp}");

        let out = encode_auto(&path, ImageExt::Jpeg, &baseline, &opts(), src_bytes)
            .unwrap()
            .expect("lossless transcode should be available");
        let dec = decode(&out.tmp_path, ImageExt::Jpeg).unwrap();
        let identical = judge::pixels_identical(&baseline, &dec);
        let _ = std::fs::remove_file(&out.tmp_path);
        assert!(identical, "B2 JPEG must come from the lossless transcode");
    }

    #[test]
    fn auto_b1_jpeg_searches_lossy_and_clears_gate() {
        // landscape2.jpg：真實相機 JPEG，bpp ≈ 2.6 → B1，preset 門檻直接適用，
        // 有損搜尋必須找到過門檻的候選。
        let path = fixture("landscape2.jpg");
        let baseline = decode(&path, ImageExt::Jpeg).unwrap();
        let src_bytes = std::fs::metadata(&path).unwrap().len();
        let bpp = judge::bits_per_pixel(src_bytes, baseline.width, baseline.height);
        assert!(
            (B1_BPP_THRESHOLD..LOSSY_FUTILE_BPP).contains(&bpp),
            "fixture assumption: B1-range bpp, got {bpp}"
        );

        let o = EncodeOpts { target_quality: 70.0, ..opts() };
        let out = encode_auto(&path, ImageExt::Jpeg, &baseline, &o, src_bytes)
            .unwrap()
            .expect("B1 camera JPEG should find a passing candidate");
        let dec = decode(&out.tmp_path, ImageExt::Jpeg).unwrap();
        let identical = judge::pixels_identical(&baseline, &dec);
        let score = judge::ssimulacra2_score(&baseline, &dec).unwrap();
        let _ = std::fs::remove_file(&out.tmp_path);
        assert!(identical || score >= 70.0, "winner must clear the gate, got {score:.2}");
    }

    #[test]
    fn auto_futile_bpp_jpeg_skips_lossy_search_and_transcodes() {
        // 顆粒/雜訊型超高 bpp 來源：有損候選全滅（large_photo q85 只得 16.1 分）
        // 且每發數秒——唯一出路是無損 DCT 轉碼，輸出像素必須與基準圖相同。
        // 合成 q100 噪點 JPEG 重現 large_photo 的 bpp 級別（fixture 是 gitignored）。
        let (w, h) = (256u32, 256u32);
        let mut state = 0x12345678u32;
        let rgba: Vec<u8> = (0..w * h * 4)
            .map(|i| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                if i % 4 == 3 { 255 } else { (state >> 24) as u8 }
            })
            .collect();
        let noise = DecodedImage { rgba, width: w, height: h, icc_profile: None };
        let q100 = jpeg::encode(&noise, 100, true).unwrap();
        let tmp = tempfile::Builder::new().suffix(".jpg").tempfile().unwrap();
        std::fs::copy(&q100.tmp_path, tmp.path()).unwrap();
        let _ = std::fs::remove_file(&q100.tmp_path);

        let baseline = decode(tmp.path(), ImageExt::Jpeg).unwrap();
        let src_bytes = std::fs::metadata(tmp.path()).unwrap().len();
        let bpp = judge::bits_per_pixel(src_bytes, w, h);
        assert!(bpp >= LOSSY_FUTILE_BPP, "q100 noise should be futile-high bpp, got {bpp}");

        let out = encode_auto(tmp.path(), ImageExt::Jpeg, &baseline, &opts(), src_bytes)
            .unwrap()
            .expect("lossless transcode should be available");
        let dec = decode(&out.tmp_path, ImageExt::Jpeg).unwrap();
        let identical = judge::pixels_identical(&baseline, &dec);
        let _ = std::fs::remove_file(&out.tmp_path);
        assert!(identical, "futile-bpp JPEG must come from the lossless transcode");
    }

    #[test]
    fn large_image_knob_guards() {
        // 2026-06-11 goal loop：2MP+ 來源 avif speed <9 / oxipng >2 會把單發
        // 編碼時間撐破預算（搜尋還要乘上發數）；小圖維持使用者旋鈕。
        let dims = |w: u32, h: u32| DecodedImage {
            rgba: Vec::new(),
            width: w,
            height: h,
            icc_profile: None,
        };
        let small = dims(320, 240);
        let large = dims(1920, 1080); // 2,073,600 px ≥ 2MP
        let o = opts(); // avif_speed 8, png_oxipng_level 4

        assert_eq!(auto_avif_speed(&small, &o), 8, "small keeps user speed");
        assert_eq!(auto_avif_speed(&large, &o), 9, "large floors at speed 9");
        let fast = EncodeOpts { avif_speed: 10, ..opts() };
        assert_eq!(auto_avif_speed(&large, &fast), 10, "never slows a faster user choice");

        assert_eq!(auto_oxipng_level(&small, &o), 4, "small keeps user level");
        assert_eq!(auto_oxipng_level(&large, &o), 2, "large caps at level 2");
        let light = EncodeOpts { png_oxipng_level: 1, ..opts() };
        assert_eq!(auto_oxipng_level(&large, &light), 1, "never raises a lighter user choice");
    }

    #[test]
    fn gate_is_raised_for_disguised_lossy_png() {
        // A JPEG re-saved as PNG: lossless container, lossy pixels. The
        // blockiness gate must demote it to the B-class bound (rubric §1
        // second line of defense).
        let clean = decode(&fixture("screenshot.png"), ImageExt::Png).unwrap();
        let enc = jpeg::encode(&clean, 75, true).unwrap();
        let disguised = decode(&enc.tmp_path, ImageExt::Jpeg).unwrap();
        let _ = std::fs::remove_file(&enc.tmp_path);

        let gate = effective_gate(ImageExt::Png, Path::new("vendor.png"), 999_999, &disguised, 70.0);
        assert_eq!(gate, B_CLASS_MIN_TARGET, "disguised PNG must use the B-class bound");

        let clean_gate = effective_gate(ImageExt::Png, Path::new("clean.png"), 999_999, &clean, 70.0);
        assert_eq!(clean_gate, 70.0, "genuinely clean PNG keeps the preset target");
    }

    #[test]
    fn gate_is_raised_for_low_bpp_lossy_sources() {
        let path = fixture("landscape.jpg");
        let baseline = decode(&path, ImageExt::Jpeg).unwrap();
        let src_bytes = std::fs::metadata(&path).unwrap().len();
        let gate = effective_gate(ImageExt::Jpeg, &path, src_bytes, &baseline, 70.0);
        assert_eq!(gate, B_CLASS_MIN_TARGET, "low-bpp lossy source must use the bound");
        // NB: the PNG control must use genuinely clean pixels — passing the
        // decoded JPEG here would (correctly) trip the blockiness gate.
        let png_path = fixture("screenshot.png");
        let png_baseline = decode(&png_path, ImageExt::Png).unwrap();
        let png_gate = effective_gate(ImageExt::Png, &png_path, 999, &png_baseline, 70.0);
        assert_eq!(png_gate, 70.0, "lossless sources keep the preset target");
    }
}
