// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;
use serde::{Deserialize, Serialize};

pub mod auto;
pub mod decode;
pub mod event_sink;
pub mod icc;
pub mod judge;
pub mod jpeg;
pub mod metadata;
pub mod png;
pub mod webp;
pub mod avif;

/// Source/output format. Mirrors the strings the frontend sends in `CompressFile.ext`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageExt {
    Jpeg,
    Png,
    Webp,
    Avif,
}

impl FromStr for ImageExt {
    type Err = EncodeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().trim_start_matches('.') {
            "jpg" | "jpeg" => Ok(ImageExt::Jpeg),
            "png" => Ok(ImageExt::Png),
            "webp" => Ok(ImageExt::Webp),
            "avif" => Ok(ImageExt::Avif),
            other => Err(EncodeError::UnsupportedFormat(other.to_string())),
        }
    }
}

impl ImageExt {
    /// File extension (with leading dot) for output naming.
    pub fn dotted(&self) -> &'static str {
        match self {
            ImageExt::Jpeg => ".jpg",
            ImageExt::Png => ".png",
            ImageExt::Webp => ".webp",
            ImageExt::Avif => ".avif",
        }
    }
}

/// Settings forwarded from the frontend. Field names match TS via serde camelCase.
/// Effort knobs default to the previous hardcoded values so older frontends
/// (and stored settings) keep working unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeOpts {
    pub jpeg_quality: u32,
    pub png_quality: u32,
    pub webp_quality: u32,
    pub avif_quality: u32,
    pub emit_webp: bool,
    pub emit_avif: bool,
    /// ravif speed 1 (slowest/smallest) ..= 10 (fastest)
    #[serde(default = "default_avif_speed")]
    pub avif_speed: u8,
    /// oxipng preset 0 (fastest) ..= 6 (max effort)
    #[serde(default = "default_oxipng_level")]
    pub png_oxipng_level: u8,
    /// libwebp method 0 (fastest) ..= 6 (slowest/smallest)
    #[serde(default = "default_webp_method")]
    pub webp_method: u8,
    /// progressive scan script + optimized scans for JPEG output
    #[serde(default = "default_true")]
    pub jpeg_progressive: bool,
    /// `manual` = fixed quality numbers (legacy behavior); `auto` = search the
    /// smallest quality that still clears `target_quality` (rubric §8.6).
    /// Defaults to manual so older frontends/stored settings are unaffected.
    #[serde(default)]
    pub mode: EncodeMode,
    /// ssimulacra2 target `S` for auto mode. Presets in the UI: 90 visually
    /// lossless / 80 balanced / 70 aggressive (docs/calibration.md).
    #[serde(default = "default_target_quality")]
    pub target_quality: f64,
    /// Keep EXIF metadata (capture time, camera model, GPS, …) in the output.
    /// Off by default: stripping is the historical behavior and the privacy-
    /// safe choice. AVIF output can't carry it (ravif has no metadata API).
    #[serde(default)]
    pub keep_metadata: bool,
    /// Keep the ICC color profile. On by default — dropping it shifts colors
    /// on wide-gamut sources (rubric §0.5). AVIF output can't carry it either
    /// way (ravif is nclx-only).
    #[serde(default = "default_true")]
    pub keep_icc: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EncodeMode {
    #[default]
    Manual,
    Auto,
}

fn default_target_quality() -> f64 { 90.0 }
fn default_avif_speed() -> u8 { 8 }
// level 4 over the old 2: −8…−28% size on fixtures for ~2x time, still well
// inside the per-file budget (see docs/bench-results.md)
fn default_oxipng_level() -> u8 { 4 }
fn default_webp_method() -> u8 { 4 }
fn default_true() -> bool { true }

impl Default for EncodeOpts {
    /// Mirrors the frontend DEFAULT_SETTINGS in src/ui/settings.ts.
    fn default() -> Self {
        EncodeOpts {
            jpeg_quality: 75,
            png_quality: 75,
            webp_quality: 75,
            avif_quality: 50,
            emit_webp: false,
            emit_avif: false,
            avif_speed: default_avif_speed(),
            png_oxipng_level: default_oxipng_level(),
            webp_method: default_webp_method(),
            jpeg_progressive: true,
            mode: EncodeMode::Manual,
            target_quality: default_target_quality(),
            keep_metadata: false,
            keep_icc: true,
        }
    }
}

