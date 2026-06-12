// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! EXIF metadata passthrough helpers (user-controlled via `EncodeOpts.keep_metadata`).
//!
//! The decode pipeline extracts the raw EXIF blob (a TIFF stream, without the
//! JPEG `Exif\0\0` prefix) into `DecodedImage.exif`; each encoder re-embeds it:
//!
//! - JPEG: APP1 marker (`Exif\0\0` + TIFF), written in `jpeg::encode`
//! - PNG:  eXIf chunk spliced after IHDR ([`embed_png_exif`], post-oxipng)
//! - WebP: EXIF chunk via the VP8X extended container ([`embed_webp_exif`])
//! - AVIF: **not supported** — ravif/avif-serialize expose no metadata API
//!   (same limitation as ICC, see `icc.rs`)
//!
//! Decode rotates pixels upright, so a rotated source's Orientation tag must
//! be normalized to 1 ([`set_orientation_upright`]) before the blob is kept —
//! otherwise viewers would rotate the image a second time.

use super::icc::{push_riff_chunk, vp8l_has_alpha};

/// JPEG APP1 EXIF payload prefix.
pub const JPEG_EXIF_PREFIX: &[u8] = b"Exif\0\0";

/// Strip the `Exif\0\0` prefix if present. WebP EXIF chunks appear in the
/// wild both with and without it; we store the bare TIFF stream.
pub fn strip_exif_prefix(blob: &[u8]) -> &[u8] {
    blob.strip_prefix(JPEG_EXIF_PREFIX).unwrap_or(blob)
}

/// Rewrite the IFD0 Orientation tag (0x0112) to 1 in a raw TIFF stream.
/// Returns `false` when the stream can't be parsed or has no Orientation tag;
/// callers drop the blob in that case if the source pixels were rotated.
pub fn set_orientation_upright(tiff: &mut [u8]) -> bool {
    let be = match tiff.get(..4) {
        Some(b"MM\0\x2a") => true,
        Some(b"II\x2a\0") => false,
        _ => return false,
    };
    let rd16 = |b: &[u8], o: usize| -> Option<u16> {
        let v: [u8; 2] = b.get(o..o + 2)?.try_into().ok()?;
        Some(if be { u16::from_be_bytes(v) } else { u16::from_le_bytes(v) })
    };
    let rd32 = |b: &[u8], o: usize| -> Option<u32> {
        let v: [u8; 4] = b.get(o..o + 4)?.try_into().ok()?;
        Some(if be { u32::from_be_bytes(v) } else { u32::from_le_bytes(v) })
    };
    let Some(ifd0) = rd32(tiff, 4) else { return false };
    let ifd0 = ifd0 as usize;
    let Some(count) = rd16(tiff, ifd0) else { return false };
    for i in 0..count as usize {
        let entry = ifd0 + 2 + i * 12;
        let Some(tag) = rd16(tiff, entry) else { return false };
        if tag == 0x0112 {
            // SHORT value lives inline in the first 2 bytes of the value field
            let one = if be { 1u16.to_be_bytes() } else { 1u16.to_le_bytes() };
            let Some(slot) = tiff.get_mut(entry + 8..entry + 10) else { return false };
            slot.copy_from_slice(&one);
            return true;
        }
    }
    false
}

/// Pull the raw EXIF (TIFF) stream out of a PNG's eXIf chunk.
pub fn extract_png_exif(png: &[u8]) -> Option<Vec<u8>> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if !png.starts_with(SIG) {
        return None;
    }
    let mut pos = SIG.len();
    while pos + 8 <= png.len() {
        let len = u32::from_be_bytes(png[pos..pos + 4].try_into().ok()?) as usize;
        let typ = &png[pos + 4..pos + 8];
        let data = png.get(pos + 8..pos + 8 + len)?;
        if typ == b"eXIf" {
            return (!data.is_empty()).then(|| data.to_vec());
        }
        if typ == b"IEND" {
            break;
        }
        pos += 12 + len; // len + type + data + crc
    }
    None
}

/// Pull the raw EXIF (TIFF) stream out of a WebP's EXIF chunk.
pub fn extract_webp_exif(webp: &[u8]) -> Option<Vec<u8>> {
    for (fourcc, payload) in webp_chunks(webp)? {
        if fourcc == b"EXIF" {
            let tiff = strip_exif_prefix(payload);
            return (!tiff.is_empty()).then(|| tiff.to_vec());
        }
    }
    None
}

