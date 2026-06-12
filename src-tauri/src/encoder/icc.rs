// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! ICC profile passthrough helpers (rubric §0.5 metadata row).
//!
//! Wide-gamut sources (e.g. Display P3 from iPhones) shift colors when their
//! ICC profile is dropped on re-encode. The decode pipeline extracts the
//! profile into `DecodedImage.icc_profile`; each encoder re-embeds it:
//!
//! - JPEG: APP2 markers (`mozjpeg::CompressStarted::write_icc_profile`)
//! - PNG:  iCCP chunk (`image::codecs::png::PngEncoder::set_icc_profile`)
//! - WebP: ICCP chunk via VP8X extended container ([`embed_webp_icc`])
//! - AVIF: **not supported** — ravif/avif-serialize only write nclx `colr`
//!   boxes, not ICC (`prof`) ones; AVIF output drops the profile (known
//!   limitation, documented in docs/rubric.md)

/// Prefix of every ICC APP2 marker payload: identifier + seq byte + count byte.
const JPEG_ICC_PREFIX: &[u8] = b"ICC_PROFILE\0";
const JPEG_ICC_HEADER_LEN: usize = 14; // prefix(12) + seq(1) + count(1)

/// Split an ICC profile into spec-compliant APP2 marker payloads (ICC spec
/// annex B: `ICC_PROFILE\0` + 1-based seq + chunk count, ≤65519 profile bytes
/// per marker). Hand these to `CompressStarted::write_marker`.
///
/// Not `mozjpeg::CompressStarted::write_icc_profile`: as of mozjpeg 0.10.13
/// it numbers chunks 0-based, which compliant readers (e.g. libjpeg-turbo's
/// `jpeg_read_icc_profile`) reject.
pub fn jpeg_app2_segments(icc: &[u8]) -> Vec<Vec<u8>> {
    // 65533 max marker data − 14 byte ICC chunk header
    const MAX_PROFILE_BYTES_PER_MARKER: usize = 65519;
    let chunks = icc.chunks(MAX_PROFILE_BYTES_PER_MARKER);
    let count = chunks.len();
    chunks
        .enumerate()
        .map(|(i, chunk)| {
            let mut seg = Vec::with_capacity(JPEG_ICC_HEADER_LEN + chunk.len());
            seg.extend_from_slice(JPEG_ICC_PREFIX);
            seg.push((i + 1) as u8);
            seg.push(count as u8);
            seg.extend_from_slice(chunk);
            seg
        })
        .collect()
}

/// Reassemble an ICC profile from JPEG APP2 marker payloads (ICC spec annex B:
/// the profile is split into up to 255 chunks, each tagged `seq/count`,
/// 1-based, not guaranteed to appear in order). Files written by the Rust
/// mozjpeg crate's own `write_icc_profile` number chunks 0-based; that
/// off-by-one is tolerated on read.
///
/// Returns `None` when no ICC markers are present or the chunk set is
/// inconsistent (missing/duplicate seq, disagreeing counts).
pub fn assemble_jpeg_app2_icc<'a>(
    payloads: impl IntoIterator<Item = &'a [u8]>,
) -> Option<Vec<u8>> {
    let mut chunks: Vec<(u8, &[u8])> = Vec::new();
    let mut count: Option<u8> = None;
    for p in payloads {
        if p.len() < JPEG_ICC_HEADER_LEN || &p[..12] != JPEG_ICC_PREFIX {
            continue; // some other APP2 use (e.g. MPF) — ignore
        }
        let (seq, total) = (p[12], p[13]);
        if total == 0 {
            return None;
        }
        match count {
            Some(c) if c != total => return None,
            _ => count = Some(total),
        }
        chunks.push((seq, &p[JPEG_ICC_HEADER_LEN..]));
    }
    let count = count?;
    if chunks.len() != count as usize {
        return None;
    }
    chunks.sort_by_key(|&(seq, _)| seq);
    let base = chunks[0].0; // 1 per spec; 0 from the buggy mozjpeg crate writer
    if base > 1 || chunks.iter().enumerate().any(|(i, &(seq, _))| seq as usize != i + base as usize) {
        return None;
    }
    let icc: Vec<u8> = chunks.into_iter().flat_map(|(_, d)| d.iter().copied()).collect();
    (!icc.is_empty()).then_some(icc)
}

