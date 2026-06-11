// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Synthetic test fixture generator. Run with `cargo run --example generate-fixtures --features generate-fixtures`.
// Outputs to `tests/fixtures/` relative to the Cargo manifest.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn main() {
    let dir = out_dir();
    fs::create_dir_all(&dir).expect("create fixtures dir");

    gen_screenshot(&dir);
    gen_transparent(&dir);
    gen_tiny(&dir);
    gen_corrupt(&dir);
    gen_landscape(&dir);
    gen_portrait_exif_rotated(&dir);
    gen_large_photo(&dir);
    gen_with_icc(&dir);

    eprintln!("All fixtures generated in {}", dir.display());
    eprintln!("Note: large_photo.jpg is gitignored (regenerate on demand).");
    eprintln!("Note: with_icc.avif is macOS-generated (checked in). Source must be small —");
    eprintln!("      sips tiles larger images into grid AVIFs, which avif-decode rejects:");
    eprintln!("  sips -s format avif --embedProfile '/System/Library/ColorSync/Profiles/Display P3.icc' with_icc.png --out with_icc.avif");
}

/// Deterministic, structurally valid ICC blob (size field + `acsp` at offset
/// 36, zero tags, patterned tail). Mirrors `encoder::icc::test_profile`.
fn synthetic_icc(len: usize) -> Vec<u8> {
    assert!(len >= 132);
    let mut p = vec![0u8; len];
    p[..4].copy_from_slice(&(len as u32).to_be_bytes());
    p[36..40].copy_from_slice(b"acsp");
    for (i, b) in p[132..].iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    p
}

fn gen_with_icc(dir: &PathBuf) {
    let icc = synthetic_icc(3000);
    let (w, h) = (320u32, 240u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            rgba[i] = (x * 255 / w) as u8;
            rgba[i + 1] = (y * 255 / h) as u8;
            rgba[i + 2] = 90;
            rgba[i + 3] = 255;
        }
    }

    // with_icc.jpg — mozjpeg APP2 markers
    let mut compress = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    compress.set_size(w as usize, h as usize);
    compress.set_quality(90.0);
    let mut comp = compress.start_compress(Vec::new()).expect("mozjpeg start");
    // Spec-compliant APP2 ICC marker (1-based seq/count) — the mozjpeg
    // crate's write_icc_profile numbers chunks 0-based, which compliant
    // readers reject; mirrors encoder::icc::jpeg_app2_segments
    let mut app2 = b"ICC_PROFILE\0\x01\x01".to_vec();
    app2.extend_from_slice(&icc);
    comp.write_marker(mozjpeg::Marker::APP(2), &app2);
    let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    comp.write_scanlines(&rgb).expect("mozjpeg scanlines");
    let jpeg_bytes = comp.finish().expect("mozjpeg finish");
    fs::write(dir.join("with_icc.jpg"), &jpeg_bytes).expect("with_icc.jpg");
    eprintln!("  with_icc.jpg (synthetic 3000-byte ICC in APP2)");

    // with_icc.png — iCCP chunk
    let mut png_bytes = Vec::new();
    {
        use image::ImageEncoder;
        let mut encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        encoder.set_icc_profile(icc.clone()).expect("png icc");
        encoder
            .write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
            .expect("png encode");
    }
    fs::write(dir.join("with_icc.png"), &png_bytes).expect("with_icc.png");
    eprintln!("  with_icc.png (synthetic 3000-byte ICC in iCCP)");
}

fn gen_screenshot(dir: &PathBuf) {
    let (w, h) = (1440u32, 900u32);
    let mut buf = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 3) as usize;
            let band = (y / 100) as u8;
            buf[i]     = match band % 4 { 0 => 240, 1 => 90,  2 => 30,  _ => 200 };
            buf[i + 1] = match band % 4 { 0 => 240, 1 => 160, 2 => 30,  _ => 60  };
            buf[i + 2] = match band % 4 { 0 => 240, 1 => 220, 2 => 30,  _ => 60  };
        }
    }
    image::save_buffer(dir.join("screenshot.png"), &buf, w, h, image::ColorType::Rgb8)
        .expect("screenshot.png");
    eprintln!("  screenshot.png");
}

