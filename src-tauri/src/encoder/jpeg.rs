// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{decode::DecodedImage, EncodeError, EncodedFile, ImageExt};
use std::io::Write;
use tempfile::NamedTempFile;

pub fn encode(decoded: &DecodedImage, quality: u32, progressive: bool) -> Result<EncodedFile, EncodeError> {
    let q = quality.clamp(1, 100) as f32;

    let mut compress = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    compress.set_size(decoded.width as usize, decoded.height as usize);
    if progressive {
        compress.set_progressive_mode();
        compress.set_optimize_scans(true);
    } else {
        // Baseline profile (libjpeg-turbo equivalent): faster encode, slightly
        // larger files. mozjpeg defaults to progressive, so this is the only
        // way to actually turn it off.
        compress.set_fastest_defaults();
    }
    // set_fastest_defaults() resets compression params — quality must be set after
    compress.set_quality(q);

    let mut comp = compress.start_compress(Vec::new())
        .map_err(|e| EncodeError::Encode(format!("mozjpeg start: {e}")))?;

    // ICC passthrough: re-embed the source profile as APP2 markers so
    // wide-gamut colors survive. Spec-compliant 1-based chunk numbering via
    // our own splitter — see `icc::jpeg_app2_segments` for why not the
    // mozjpeg crate's `write_icc_profile`.
    if let Some(icc) = decoded.icc_profile.as_deref().filter(|p| !p.is_empty()) {
        for segment in super::icc::jpeg_app2_segments(icc) {
            comp.write_marker(mozjpeg::Marker::APP(2), &segment);
        }
    }

    // JPEG doesn't support alpha — convert RGBA → RGB by dropping alpha
    let rgb: Vec<u8> = decoded.rgba.chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();

    comp.write_scanlines(&rgb)
        .map_err(|e| EncodeError::Encode(format!("mozjpeg scanlines: {e}")))?;
    let bytes = comp.finish()
        .map_err(|e| EncodeError::Encode(format!("mozjpeg finish: {e}")))?;

    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(&bytes)?;
    tmp.flush()?;
    let len = bytes.len() as u64;
    let (_file, path) = tmp.keep().map_err(|e| EncodeError::Io(e.error))?;

    Ok(EncodedFile { ext: ImageExt::Jpeg, tmp_path: path, bytes: len })
}

/// Lossless JPEG transcode — `jpegtran -optimize -progressive` equivalent
/// (rubric §4 rule 1). Operates on DCT coefficients: pixels are untouched, so
/// there is zero quality risk. Rebuilds optimized Huffman tables and a
/// progressive scan script. ICC (APP2) markers are copied through; all other
/// metadata is stripped.
///
/// Callers must check EXIF orientation first: this path keeps pixels exactly
/// as stored, and stripping the orientation tag without rotating would make
/// the image display wrong. Skip it when orientation ≠ 1.
pub fn optimize_lossless(src_bytes: &[u8]) -> Result<EncodedFile, EncodeError> {
    let bytes = std::panic::catch_unwind(|| unsafe { transcode_coefficients(src_bytes) })
        .map_err(|p| {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "unknown libjpeg error".into());
            EncodeError::Encode(format!("jpeg lossless transcode: {msg}"))
        })?;

    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(&bytes)?;
    tmp.flush()?;
    let len = bytes.len() as u64;
    let (_file, path) = tmp.keep().map_err(|e| EncodeError::Io(e.error))?;
    Ok(EncodedFile { ext: ImageExt::Jpeg, tmp_path: path, bytes: len })
}

#[cold]
extern "C-unwind" fn unwind_error_exit(cinfo: &mut mozjpeg_sys::jpeg_common_struct) {
    let code = unsafe { cinfo.err.as_ref().map(|e| e.msg_code).unwrap_or(-1) };
    std::panic::resume_unwind(Box::new(format!("libjpeg error code {code}")));
}

#[cold]
extern "C-unwind" fn silence_message(
    _cinfo: &mut mozjpeg_sys::jpeg_common_struct,
    _level: std::os::raw::c_int,
) {
}

unsafe fn make_error_mgr() -> Box<mozjpeg_sys::jpeg_error_mgr> {
    let mut err: Box<mozjpeg_sys::jpeg_error_mgr> = Box::new(std::mem::zeroed());
    mozjpeg_sys::jpeg_std_error(&mut err);
    err.error_exit = Some(unwind_error_exit);
    err.emit_message = Some(silence_message);
    err
}