/// Insert an ICCP chunk into a WebP file, upgrading the simple container that
/// libwebp's one-shot encoder emits to the VP8X extended format when needed.
/// `canvas_w`/`canvas_h` are the pixel dimensions (used for a fresh VP8X).
///
/// Returns `None` when the input is not a parseable WebP (callers fall back
/// to the profile-less original rather than failing the encode).
pub fn embed_webp_icc(webp: &[u8], icc: &[u8], canvas_w: u32, canvas_h: u32) -> Option<Vec<u8>> {
    if icc.is_empty() || webp.len() < 12 || &webp[..4] != b"RIFF" || &webp[8..12] != b"WEBP" {
        return None;
    }

    // Parse the chunk list
    let mut chunks: Vec<(&[u8], &[u8])> = Vec::new(); // (fourcc, payload)
    let mut pos = 12;
    while pos + 8 <= webp.len() {
        let fourcc = &webp[pos..pos + 4];
        let size = u32::from_le_bytes(webp[pos + 4..pos + 8].try_into().ok()?) as usize;
        let payload = webp.get(pos + 8..pos + 8 + size)?;
        chunks.push((fourcc, payload));
        pos += 8 + size + (size & 1); // chunks are padded to even length
    }
    if chunks.iter().any(|&(f, _)| f == b"ICCP") {
        return None; // already tagged — don't double up
    }

    // VP8X payload: 1 byte flags, 3 reserved, canvas (w-1, h-1) as 24-bit LE
    const FLAG_ICC: u8 = 0x20;
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
    vp8x[0] |= FLAG_ICC;
    let has_alpha = chunks.iter().any(|&(f, _)| f == b"ALPH")
        || chunks
            .iter()
            .find(|&&(f, _)| f == b"VP8L")
            .is_some_and(|&(_, d)| vp8l_has_alpha(d));
    if has_alpha {
        vp8x[0] |= FLAG_ALPHA;
    }

    // Rebuild: VP8X first, ICCP right after (spec order), then the rest
    let mut body = Vec::with_capacity(webp.len() + icc.len() + 32);
    push_riff_chunk(&mut body, b"VP8X", &vp8x);
    push_riff_chunk(&mut body, b"ICCP", icc);
    for &(fourcc, payload) in chunks.iter().filter(|&&(f, _)| f != b"VP8X") {
        push_riff_chunk(&mut body, fourcc, payload);
    }

    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&u32::try_from(body.len() + 4).ok()?.to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&body);
    Some(out)
}

pub(crate) fn push_riff_chunk(out: &mut Vec<u8>, fourcc: &[u8], payload: &[u8]) {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0); // pad to even
    }
}

/// VP8L header: signature byte 0x2F, then a 32-bit LE word whose bit 28 is
/// the alpha_is_used hint.
pub(crate) fn vp8l_has_alpha(payload: &[u8]) -> bool {
    payload.len() >= 5
        && payload[0] == 0x2F
        && (u32::from_le_bytes(payload[1..5].try_into().unwrap()) >> 28) & 1 == 1
}

/// Extract an ICC profile from an AVIF: walk `meta` → `iprp` → `ipco` and
/// take the first `colr` box of colour_type `prof`/`ricc`.
///
/// Simplification: ipma item association is not checked — Hando only deals
/// with single-image AVIFs, where the first ICC `colr` is the primary item's.
pub fn extract_avif_icc(bytes: &[u8]) -> Option<Vec<u8>> {
    let meta = find_isobmff_box(bytes, b"meta")?;
    let meta_payload = meta.get(4..)?; // meta is a FullBox: skip version+flags
    let iprp = find_isobmff_box(meta_payload, b"iprp")?;
    let ipco = find_isobmff_box(iprp, b"ipco")?;

    let mut rest = ipco;
    while let Some((box_type, payload, remaining)) = next_isobmff_box(rest) {
        if box_type == *b"colr" && payload.len() > 4 {
            let colour_type = &payload[..4];
            if colour_type == b"prof" || colour_type == b"ricc" {
                return Some(payload[4..].to_vec());
            }
        }
        rest = remaining;
    }
    None
}

/// Find the first top-level box of `box_type` in `data`, returning its payload.
fn find_isobmff_box<'a>(data: &'a [u8], box_type: &[u8; 4]) -> Option<&'a [u8]> {
    let mut rest = data;
    while let Some((typ, payload, remaining)) = next_isobmff_box(rest) {
        if typ == *box_type {
            return Some(payload);
        }
        rest = remaining;
    }
    None
}

/// Parse one ISOBMFF box header; returns (type, payload, rest-after-box).
fn next_isobmff_box(data: &[u8]) -> Option<([u8; 4], &[u8], &[u8])> {
    if data.len() < 8 {
        return None;
    }
    let size32 = u32::from_be_bytes(data[..4].try_into().ok()?) as u64;
    let box_type: [u8; 4] = data[4..8].try_into().ok()?;
    let (header_len, box_len) = match size32 {
        0 => (8u64, data.len() as u64), // box extends to end of enclosing space
        1 => {
            let large = u64::from_be_bytes(data.get(8..16)?.try_into().ok()?);
            (16u64, large)
        }
        n => (8u64, n),
    };
    if box_len < header_len || box_len > data.len() as u64 {
        return None;
    }
    let payload = &data[header_len as usize..box_len as usize];
    Some((box_type, payload, &data[box_len as usize..]))
}

