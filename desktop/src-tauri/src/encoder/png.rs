// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{decode::DecodedImage, EncodeError, EncodedFile, ImageExt};
use image::ImageEncoder;
use std::io::Write;
use tempfile::NamedTempFile;

pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    let q = quality.clamp(1, 100);

    let png_bytes = if q < 100 {
        quantize_to_png(decoded, q as u8)?
    } else {
        encode_truecolor_png(decoded)?
    };

    let opts = oxipng::Options::from_preset(2);
    let optimized = oxipng::optimize_from_memory(&png_bytes, &opts)
        .map_err(|e| EncodeError::Encode(format!("oxipng: {e}")))?;

    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(&optimized)?;
    tmp.flush()?;
    let len = optimized.len() as u64;
    let (_file, path) = tmp.keep().map_err(|e| EncodeError::Io(e.error))?;

    Ok(EncodedFile { ext: ImageExt::Png, tmp_path: path, bytes: len })
}

fn quantize_to_png(decoded: &DecodedImage, quality: u8) -> Result<Vec<u8>, EncodeError> {
    let mut attr = imagequant::Attributes::new();
    attr.set_quality(0, quality)
        .map_err(|e| EncodeError::Encode(format!("imagequant quality: {e}")))?;

    let pixels: Vec<imagequant::RGBA> = decoded.rgba.chunks_exact(4)
        .map(|p| imagequant::RGBA::new(p[0], p[1], p[2], p[3]))
        .collect();

    let mut img = attr.new_image(pixels.as_slice(), decoded.width as usize, decoded.height as usize, 0.0)
        .map_err(|e| EncodeError::Encode(format!("imagequant new_image: {e}")))?;
    let mut res = attr.quantize(&mut img)
        .map_err(|e| EncodeError::Encode(format!("imagequant quantize: {e}")))?;
    res.set_dithering_level(1.0).ok();
    let (palette, indices) = res.remapped(&mut img)
        .map_err(|e| EncodeError::Encode(format!("imagequant remap: {e}")))?;

    // Reconstruct RGBA from quantized palette
    let mut rgba: Vec<u8> = Vec::with_capacity(decoded.rgba.len());
    for &i in &indices {
        let c = palette[i as usize];
        rgba.extend_from_slice(&[c.r, c.g, c.b, c.a]);
    }

    encode_rgba_to_png_bytes(&rgba, decoded.width, decoded.height)
}

fn encode_truecolor_png(decoded: &DecodedImage) -> Result<Vec<u8>, EncodeError> {
    encode_rgba_to_png_bytes(&decoded.rgba, decoded.width, decoded.height)
}

fn encode_rgba_to_png_bytes(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, EncodeError> {
    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        &mut buf,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::Adaptive,
    );
    encoder.write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| EncodeError::Encode(format!("png encode: {e}")))?;
    Ok(buf)
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
    fn encodes_png_smaller_than_input_via_quantization() {
        let decoded = decode(&fixture("screenshot.png"), ImageExt::Png).unwrap();
        let src_bytes = std::fs::metadata(fixture("screenshot.png")).unwrap().len();
        let out = encode(&decoded, 80).unwrap();
        assert!(out.bytes < src_bytes, "out {} should be < src {}", out.bytes, src_bytes);
    }

    #[test]
    fn quality_100_lossless_mode_still_produces_valid_png() {
        let decoded = decode(&fixture("transparent.png"), ImageExt::Png).unwrap();
        let out = encode(&decoded, 100).unwrap();
        assert!(out.tmp_path.exists());
        assert!(out.bytes > 0);
    }

    #[test]
    fn preserves_alpha_channel() {
        let decoded = decode(&fixture("transparent.png"), ImageExt::Png).unwrap();
        let out = encode(&decoded, 80).unwrap();
        let re = decode(&out.tmp_path, ImageExt::Png).unwrap();
        let has_transparency = re.rgba.chunks_exact(4).any(|p| p[3] < 255);
        assert!(has_transparency, "alpha channel should survive encode");
    }
}