unsafe fn transcode_coefficients(src: &[u8]) -> Vec<u8> {
    use mozjpeg_sys::*;
    use std::os::raw::{c_int, c_ulong, c_void};

    const JPEG_APP2: c_int = 0xE2;

    let mut src_err = make_error_mgr();
    let mut srcinfo: jpeg_decompress_struct = std::mem::zeroed();
    srcinfo.common.err = &mut *src_err;
    jpeg_create_decompress(&mut srcinfo);

    jpeg_mem_src(&mut srcinfo, src.as_ptr(), src.len() as c_ulong);
    // Keep ICC profile chunks so colors survive the transcode
    jpeg_save_markers(&mut srcinfo, JPEG_APP2, 0xFFFF);
    jpeg_read_header(&mut srcinfo, 1);
    let coefficients = jpeg_read_coefficients(&mut srcinfo);

    let mut dst_err = make_error_mgr();
    let mut dstinfo: jpeg_compress_struct = std::mem::zeroed();
    dstinfo.common.err = &mut *dst_err;
    jpeg_create_compress(&mut dstinfo);

    let mut outbuf: *mut u8 = std::ptr::null_mut();
    let mut outsize: c_ulong = 0;
    jpeg_mem_dest(&mut dstinfo, &mut outbuf, &mut outsize);

    jpeg_copy_critical_parameters(&srcinfo, &mut dstinfo);
    dstinfo.optimize_coding = 1;
    jpeg_simple_progression(&mut dstinfo);
    jpeg_write_coefficients(&mut dstinfo, coefficients);

    // Replay saved APP2 (ICC) markers into the output
    let mut marker = srcinfo.marker_list;
    while !marker.is_null() {
        jpeg_write_marker(
            &mut dstinfo,
            (*marker).marker as c_int,
            (*marker).data,
            (*marker).data_length,
        );
        marker = (*marker).next;
    }

    jpeg_finish_compress(&mut dstinfo);
    jpeg_destroy_compress(&mut dstinfo);
    jpeg_finish_decompress(&mut srcinfo);
    jpeg_destroy_decompress(&mut srcinfo);

    let out = std::slice::from_raw_parts(outbuf, outsize as usize).to_vec();
    libc::free(outbuf as *mut c_void);
    out
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
    fn encodes_jpeg_smaller_than_input_at_q80() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let src_bytes = std::fs::metadata(fixture("landscape.jpg")).unwrap().len();
        let out = encode(&decoded, 80, true).unwrap();
        assert!(out.bytes < src_bytes, "out {} should be < src {}", out.bytes, src_bytes);
        assert!(out.tmp_path.exists());
    }

    #[test]
    fn higher_quality_produces_larger_file() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let q40 = encode(&decoded, 40, true).unwrap();
        let q90 = encode(&decoded, 90, true).unwrap();
        assert!(q90.bytes > q40.bytes);
    }

    #[test]
    fn output_extension_is_jpeg() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let out = encode(&decoded, 80, true).unwrap();
        assert_eq!(out.ext, ImageExt::Jpeg);
    }

    #[test]
    fn lossless_transcode_keeps_pixels_bit_identical() {
        // The whole point of rubric §4 rule 1: DCT coefficients untouched →
        // decoded pixels must match the source exactly.
        let src = fixture("landscape.jpg");
        let bytes = std::fs::read(&src).unwrap();
        let out = optimize_lossless(&bytes).unwrap();
        let before = decode(&src, ImageExt::Jpeg).unwrap();
        let after = decode(&out.tmp_path, ImageExt::Jpeg).unwrap();
        let _ = std::fs::remove_file(&out.tmp_path);
        assert_eq!((before.width, before.height), (after.width, after.height));
        assert_eq!(before.rgba, after.rgba, "lossless transcode must not change pixels");
    }

    #[test]
    fn lossless_transcode_shrinks_unoptimized_jpeg() {
        // Fixture JPEGs are written without optimized Huffman tables, so the
        // transcode should save bytes.
        let src = fixture("landscape.jpg");
        let bytes = std::fs::read(&src).unwrap();
        let out = optimize_lossless(&bytes).unwrap();
        let _ = std::fs::remove_file(&out.tmp_path);
        assert!(
            out.bytes < bytes.len() as u64,
            "transcode {} should be < src {}",
            out.bytes,
            bytes.len()
        );
    }

    #[test]
    fn lossless_transcode_rejects_corrupt_input() {
        let bytes = std::fs::read(fixture("corrupt.jpg")).unwrap();
        assert!(optimize_lossless(&bytes).is_err());
    }

    #[test]
    fn icc_profile_roundtrips_through_lossy_encode() {
        let mut decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        // >65519 bytes forces the multi-APP2 split on write and the
        // out-of-order reassembly path on read
        let profile = crate::encoder::icc::test_profile(150_000);
        decoded.icc_profile = Some(profile.clone());
        let out = encode(&decoded, 80, true).unwrap();
        let back = decode(&out.tmp_path, ImageExt::Jpeg).unwrap();
        let _ = std::fs::remove_file(&out.tmp_path);
        assert_eq!(back.icc_profile.as_deref(), Some(profile.as_slice()));
    }

    #[test]
    fn lossless_transcode_preserves_icc() {
        let src = fixture("with_icc.jpg");
        let original = decode(&src, ImageExt::Jpeg).unwrap();
        assert!(original.icc_profile.is_some(), "fixture must carry ICC");
        let bytes = std::fs::read(&src).unwrap();
        let out = optimize_lossless(&bytes).unwrap();
        let back = decode(&out.tmp_path, ImageExt::Jpeg).unwrap();
        let _ = std::fs::remove_file(&out.tmp_path);
        assert_eq!(back.icc_profile, original.icc_profile);
    }
}
