// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{EncodeError, ImageExt};
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
        ImageExt::Png | ImageExt::Webp | ImageExt::Avif => decode_via_image_crate(src),
    }
}

fn decode_jpeg(src: &Path) -> Result<DecodedImage, EncodeError> {
    let bytes = std::fs::read(src)?;
    let orientation = read_exif_orientation(&bytes).unwrap_or(1);
    let icc = None; // ICC passthrough deferred to future version

    let d = mozjpeg::Decompress::new_mem(&bytes)
        .map_err(|e| EncodeError::Decode(format!("mozjpeg open: {e}")))?;
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
    let dyn_img = reader.decode()
        .map_err(|e| EncodeError::Decode(format!("decode: {e}")))?;
    let rgba_img = dyn_img.to_rgba8();
    let (width, height) = rgba_img.dimensions();
    Ok(DecodedImage {
        rgba: rgba_img.into_raw(),
        width,
        height,
        icc_profile: None,
    })
}

fn read_exif_orientation(jpeg_bytes: &[u8]) -> Option<u32> {
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
    fn returns_error_on_corrupt_input() {
        let result = decode(&fixture("corrupt.jpg"), ImageExt::Jpeg);
        assert!(matches!(result, Err(EncodeError::Decode(_))));
    }
}