fn gen_transparent(dir: &PathBuf) {
    let (w, h) = (512u32, 512u32);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            buf[i]     = (x * 255 / w) as u8;
            buf[i + 1] = (y * 255 / h) as u8;
            buf[i + 2] = 128;
            buf[i + 3] = ((x + y) * 255 / (w + h)) as u8;
        }
    }
    image::save_buffer(dir.join("transparent.png"), &buf, w, h, image::ColorType::Rgba8)
        .expect("transparent.png");
    eprintln!("  transparent.png");
}

fn gen_tiny(dir: &PathBuf) {
    // Solid gray 128x128 — run through imagequant + oxipng so it's already at the
    // floor our encoder can achieve (SkippedNoGain on re-encode).
    let (w, h) = (128u32, 128u32);
    let rgba: Vec<u8> = vec![128u8, 128u8, 128u8, 255u8].into_iter()
        .cycle()
        .take((w * h * 4) as usize)
        .collect();

    // Palette-quantize via imagequant (same as our PNG encoder)
    let mut attr = imagequant::Attributes::new();
    attr.set_quality(0, 80).expect("imagequant quality");
    let pixels: Vec<imagequant::RGBA> = rgba.chunks_exact(4)
        .map(|p| imagequant::RGBA::new(p[0], p[1], p[2], p[3]))
        .collect();
    let mut img = attr.new_image(pixels.as_slice(), w as usize, h as usize, 0.0)
        .expect("imagequant new_image");
    let mut res = attr.quantize(&mut img).expect("imagequant quantize");
    res.set_dithering_level(1.0).ok();
    let (palette, indices) = res.remapped(&mut img).expect("imagequant remap");

    // Reconstruct RGBA from palette
    let mut out_rgba: Vec<u8> = Vec::with_capacity((w * h * 4) as usize);
    for &i in &indices {
        let c = palette[i as usize];
        out_rgba.extend_from_slice(&[c.r, c.g, c.b, c.a]);
    }

    // Encode to PNG
    let mut png_bytes = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = image::codecs::png::PngEncoder::new_with_quality(
            &mut png_bytes,
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        );
        encoder.write_image(&out_rgba, w, h, image::ExtendedColorType::Rgba8)
            .expect("png encode");
    }

    // Optimize with oxipng
    let opts = oxipng::Options::from_preset(2);
    let optimized = oxipng::optimize_from_memory(&png_bytes, &opts)
        .expect("oxipng optimize");

    fs::write(dir.join("tiny.png"), &optimized).expect("tiny.png");
    eprintln!("  tiny.png");
}

fn gen_corrupt(dir: &PathBuf) {
    let path = dir.join("corrupt.jpg");
    let mut f = fs::File::create(&path).expect("corrupt.jpg");
    // Start with valid JPEG SOI marker (FF D8) so mozjpeg begins parsing,
    // then immediately truncate with garbage so it fails gracefully via longjmp.
    // Without the SOI, mozjpeg may call exit() before Rust can catch the error.
    let mut bytes = vec![0xFF, 0xD8]; // JPEG SOI
    // Add garbage that is definitely not a valid JPEG marker sequence
    bytes.extend_from_slice(&[0x00u8; 98]);
    f.write_all(&bytes).expect("write corrupt.jpg");
    eprintln!("  corrupt.jpg");
}

fn gen_landscape(dir: &PathBuf) {
    // 1920x1080 synthetic photo-like gradient (landscape orientation)
    let (w, h) = (1920u32, 1080u32);
    let mut buf = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 3) as usize;
            // Sky-to-ground gradient
            let sky_factor = 1.0 - (y as f32 / h as f32);
            let horizon = (x as f32 / w as f32) * 30.0;
            buf[i]     = (30.0  + sky_factor * 140.0 + horizon) as u8;
            buf[i + 1] = (60.0  + sky_factor * 100.0) as u8;
            buf[i + 2] = (80.0  + sky_factor * 175.0) as u8;
        }
    }
    // Save as JPEG so it's already a compressed format
    let img = image::RgbImage::from_raw(w, h, buf).expect("create landscape buffer");
    img.save_with_format(dir.join("landscape.jpg"), image::ImageFormat::Jpeg)
        .expect("landscape.jpg");
    eprintln!("  landscape.jpg");
}