/// Insert an eXIf chunk right after IHDR. Returns `None` when the input is
/// not a parseable PNG or already carries eXIf (callers keep the un-tagged
/// bytes rather than failing the encode).
pub fn embed_png_exif(png: &[u8], exif: &[u8]) -> Option<Vec<u8>> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if exif.is_empty() || !png.starts_with(SIG) {
        return None;
    }
    let mut pos = SIG.len();
    let mut ihdr_end = None;
    while pos + 8 <= png.len() {
        let len = u32::from_be_bytes(png[pos..pos + 4].try_into().ok()?) as usize;
        let typ = &png[pos + 4..pos + 8];
        let next = pos + 12 + len;
        if next > png.len() {
            return None;
        }
        if typ == b"eXIf" {
            return None; // already tagged — don't double up
        }
        if typ == b"IHDR" {
            ihdr_end = Some(next);
        }
        if typ == b"IEND" {
            break;
        }
        pos = next;
    }
    let at = ihdr_end?;

    let mut crc_input = Vec::with_capacity(4 + exif.len());
    crc_input.extend_from_slice(b"eXIf");
    crc_input.extend_from_slice(exif);

    let mut out = Vec::with_capacity(png.len() + exif.len() + 12);
    out.extend_from_slice(&png[..at]);
    out.extend_from_slice(&u32::try_from(exif.len()).ok()?.to_be_bytes());
    out.extend_from_slice(b"eXIf");
    out.extend_from_slice(exif);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out.extend_from_slice(&png[at..]);
    Some(out)
}

/// Append an EXIF chunk to a WebP, upgrading the simple container to VP8X
/// when needed (mirrors `icc::embed_webp_icc`; EXIF sits after the image data
/// per the container spec's chunk order). Returns `None` on unparseable input
/// or when an EXIF chunk already exists.
pub fn embed_webp_exif(webp: &[u8], exif: &[u8], canvas_w: u32, canvas_h: u32) -> Option<Vec<u8>> {
    if exif.is_empty() {
        return None;
    }
    let chunks = webp_chunks(webp)?;
    if chunks.iter().any(|&(f, _)| f == b"EXIF") {
        return None;
    }

    const FLAG_EXIF: u8 = 0x08;
    const FLAG_ALPHA: u8 = 0x10;
    let mut vp8x = match chunks.iter().find(|&&(f, _)| f == b"VP8X") {
        Some(&(_, payload)) if payload.len() == 10 => payload.to_vec(),
        Some(_) => return None, // malformed VP8X
        None => {
            let mut p = vec![0u8; 10];
            let w = canvas_w.checked_sub(1)?.to_le_bytes();
            let h = canvas_h.checked_sub(1)?.to_le_bytes();
            p[4..7].copy_from_slice(&w[..3]);
            p[7..10].copy_from_slice(&h[..3]);
            p
        }
    };
    vp8x[0] |= FLAG_EXIF;
    let has_alpha = chunks.iter().any(|&(f, _)| f == b"ALPH")
        || chunks
            .iter()
            .find(|&&(f, _)| f == b"VP8L")
            .is_some_and(|&(_, d)| vp8l_has_alpha(d));
    if has_alpha {
        vp8x[0] |= FLAG_ALPHA;
    }

    let mut body = Vec::with_capacity(webp.len() + exif.len() + 32);
    push_riff_chunk(&mut body, b"VP8X", &vp8x);
    for &(fourcc, payload) in chunks.iter().filter(|&&(f, _)| f != b"VP8X") {
        push_riff_chunk(&mut body, fourcc, payload);
    }
    push_riff_chunk(&mut body, b"EXIF", exif);

    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&u32::try_from(body.len() + 4).ok()?.to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&body);
    Some(out)
}

/// Parse a WebP's RIFF chunk list as (fourcc, payload) pairs.
fn webp_chunks(webp: &[u8]) -> Option<Vec<(&[u8], &[u8])>> {
    if webp.len() < 12 || &webp[..4] != b"RIFF" || &webp[8..12] != b"WEBP" {
        return None;
    }
    let mut chunks = Vec::new();
    let mut pos = 12;
    while pos + 8 <= webp.len() {
        let fourcc = &webp[pos..pos + 4];
        let size = u32::from_le_bytes(webp[pos + 4..pos + 8].try_into().ok()?) as usize;
        let payload = webp.get(pos + 8..pos + 8 + size)?;
        chunks.push((fourcc, payload));
        pos += 8 + size + (size & 1); // chunks are padded to even length
    }
    Some(chunks)
}

