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

pub struct EncodeResult {
    pub main: EncodedFile,
    pub companions: Vec<EncodedFile>,
    pub companion_errors: Vec<CompanionError>,
}

pub struct EncodedFile {
    pub ext: ImageExt,
    pub tmp_path: PathBuf,
    pub bytes: u64,
}

pub struct CompanionError {
    pub ext: ImageExt,
    pub msg: String,
}

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

    if main.bytes >= src_bytes {
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
}