/// Deterministic, structurally valid ICC profile blob for tests: correct size
/// field, `acsp` signature at offset 36, zero tags, patterned tail.
#[cfg(test)]
pub(crate) fn test_profile(len: usize) -> Vec<u8> {
    assert!(len >= 132, "ICC header(128) + tag count(4)");
    let mut p = vec![0u8; len];
    p[..4].copy_from_slice(&(len as u32).to_be_bytes());
    p[36..40].copy_from_slice(b"acsp");
    for (i, b) in p[132..].iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app2(seq: u8, count: u8, data: &[u8]) -> Vec<u8> {
        let mut p = JPEG_ICC_PREFIX.to_vec();
        p.push(seq);
        p.push(count);
        p.extend_from_slice(data);
        p
    }

    #[test]
    fn assembles_multichunk_app2_out_of_order() {
        let chunks = [app2(2, 3, b"BBB"), app2(3, 3, b"CC"), app2(1, 3, b"AAAA")];
        let icc = assemble_jpeg_app2_icc(chunks.iter().map(|c| c.as_slice())).unwrap();
        assert_eq!(icc, b"AAAABBBCC");
    }

    #[test]
    fn segments_are_one_based_and_reassemble() {
        let profile = test_profile(150_000); // 3 chunks
        let segments = jpeg_app2_segments(&profile);
        assert_eq!(segments.len(), 3);
        assert_eq!((segments[0][12], segments[0][13]), (1, 3), "seq must be 1-based");
        assert_eq!((segments[2][12], segments[2][13]), (3, 3));
        let back = assemble_jpeg_app2_icc(segments.iter().map(|s| s.as_slice())).unwrap();
        assert_eq!(back, profile);
    }

    #[test]
    fn tolerates_zero_based_chunks_from_buggy_mozjpeg_writer() {
        // mozjpeg 0.10.13's write_icc_profile numbers chunks 0-based
        let chunks = [app2(0, 2, b"AA"), app2(1, 2, b"BB")];
        let icc = assemble_jpeg_app2_icc(chunks.iter().map(|c| c.as_slice())).unwrap();
        assert_eq!(icc, b"AABB");
    }

    #[test]
    fn rejects_missing_chunk_and_ignores_foreign_app2() {
        let missing = [app2(1, 2, b"AA")]; // chunk 2/2 absent
        assert!(assemble_jpeg_app2_icc(missing.iter().map(|c| c.as_slice())).is_none());
        let foreign = [b"MPF\0junk-that-is-long-enough".to_vec()];
        assert!(assemble_jpeg_app2_icc(foreign.iter().map(|c| c.as_slice())).is_none());
    }

    #[test]
    fn webp_embed_upgrades_simple_container() {
        // Minimal simple-container WebP: a fake VP8 chunk (content irrelevant
        // to the RIFF surgery; bitstream validity is covered in webp.rs tests)
        let mut webp = b"RIFF\0\0\0\0WEBP".to_vec();
        push_riff_chunk(&mut webp, b"VP8 ", &[0xAB; 11]); // odd len → exercises padding
        let len = (webp.len() - 8) as u32;
        webp[4..8].copy_from_slice(&len.to_le_bytes());

        let icc = test_profile(200);
        let out = embed_webp_icc(&webp, &icc, 640, 480).unwrap();

        assert_eq!(&out[..4], b"RIFF");
        assert_eq!(&out[8..12], b"WEBP");
        // VP8X first with ICC flag and canvas 639x479
        assert_eq!(&out[12..16], b"VP8X");
        let vp8x = &out[20..30];
        assert_ne!(vp8x[0] & 0x20, 0, "ICC flag must be set");
        assert_eq!(&vp8x[4..7], &639u32.to_le_bytes()[..3]);
        assert_eq!(&vp8x[7..10], &479u32.to_le_bytes()[..3]);
        // ICCP immediately after VP8X
        assert_eq!(&out[30..34], b"ICCP");
        let iccp_len = u32::from_le_bytes(out[34..38].try_into().unwrap()) as usize;
        assert_eq!(&out[38..38 + iccp_len], icc.as_slice());
        // RIFF size field consistent with the real length
        let riff = u32::from_le_bytes(out[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff + 8, out.len());
    }

    #[test]
    fn webp_embed_rejects_garbage() {
        assert!(embed_webp_icc(b"not a webp", &test_profile(132), 1, 1).is_none());
    }

    #[test]
    fn avif_extracts_prof_colr_from_synthetic_meta() {
        let icc = test_profile(160);
        // colr box: size + 'colr' + 'prof' + profile
        let mut colr = ((8 + 4 + icc.len()) as u32).to_be_bytes().to_vec();
        colr.extend_from_slice(b"colr");
        colr.extend_from_slice(b"prof");
        colr.extend_from_slice(&icc);
        let wrap = |typ: &[u8; 4], payload: &[u8]| {
            let mut b = ((8 + payload.len()) as u32).to_be_bytes().to_vec();
            b.extend_from_slice(typ);
            b.extend_from_slice(payload);
            b
        };
        let ipco = wrap(b"ipco", &colr);
        let iprp = wrap(b"iprp", &ipco);
        let mut meta_payload = vec![0u8; 4]; // FullBox version+flags
        meta_payload.extend_from_slice(&iprp);
        let meta = wrap(b"meta", &meta_payload);
        let mut file = wrap(b"ftyp", b"avif\0\0\0\0avifmif1");
        file.extend_from_slice(&meta);

        assert_eq!(extract_avif_icc(&file).unwrap(), icc);
        assert!(extract_avif_icc(b"\0\0\0\x08ftyp").is_none());
    }
}