/// CRC-32 (ISO 3309, the PNG flavor), bitwise. One small chunk per file —
/// speed is irrelevant, a dependency is not worth it.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Minimal valid EXIF/TIFF stream for tests: IFD0 with a single Orientation
/// entry set to `orientation`.
#[cfg(test)]
pub(crate) fn test_exif(orientation: u16, big_endian: bool) -> Vec<u8> {
    let mut t = Vec::new();
    let w16 = |v: u16| if big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
    let w32 = |v: u32| if big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
    t.extend_from_slice(if big_endian { b"MM\0\x2a" } else { b"II\x2a\0" });
    t.extend_from_slice(&w32(8)); // IFD0 offset
    t.extend_from_slice(&w16(1)); // entry count
    t.extend_from_slice(&w16(0x0112)); // Orientation
    t.extend_from_slice(&w16(3)); // SHORT
    t.extend_from_slice(&w32(1)); // count
    t.extend_from_slice(&w16(orientation));
    t.extend_from_slice(&w16(0)); // value padding
    t.extend_from_slice(&w32(0)); // next IFD
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_orientation_in_both_byte_orders() {
        for be in [false, true] {
            let mut tiff = test_exif(6, be);
            assert!(set_orientation_upright(&mut tiff), "be={be}");
            // re-parse via the same minimal layout: value at offset 18
            let v = if be {
                u16::from_be_bytes([tiff[18], tiff[19]])
            } else {
                u16::from_le_bytes([tiff[18], tiff[19]])
            };
            assert_eq!(v, 1, "be={be}");
        }
    }

    #[test]
    fn rejects_garbage_tiff() {
        assert!(!set_orientation_upright(&mut b"not tiff at all".to_vec()));
        assert!(!set_orientation_upright(&mut Vec::new()));
    }

    #[test]
    fn strips_exif_prefix_only_when_present() {
        assert_eq!(strip_exif_prefix(b"Exif\0\0II"), b"II");
        assert_eq!(strip_exif_prefix(b"II\x2a\0rest"), b"II\x2a\0rest");
    }

    fn synthetic_png() -> Vec<u8> {
        // signature + IHDR(13) + IDAT(3) + IEND(0); CRCs are dummies — the
        // splicer never validates them
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let chunk = |out: &mut Vec<u8>, typ: &[u8; 4], data: &[u8]| {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(typ);
            out.extend_from_slice(data);
            out.extend_from_slice(&[0; 4]);
        };
        chunk(&mut png, b"IHDR", &[0; 13]);
        chunk(&mut png, b"IDAT", &[1, 2, 3]);
        chunk(&mut png, b"IEND", &[]);
        png
    }

    #[test]
    fn png_embed_extract_roundtrip() {
        let exif = test_exif(1, false);
        let tagged = embed_png_exif(&synthetic_png(), &exif).unwrap();
        // eXIf must sit right after IHDR (offset 8 sig + 25 IHDR chunk)
        assert_eq!(&tagged[37..41], b"eXIf");
        assert_eq!(extract_png_exif(&tagged).unwrap(), exif);
        // no double-tagging
        assert!(embed_png_exif(&tagged, &exif).is_none());
        assert!(embed_png_exif(b"not a png", &exif).is_none());
    }

    #[test]
    fn png_exif_chunk_crc_is_correct() {
        // PNG spec CRC over type+data; verified against the known CRC of
        // "IEND" with empty data (0xAE426082)
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    fn synthetic_webp() -> Vec<u8> {
        let mut webp = b"RIFF\0\0\0\0WEBP".to_vec();
        push_riff_chunk(&mut webp, b"VP8 ", &[0xAB; 11]);
        let len = (webp.len() - 8) as u32;
        webp[4..8].copy_from_slice(&len.to_le_bytes());
        webp
    }

    #[test]
    fn webp_embed_extract_roundtrip_upgrades_container() {
        let exif = test_exif(1, true);
        let out = embed_webp_exif(&synthetic_webp(), &exif, 640, 480).unwrap();
        assert_eq!(&out[..4], b"RIFF");
        assert_eq!(&out[12..16], b"VP8X");
        let vp8x = &out[20..30];
        assert_ne!(vp8x[0] & 0x08, 0, "EXIF flag must be set");
        assert_eq!(extract_webp_exif(&out).unwrap(), exif);
        // RIFF size field consistent
        let riff = u32::from_le_bytes(out[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff + 8, out.len());
        // no double-tagging, garbage rejected
        assert!(embed_webp_exif(&out, &exif, 640, 480).is_none());
        assert!(embed_webp_exif(b"junk", &exif, 1, 1).is_none());
    }

    #[test]
    fn webp_extract_tolerates_exif_prefix() {
        let exif = test_exif(1, false);
        let mut prefixed = JPEG_EXIF_PREFIX.to_vec();
        prefixed.extend_from_slice(&exif);
        let mut webp = synthetic_webp();
        push_riff_chunk(&mut webp, b"EXIF", &prefixed);
        let len = (webp.len() - 8) as u32;
        webp[4..8].copy_from_slice(&len.to_le_bytes());
        assert_eq!(extract_webp_exif(&webp).unwrap(), exif);
    }
}