fn gen_portrait_exif_rotated(dir: &PathBuf) {
    // Create an 800x600 image (landscape pixel layout) but with EXIF orientation=6
    // which means "rotate 90° CW to display upright" => displayed dimensions are 600x800 (portrait)
    let (w, h) = (800u32, 600u32);
    let mut buf = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 3) as usize;
            // Simple gradient that makes rotation visible
            buf[i]     = (x * 200 / w) as u8;
            buf[i + 1] = (y * 200 / h) as u8;
            buf[i + 2] = 128;
        }
    }

    // Encode to JPEG bytes first (in-memory)
    let img = image::RgbImage::from_raw(w, h, buf).expect("create portrait buffer");
    let mut jpeg_bytes = Vec::new();
    {
        let mut cursor = std::io::Cursor::new(&mut jpeg_bytes);
        img.write_to(&mut cursor, image::ImageFormat::Jpeg).expect("encode portrait");
    }

    // Inject EXIF orientation=6 using a minimal EXIF APP1 marker
    // JPEG structure: FF D8 (SOI), then we inject APP1, then the rest
    let exif_payload = build_minimal_exif_orientation_6();
    let mut output = Vec::new();
    // Copy SOI (first 2 bytes: FF D8)
    output.extend_from_slice(&jpeg_bytes[0..2]);
    // Inject APP1 marker (FF E1) + length (2 + exif_payload.len()) + payload
    let app1_len = (exif_payload.len() + 2) as u16;
    output.push(0xFF);
    output.push(0xE1);
    output.push((app1_len >> 8) as u8);
    output.push((app1_len & 0xFF) as u8);
    output.extend_from_slice(&exif_payload);
    // Append rest of JPEG (skip original SOI which we already copied)
    output.extend_from_slice(&jpeg_bytes[2..]);

    fs::write(dir.join("portrait_exif_rotated.jpg"), &output)
        .expect("portrait_exif_rotated.jpg");
    eprintln!("  portrait_exif_rotated.jpg (800x600 pixels, EXIF orientation=6, displays as 600x800)");
}

/// Build a minimal EXIF APP1 payload that sets Orientation = 6.
/// Format: "Exif\0\0" + TIFF header + IFD with Orientation tag
fn build_minimal_exif_orientation_6() -> Vec<u8> {
    let mut data = Vec::new();

    // EXIF header identifier
    data.extend_from_slice(b"Exif\x00\x00");

    // TIFF header (little-endian: "II" + magic 42 + offset 8 = start of IFD)
    let tiff_start = data.len();
    data.extend_from_slice(b"II"); // little-endian
    data.extend_from_slice(&42u16.to_le_bytes()); // magic
    data.extend_from_slice(&8u32.to_le_bytes());  // offset to IFD0 (relative to TIFF header start)

    // IFD0: 1 entry
    data.extend_from_slice(&1u16.to_le_bytes()); // entry count

    // Orientation tag: tag=0x0112, type=3 (SHORT), count=1, value=6
    data.extend_from_slice(&0x0112u16.to_le_bytes()); // tag
    data.extend_from_slice(&3u16.to_le_bytes());       // type SHORT
    data.extend_from_slice(&1u32.to_le_bytes());       // count
    data.extend_from_slice(&6u16.to_le_bytes());       // value: orientation 6 = rotate 90° CW
    data.extend_from_slice(&0u16.to_le_bytes());       // padding to 4 bytes

    // Next IFD offset = 0 (no more IFDs)
    data.extend_from_slice(&0u32.to_le_bytes());

    let _ = tiff_start; // used for documentation
    data
}

fn gen_large_photo(dir: &PathBuf) {
    // 6000x4000 noise image for memory pressure testing — gitignored
    let path = dir.join("large_photo.jpg");
    if path.exists() {
        eprintln!("  large_photo.jpg (already exists, skipping)");
        return;
    }
    let (w, h) = (6000u32, 4000u32);
    let mut buf = vec![0u8; (w * h * 3) as usize];
    // Simple pseudo-random fill using LCG
    let mut state = 0xCAFEu32;
    for byte in buf.iter_mut() {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        *byte = (state >> 16) as u8;
    }
    let img = image::RgbImage::from_raw(w, h, buf).expect("large_photo buffer");
    img.save_with_format(&path, image::ImageFormat::Jpeg).expect("large_photo.jpg");
    eprintln!("  large_photo.jpg (6000x4000, gitignored)");
}
