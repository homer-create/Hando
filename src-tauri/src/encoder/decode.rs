// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{icc, EncodeError, ImageExt};
use image::ImageDecoder;
use std::io::BufReader;
use std::path::Path;

pub struct DecodedImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub icc_profile: Option<Vec<u8>>,
}

pub fn decode(src: &Path, ext: ImageExt) -> Result<DecodedImage, EncodeError> {
    match ext {
        ImageExt::Jpeg => decode_jpeg(src),
        ImageExt::Png | ImageExt::Webp => decode_via_image_crate(src),
        // The image crate's `avif` feature is encode-only; decoding goes
        // through avif-decode (bundled libaom), otherwise AVIF inputs fail
        // at runtime with "format not supported".
        ImageExt::Avif => decode_avif(src),
    }
}

fn decode_avif(src: &Path) -> Result<DecodedImage, EncodeError> {
    let bytes = std::fs::read(src)?;
    let decoder = avif_decode::Decoder::from_avif(&bytes)
        .map_err(|e| EncodeError::Decode(format!("avif open: {e}")))?;
    let image = decoder.to_image()
        .map_err(|e| EncodeError::Decode(format!("avif decode: {e}")))?;

    // libaom may hand back 8- or 16-bit, with or without alpha; normalize to RGBA8
    let to8 = |v: u16| (v >> 8) as u8;
    let (rgba, width, height): (Vec<u8>, usize, usize) = match image {
        avif_decode::Image::Rgb8(img) => {
            let (buf, w, h) = img.into_contiguous_buf();
            (buf.iter().flat_map(|p| [p.r, p.g, p.b, 255]).collect(), w, h)
        }
        avif_decode::Image::Rgba8(img) => {
            let (buf, w, h) = img.into_contiguous_buf();
            (buf.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect(), w, h)
        }
        avif_decode::Image::Rgb16(img) => {
            let (buf, w, h) = img.into_contiguous_buf();
            (buf.iter().flat_map(|p| [to8(p.r), to8(p.g), to8(p.b), 255]).collect(), w, h)
        }
        avif_decode::Image::Rgba16(img) => {
            let (buf, w, h) = img.into_contiguous_buf();
            (buf.iter().flat_map(|p| [to8(p.r), to8(p.g), to8(p.b), to8(p.a)]).collect(), w, h)
        }
        avif_decode::Image::Gray8(img) => {
            let (buf, w, h) = img.into_contiguous_buf();
            (buf.iter().flat_map(|p| { let v = p.0; [v, v, v, 255] }).collect(), w, h)
        }
        avif_decode::Image::Gray16(img) => {
            let (buf, w, h) = img.into_contiguous_buf();
            (buf.iter().flat_map(|p| { let v = to8(p.0); [v, v, v, 255] }).collect(), w, h)
        }
    };

    Ok(DecodedImage {
        rgba,
        width: width as u32,
        height: height as u32,
        // avif-decode exposes no ICC API; pull the colr/prof box straight
        // from the container so wide-gamut AVIFs keep their profile
        icc_profile: icc::extract_avif_icc(&bytes),
    })
}

fn decode_jpeg(src: &Path) -> Result<DecodedImage, EncodeError> {
    let bytes = std::fs::read(src)?;
    let orientation = read_exif_orientation(&bytes).unwrap_or(1);

    let d = mozjpeg::Decompress::with_markers(&[mozjpeg::Marker::APP(2)])
        .from_mem(&bytes)
        .map_err(|e| EncodeError::Decode(format!("mozjpeg open: {e}")))?;
    // Reassemble the ICC profile from APP2 markers before `rgba()` consumes
    // the decompressor (marker data borrows from it)
    let icc = icc::assemble_jpeg_app2_icc(
        d.markers()
            .filter(|m| m.marker == mozjpeg::Marker::APP(2))
            .map(|m| m.data),
    );
    let mut d = d.rgba()
        .map_err(|e| EncodeError::Decode(format!("mozjpeg rgba: {e}")))?;
    let width = d.width() as u32;
    let height = d.height() as u32;
    let pixels: Vec<[u8; 4]> = d.read_scanlines()
        .map_err(|e| EncodeError::Decode(format!("mozjpeg read: {e}")))?;
    d.finish().ok();

    let rgba: Vec<u8> = pixels.into_iter().flat_map(|p| p.into_iter()).collect();
    let mut img = DecodedImage { rgba, width, height, icc_profile: icc };
    apply_orientation(&mut img, orientation);
    Ok(img)
}

fn decode_via_image_crate(src: &Path) -> Result<DecodedImage, EncodeError> {
    let reader = image::ImageReader::open(src)
        .map_err(|e| EncodeError::Decode(format!("open: {e}")))?
        .with_guessed_format()
        .map_err(|e| EncodeError::Decode(format!("guess format: {e}")))?;
    let mut decoder = reader.into_decoder()
        .map_err(|e| EncodeError::Decode(format!("decode: {e}")))?;
    // PNG iCCP / WebP ICCP chunk; the png crate hands it back already inflated
    let icc = decoder.icc_profile().ok().flatten().filter(|p| !p.is_empty());
    let dyn_img = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| EncodeError::Decode(format!("decode: {e}")))?;
    let rgba_img = dyn_img.to_rgba8();
    let (width, height) = rgba_img.dimensions();
    Ok(DecodedImage {
        rgba: rgba_img.into_raw(),
        width,
        height,
        icc_profile: icc,
    })
}

