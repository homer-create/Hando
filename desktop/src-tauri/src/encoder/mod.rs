// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;
use serde::{Deserialize, Serialize};

pub mod decode;
pub mod event_sink;
pub mod jpeg;
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeOpts {
    pub jpeg_quality: u32,
    pub png_quality: u32,
    pub webp_quality: u32,
    pub avif_quality: u32,
    pub emit_webp: bool,
    pub emit_avif: bool,
}

pub struct EncodeRequest<'a> {
    pub src_path: &'a Path,
    pub ext: ImageExt,
    pub opts: &'a EncodeOpts,
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
    let decoded = decode::decode(req.src_path, req.ext)?;
    let src_bytes = std::fs::metadata(req.src_path)?.len();

    let main = match req.ext {
        ImageExt::Jpeg => jpeg::encode(&decoded, req.opts.jpeg_quality)?,
        ImageExt::Png => png::encode(&decoded, req.opts.png_quality)?,
        ImageExt::Webp => webp::encode(&decoded, req.opts.webp_quality)?,
        ImageExt::Avif => avif::encode(&decoded, req.opts.avif_quality)?,
    };

    // Skip if savings are less than 2% of the source — prevents endless re-compression
    // of already-optimized files where the encoder finds only marginal improvements.
    if main.bytes * 100 >= src_bytes * 98 {
        return Ok(EncodeOutcome::SkippedNoGain { src_bytes });
    }

    let mut companions = Vec::new();
    let mut companion_errors = Vec::new();

    if req.opts.emit_webp && req.ext != ImageExt::Webp {
        match webp::encode(&decoded, req.opts.webp_quality) {
            Ok(f) => companions.push(f),
            Err(e) => companion_errors.push(CompanionError { ext: ImageExt::Webp, msg: e.to_string() }),
        }
    }
    if req.opts.emit_avif && req.ext != ImageExt::Avif {
        match avif::encode(&decoded, req.opts.avif_quality) {
            Ok(f) => companions.push(f),
            Err(e) => companion_errors.push(CompanionError { ext: ImageExt::Avif, msg: e.to_string() }),
        }
    }

    Ok(EncodeOutcome::Encoded(EncodeResult { main, companions, companion_errors }))
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
        }
    }

    #[test]
    fn encode_jpeg_returns_encoded_result() {
        let o = opts_no_companions();
        let outcome = encode(EncodeRequest {
            src_path: &fixture("landscape.jpg"),
            ext: ImageExt::Jpeg,
            opts: &o,
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
        }).unwrap();
        assert!(matches!(outcome, EncodeOutcome::SkippedNoGain { .. }));
    }

    #[test]
    fn encode_corrupt_returns_decode_error() {
        let o = opts_no_companions();
        let result = encode(EncodeRequest {
            src_path: &fixture("corrupt.jpg"),
            ext: ImageExt::Jpeg,
            opts: &o,
        });
        assert!(matches!(result, Err(EncodeError::Decode(_))));
    }

    #[test]
    fn webp_source_does_not_emit_webp_companion() {
        // Encode landscape.jpg → WebP first, then re-encode that WebP with emit_webp=true
        // The companion should NOT be emitted (no duplicate-format companions)
        let decoded = decode::decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let webp_encoded = webp::encode(&decoded, 80).unwrap();

        let o = EncodeOpts { emit_webp: true, ..opts_no_companions() };
        let outcome = encode(EncodeRequest {
            src_path: &webp_encoded.tmp_path,
            ext: ImageExt::Webp,
            opts: &o,
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