pub struct EncodeRequest<'a> {
    pub src_path: &'a Path,
    pub ext: ImageExt,
    pub opts: &'a EncodeOpts,
    pub progress_cb: Option<Box<dyn Fn(u8) + Send>>,
}

#[derive(Debug)]
pub struct EncodeResult {
    pub main: EncodedFile,
    pub companions: Vec<EncodedFile>,
    pub companion_errors: Vec<CompanionError>,
}

#[derive(Debug)]
pub struct EncodedFile {
    pub ext: ImageExt,
    pub tmp_path: PathBuf,
    pub bytes: u64,
}

#[derive(Debug)]
pub struct CompanionError {
    pub ext: ImageExt,
    pub msg: String,
}

#[derive(Debug)]
pub enum EncodeOutcome {
    Encoded(EncodeResult),
    SkippedNoGain { src_bytes: u64 },
}

#[derive(Debug, Error)]
pub enum EncodeError {
    #[error("decode failed: {0}")]
    Decode(String),
    #[error("encode failed: {0}")]
    Encode(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
}

/// Top-level encode entry point. Dispatches by `req.ext`.
///
/// CPU-bound; callers should run via `tokio::task::spawn_blocking`.
pub fn encode(req: EncodeRequest) -> Result<EncodeOutcome, EncodeError> {
    let progress = |pct: u8| {
        if let Some(cb) = &req.progress_cb { cb(pct); }
    };

    progress(5);
    let mut decoded = decode::decode(req.src_path, req.ext)?;
    // User opt-outs: clear what the encoders would otherwise re-embed
    if !req.opts.keep_icc {
        decoded.icc_profile = None;
    }
    if !req.opts.keep_metadata {
        decoded.exif = None;
    }
    progress(20);
    let src_bytes = std::fs::metadata(req.src_path)?.len();

    let auto_mode = req.opts.mode == EncodeMode::Auto;

    let mut main = if auto_mode {
        // The quality search is the slow part (several encode+judge rounds);
        // advance the bar per probe so it doesn't sit at 20% then jump to 75%
        let probes = std::cell::Cell::new(0u32);
        let on_probe = || {
            let i = probes.get() + 1;
            probes.set(i);
            progress(20 + (i * 8).min(52) as u8); // 28, 36, … capped at 72
        };
        match auto::encode_auto(req.src_path, req.ext, &decoded, req.opts, src_bytes, Some(&on_probe))? {
            Some(f) => f,
            // No candidate cleared the quality gates — nothing can be
            // improved within the target, which the UI reports as a skip.
            None => return Ok(EncodeOutcome::SkippedNoGain { src_bytes }),
        }
    } else {
        match req.ext {
            ImageExt::Jpeg => jpeg::encode(&decoded, req.opts.jpeg_quality, req.opts.jpeg_progressive)?,
            ImageExt::Png  => png::encode(&decoded, req.opts.png_quality, req.opts.png_oxipng_level)?,
            ImageExt::Webp => webp::encode(&decoded, req.opts.webp_quality, req.opts.webp_method)?,
            ImageExt::Avif => avif::encode(&decoded, req.opts.avif_quality, req.opts.avif_speed)?,
        }
    };
    progress(75);

    // Skip when the savings don't justify the cost (bar from `keep_bar`).
    if main.bytes * 100 >= src_bytes * keep_bar(main.ext, decoded.icc_profile.is_some()) {
        let _ = std::fs::remove_file(&main.tmp_path);
        // JPEG second chance (rubric §4 rule 1): when lossy re-encode has no
        // gain, the lossless DCT transcode often still shaves a few percent
        // with zero quality risk. Auto mode already tried the transcode as a
        // candidate, so this only applies to manual mode.
        match (!auto_mode).then(|| jpeg_lossless_fallback(req.src_path, req.ext, src_bytes, req.opts)).flatten() {
            Some(better) => main = better,
            None => return Ok(EncodeOutcome::SkippedNoGain { src_bytes }),
        }
    }

    let mut companions = Vec::new();
    let mut companion_errors = Vec::new();
    let companion_gate = auto::effective_gate(
        req.ext, req.src_path, src_bytes, &decoded, req.opts.target_quality,
    );

    // Companion searches advance 78→84 per probe (AVIF auto searches are the
    // slow tail; without this the bar parks at 78)
    let cprobes = std::cell::Cell::new(0u32);
    let on_cprobe = || {
        let i = cprobes.get() + 1;
        cprobes.set(i);
        progress(78 + i.min(6) as u8);
    };

    if req.opts.emit_webp && req.ext != ImageExt::Webp {
        progress(77);
        let result = if auto_mode {
            auto::encode_companion_auto(&decoded, ImageExt::Webp, companion_gate, req.opts, Some(&on_cprobe))
        } else {
            webp::encode(&decoded, req.opts.webp_quality, req.opts.webp_method)
        };
        match result {
            Ok(f)  => companions.push(f),
            Err(e) => companion_errors.push(CompanionError { ext: ImageExt::Webp, msg: e.to_string() }),
        }
    }
    if req.opts.emit_avif && req.ext != ImageExt::Avif {
        let result = if auto_mode {
            auto::encode_companion_auto(&decoded, ImageExt::Avif, companion_gate, req.opts, Some(&on_cprobe))
        } else {
            avif::encode(&decoded, req.opts.avif_quality, req.opts.avif_speed)
        };
        match result {
            Ok(f)  => companions.push(f),
            Err(e) => companion_errors.push(CompanionError { ext: ImageExt::Avif, msg: e.to_string() }),
        }
    }
    progress(85);

    Ok(EncodeOutcome::Encoded(EncodeResult { main, companions, companion_errors }))
}

/// Minimum-gain bar: the output must stay under this percentage of the source
/// or the file is skipped. Baseline: <2% gain is churn, not compression. AVIF
/// main output additionally drops the ICC profile (ravif is nclx-only, rubric
/// §0.5), so an ICC-tagged source must clear 10% before that loss is worth it
/// (docs/goal-learnings.md, with_icc.avif: 2.8% gain is not).
fn keep_bar(main_ext: ImageExt, has_icc: bool) -> u64 {
    if main_ext == ImageExt::Avif && has_icc { 90 } else { 98 }
}

/// Try the lossless JPEG transcode and keep it only if it clears the same 2%
/// gain bar. Skipped for EXIF-rotated sources: the transcode keeps pixels as
/// stored but strips the orientation tag, which would display wrong.
fn jpeg_lossless_fallback(src_path: &Path, ext: ImageExt, src_bytes: u64, opts: &EncodeOpts) -> Option<EncodedFile> {
    if ext != ImageExt::Jpeg {
        return None;
    }
    let bytes = std::fs::read(src_path).ok()?;
    if decode::read_exif_orientation(&bytes).unwrap_or(1) != 1 {
        return None;
    }
    let out = jpeg::optimize_lossless(&bytes, opts.keep_icc, opts.keep_metadata).ok()?;
    if out.bytes * 100 < src_bytes * 98 {
        Some(out)
    } else {
        let _ = std::fs::remove_file(&out.tmp_path);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{decode, webp};
    use std::path::PathBuf;

    #[test]
    fn parses_known_extensions() {
        assert_eq!(ImageExt::from_str("jpg").unwrap(), ImageExt::Jpeg);
        assert_eq!(ImageExt::from_str(".JPEG").unwrap(), ImageExt::Jpeg);
        assert_eq!(ImageExt::from_str("png").unwrap(), ImageExt::Png);
        assert_eq!(ImageExt::from_str("webp").unwrap(), ImageExt::Webp);
        assert_eq!(ImageExt::from_str("avif").unwrap(), ImageExt::Avif);
    }

    #[test]
    fn rejects_unknown_extension() {
        assert!(matches!(
            ImageExt::from_str("gif"),
            Err(EncodeError::UnsupportedFormat(_))
        ));
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn opts_no_companions() -> EncodeOpts {
        EncodeOpts {
            jpeg_quality: 80,
            png_quality: 80,
            webp_quality: 80,
            avif_quality: 60,
            emit_webp: false,
            emit_avif: false,
            ..EncodeOpts::default()
        }
    }

    #[allow(dead_code)]
    fn opts_with_companions() -> EncodeOpts {
        EncodeOpts {
            jpeg_quality: 80,
            png_quality: 80,
            webp_quality: 80,
            avif_quality: 60,
            emit_webp: true,
            emit_avif: true,
            ..EncodeOpts::default()
        }
    }

    #[test]
    fn encode_jpeg_returns_encoded_result() {
        let o = opts_no_companions();
        let outcome = encode(EncodeRequest {
            src_path: &fixture("landscape.jpg"),
            ext: ImageExt::Jpeg,
            opts: &o,
            progress_cb: None,
        }).unwrap();
        match outcome {
            EncodeOutcome::Encoded(r) => {
                assert_eq!(r.main.ext, ImageExt::Jpeg);
                assert!(r.companions.is_empty());
                assert!(r.companion_errors.is_empty());
            }
            EncodeOutcome::SkippedNoGain { .. } => panic!("should encode, not skip"),
        }
    }

    #[test]
    fn encode_with_webp_companion_produces_one_companion() {
        let o = EncodeOpts {
            emit_webp: true,
            emit_avif: false,
            ..opts_no_companions()
        };
        let outcome = encode(EncodeRequest {
            src_path: &fixture("landscape.jpg"),
            ext: ImageExt::Jpeg,
            opts: &o,
            progress_cb: None,
        }).unwrap();
        if let EncodeOutcome::Encoded(r) = outcome {
            assert_eq!(r.companions.len(), 1);
            assert_eq!(r.companions[0].ext, ImageExt::Webp);
        } else {
            panic!("expected Encoded");
        }
    }

    #[test]
    fn encode_tiny_already_optimized_skips() {
        let o = opts_no_companions();
        let outcome = encode(EncodeRequest {
            src_path: &fixture("tiny.png"),
            ext: ImageExt::Png,
            opts: &o,
            progress_cb: None,
        }).unwrap();
        assert!(matches!(outcome, EncodeOutcome::SkippedNoGain { .. }));
    }

    #[test]
    fn keep_bar_requires_10_percent_for_icc_avif() {
        // P0 §1.1: AVIF 主輸出會丟 ICC（ravif 只寫 nclx），帶 ICC 的來源
        // 要省滿 10% 才值得；其餘維持 2% 的反空轉 bar。
        assert_eq!(keep_bar(ImageExt::Avif, true), 90);
        assert_eq!(keep_bar(ImageExt::Avif, false), 98);
        assert_eq!(keep_bar(ImageExt::Jpeg, true), 98);
        assert_eq!(keep_bar(ImageExt::Png, true), 98);
    }

    #[test]
    fn avif_source_with_icc_and_small_gain_skips() {
        // P0 §1.1: AVIF 重壓必丟 ICC（ravif 只寫 nclx）。with_icc.avif 的
        // B2 門檻是 max(S,90)，贏家只省 ~3%（bench 實測 2.8%）——為這點
        // 省幅丟色彩描述檔不值得，必須 skip 保留原檔。
        let o = EncodeOpts {
            mode: EncodeMode::Auto,
            target_quality: 80.0,
            ..EncodeOpts::default()
        };
        let outcome = encode(EncodeRequest {
            src_path: &fixture("with_icc.avif"),
            ext: ImageExt::Avif,
            opts: &o,
            progress_cb: None,
        })
        .unwrap();
        assert!(
            matches!(outcome, EncodeOutcome::SkippedNoGain { .. }),
            "ICC-tagged AVIF with <10% gain must skip, got {outcome:?}"
        );
    }

    #[test]
    fn encode_corrupt_returns_decode_error() {
        let o = opts_no_companions();
        let result = encode(EncodeRequest {
            src_path: &fixture("corrupt.jpg"),
            ext: ImageExt::Jpeg,
            opts: &o,
            progress_cb: None,
        });
        assert!(matches!(result, Err(EncodeError::Decode(_))));
    }

    #[test]
    fn old_frontend_opts_json_still_deserializes_to_manual() {
        // A pre-knob frontend payload: no mode, no knobs, no target —
        // must come out as manual mode with the previous hardcoded behavior.
        let json = r#"{
            "jpegQuality": 75, "pngQuality": 75, "webpQuality": 75,
            "avifQuality": 50, "emitWebp": false, "emitAvif": false
        }"#;
        let opts: EncodeOpts = serde_json::from_str(json).unwrap();
        assert_eq!(opts.mode, EncodeMode::Manual);
        assert_eq!(opts.avif_speed, 8);
        assert_eq!(opts.png_oxipng_level, 4);
        assert_eq!(opts.webp_method, 4);
        assert!(opts.jpeg_progressive);
        assert_eq!(opts.target_quality, 90.0);
    }

    #[test]
    fn auto_mode_end_to_end_encodes_png() {
        let o = EncodeOpts {
            mode: EncodeMode::Auto,
            target_quality: 80.0,
            ..EncodeOpts::default()
        };
        let outcome = encode(EncodeRequest {
            src_path: &fixture("screenshot.png"),
            ext: ImageExt::Png,
            opts: &o,
            progress_cb: None,
        }).unwrap();
        match outcome {
            EncodeOutcome::Encoded(r) => {
                assert_eq!(r.main.ext, ImageExt::Png);
                let _ = std::fs::remove_file(&r.main.tmp_path);
            }
            EncodeOutcome::SkippedNoGain { .. } => panic!("screenshot.png should compress in auto mode"),
        }
    }

    #[test]
    fn jpeg_lossless_fallback_optimizes_baseline_jpeg() {
        // Write a baseline JPEG via the image crate (standard Huffman tables,
        // not optimized) — the DCT transcode should clear the 2% bar.
        let decoded = decode::decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let rgb: Vec<u8> = decoded.rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let tmp = tempfile::Builder::new().suffix(".jpg").tempfile().unwrap();
        let mut buf = Vec::new();
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85);
        image::ImageEncoder::write_image(
            enc, &rgb, decoded.width, decoded.height, image::ExtendedColorType::Rgb8,
        ).unwrap();
        std::fs::write(tmp.path(), &buf).unwrap();

        let out = jpeg_lossless_fallback(tmp.path(), ImageExt::Jpeg, buf.len() as u64, &EncodeOpts::default());
        let out = out.expect("baseline JPEG should gain ≥2% from lossless transcode");
        assert!(out.bytes < buf.len() as u64);
        // and pixels must be identical
        let before = decode::decode(tmp.path(), ImageExt::Jpeg).unwrap();
        let after = decode::decode(&out.tmp_path, ImageExt::Jpeg).unwrap();
        let _ = std::fs::remove_file(&out.tmp_path);
        assert!(judge::pixels_identical(&before, &after));
    }

    #[test]
    fn jpeg_lossless_fallback_skips_exif_rotated_source() {
        let path = fixture("portrait_exif_rotated.jpg");
        let src_bytes = std::fs::metadata(&path).unwrap().len();
        assert!(
            jpeg_lossless_fallback(&path, ImageExt::Jpeg, src_bytes, &EncodeOpts::default()).is_none(),
            "EXIF-rotated JPEG must not take the lossless transcode path"
        );
    }

    #[test]
    fn icc_tagged_jpeg_keeps_profile_in_main_and_webp_companion() {
        let o = EncodeOpts {
            jpeg_quality: 60, // well under the fixture's q90 so the lossy path wins
            emit_webp: true,
            ..opts_no_companions()
        };
        let outcome = encode(EncodeRequest {
            src_path: &fixture("with_icc.jpg"),
            ext: ImageExt::Jpeg,
            opts: &o,
            progress_cb: None,
        }).unwrap();
        let EncodeOutcome::Encoded(r) = outcome else { panic!("should encode, not skip") };

        let expected = icc::test_profile(3000);
        let main = decode::decode(&r.main.tmp_path, ImageExt::Jpeg).unwrap();
        let companion = &r.companions[0];
        let webp_out = decode::decode(&companion.tmp_path, ImageExt::Webp).unwrap();
        let _ = std::fs::remove_file(&r.main.tmp_path);
        let _ = std::fs::remove_file(&companion.tmp_path);

        assert_eq!(main.icc_profile.as_deref(), Some(expected.as_slice()), "main JPEG");
        assert_eq!(companion.ext, ImageExt::Webp);
        assert_eq!(webp_out.icc_profile.as_deref(), Some(expected.as_slice()), "WebP companion");
    }

    #[test]
    fn webp_source_does_not_emit_webp_companion() {
        // Encode landscape.jpg → WebP first, then re-encode that WebP with emit_webp=true
        // The companion should NOT be emitted (no duplicate-format companions)
        let decoded = decode::decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let webp_encoded = webp::encode(&decoded, 80, 4).unwrap();

        let o = EncodeOpts { emit_webp: true, ..opts_no_companions() };
        let outcome = encode(EncodeRequest {
            src_path: &webp_encoded.tmp_path,
            ext: ImageExt::Webp,
            opts: &o,
            progress_cb: None,
        });

        // Clean up temp file
        let _ = std::fs::remove_file(&webp_encoded.tmp_path);

        let outcome = outcome.unwrap();
        if let EncodeOutcome::Encoded(r) = outcome {
            assert_eq!(r.companions.len(), 0, "WebP source should not emit WebP companion");
        }
        // SkippedNoGain is also acceptable (if re-compressing WebP doesn't save bytes)
    }
}