pub(crate) fn read_exif_orientation(jpeg_bytes: &[u8]) -> Option<u32> {
    let exif = exif::Reader::new()
        .read_from_container(&mut BufReader::new(std::io::Cursor::new(jpeg_bytes)))
        .ok()?;
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    field.value.get_uint(0)
}

/// Rotate/flip the RGBA buffer in-place to compensate for EXIF orientation.
/// See: https://exiftool.org/TagNames/EXIF.html#Orientation
fn apply_orientation(img: &mut DecodedImage, orientation: u32) {
    if orientation <= 1 {
        return;
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let src = std::mem::take(&mut img.rgba);

    // Helper: copy one pixel from src[sx,sy] to dst[dx,dy]
    let put = |dst: &mut Vec<u8>, dx: usize, dy: usize, dw: usize,
               src_buf: &[u8], sx: usize, sy: usize, sw: usize| {
        let s = (sy * sw + sx) * 4;
        let d = (dy * dw + dx) * 4;
        dst[d..d + 4].copy_from_slice(&src_buf[s..s + 4]);
    };

    match orientation {
        2 => {
            // Mirror horizontal
            let mut dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, w - 1 - x, y, w, &src, x, y, w); } }
            img.rgba = dst;
        }
        3 => {
            // Rotate 180°
            let mut dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, w - 1 - x, h - 1 - y, w, &src, x, y, w); } }
            img.rgba = dst;
        }
        4 => {
            // Mirror vertical
            let mut dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, x, h - 1 - y, w, &src, x, y, w); } }
            img.rgba = dst;
        }
        5 => {
            // Transpose (mirror horizontal + rotate 270 CW)
            img.width = h as u32;
            img.height = w as u32;
            let mut dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, y, x, h, &src, x, y, w); } }
            img.rgba = dst;
        }
        6 => {
            // Rotate 90° CW
            img.width = h as u32;
            img.height = w as u32;
            let mut dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, h - 1 - y, x, h, &src, x, y, w); } }
            img.rgba = dst;
        }
        7 => {
            // Mirror horizontal + rotate 90° CW
            img.width = h as u32;
            img.height = w as u32;
            let mut dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, h - 1 - y, w - 1 - x, h, &src, x, y, w); } }
            img.rgba = dst;
        }
        8 => {
            // Rotate 270° CW (= 90° CCW)
            img.width = h as u32;
            img.height = w as u32;
            let mut dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, y, w - 1 - x, h, &src, x, y, w); } }
            img.rgba = dst;
        }
        _ => {
            img.rgba = src;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn decodes_png_to_rgba() {
        let img = decode(&fixture("transparent.png"), ImageExt::Png).unwrap();
        assert!(img.width > 0 && img.height > 0);
        assert_eq!(img.rgba.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn decodes_jpeg_to_rgba() {
        let img = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        assert!(img.width > 0 && img.height > 0);
        assert_eq!(img.rgba.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn applies_exif_orientation_6_to_pixels() {
        // orientation=6 means 90° CW rotation — stored as landscape (wide), should decode as portrait (tall)
        let img = decode(&fixture("portrait_exif_rotated.jpg"), ImageExt::Jpeg).unwrap();
        // After applying orientation the image should be taller than wide
        assert!(img.height > img.width, "expected portrait after EXIF rotation, got {}x{}", img.width, img.height);
    }

    #[test]
    fn extracts_icc_from_jpeg_png_avif_fixtures() {
        let expected = crate::encoder::icc::test_profile(3000);
        let jpg = decode(&fixture("with_icc.jpg"), ImageExt::Jpeg).unwrap();
        assert_eq!(jpg.icc_profile.as_deref(), Some(expected.as_slice()), "JPEG APP2");
        let png = decode(&fixture("with_icc.png"), ImageExt::Png).unwrap();
        assert_eq!(png.icc_profile.as_deref(), Some(expected.as_slice()), "PNG iCCP");
        // Real-world colr/prof box: Display P3 profile embedded by macOS sips
        let avif = decode(&fixture("with_icc.avif"), ImageExt::Avif).unwrap();
        let icc = avif.icc_profile.expect("AVIF fixture must yield its ICC profile");
        assert_eq!(&icc[36..40], b"acsp", "extracted bytes must be an ICC profile");
    }

    #[test]
    fn untagged_sources_have_no_icc() {
        let jpg = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        assert!(jpg.icc_profile.is_none());
        let png = decode(&fixture("screenshot.png"), ImageExt::Png).unwrap();
        assert!(png.icc_profile.is_none());
    }

    #[test]
    fn returns_error_on_corrupt_input() {
        let result = decode(&fixture("corrupt.jpg"), ImageExt::Jpeg);
        assert!(matches!(result, Err(EncodeError::Decode(_))));
    }

    #[test]
    fn decodes_avif_roundtrip() {
        // Regression: the image crate's avif feature is encode-only, so AVIF
        // inputs used to fail with "format not supported" at runtime.
        let img = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let out = crate::encoder::avif::encode(&img, 60, 8).unwrap();
        let back = decode(&out.tmp_path, ImageExt::Avif);
        let _ = std::fs::remove_file(&out.tmp_path);
        let back = back.unwrap();
        assert_eq!((back.width, back.height), (img.width, img.height));
        assert_eq!(back.rgba.len(), img.rgba.len());
    }
}
