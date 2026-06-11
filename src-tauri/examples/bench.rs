// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Benchmark harness for the optimization rubric (docs/rubric.md).
//!
//! For every fixture × candidate (format × quality) this prints the three
//! numbers the rubric judges by: size, ssimulacra2, encode time.
//!
//! Usage:
//!   cargo run --release --example bench                  # sweep (markdown table)
//!   cargo run --release --example bench -- calibrate DIR # write quality ladders for human eyeballing (§8.2)

use desktop_lib::encoder::{avif, decode, jpeg, judge, png, webp, ImageExt};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// One encode with explicit knobs, decoded + judged. Used by the grid mode.
struct GridResult {
    bytes: u64,
    encode_ms: u128,
    score: f64,
}

fn run_judged(
    baseline: &decode::DecodedImage,
    ext: ImageExt,
    encode_fn: impl FnOnce() -> Result<desktop_lib::encoder::EncodedFile, desktop_lib::encoder::EncodeError>,
) -> Result<GridResult, String> {
    let t0 = Instant::now();
    let out = encode_fn().map_err(|e| e.to_string())?;
    let encode_ms = t0.elapsed().as_millis();
    let decoded = decode::decode(&out.tmp_path, ext).map_err(|e| e.to_string())?;
    let score = if judge::pixels_identical(baseline, &decoded) {
        100.0
    } else {
        judge::ssimulacra2_score(baseline, &decoded).map_err(|e| e.to_string())?
    };
    let bytes = out.bytes;
    let _ = std::fs::remove_file(&out.tmp_path);
    Ok(GridResult { bytes, encode_ms, score })
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

struct Candidate {
    label: String,
    ext: ImageExt,
    bytes: u64,
    encode_ms: u128,
    score: f64,
    lossless: bool,
    tmp_path: PathBuf,
}

fn encode_candidate(
    baseline: &decode::DecodedImage,
    ext: ImageExt,
    quality: u32,
) -> Result<Candidate, String> {
    let t0 = Instant::now();
    let out = match ext {
        ImageExt::Jpeg => jpeg::encode(baseline, quality, true),
        ImageExt::Png => png::encode(baseline, quality, 2),
        ImageExt::Webp => webp::encode(baseline, quality, 4),
        ImageExt::Avif => avif::encode(baseline, quality, 8),
    }
    .map_err(|e| e.to_string())?;
    let encode_ms = t0.elapsed().as_millis();

    let decoded = decode::decode(&out.tmp_path, ext).map_err(|e| e.to_string())?;
    let lossless = judge::pixels_identical(baseline, &decoded);
    let score = if lossless {
        100.0
    } else {
        judge::ssimulacra2_score(baseline, &decoded).map_err(|e| e.to_string())?
    };

    Ok(Candidate {
        label: format!("{}@q{}", ext.dotted().trim_start_matches('.'), quality),
        ext,
        bytes: out.bytes,
        encode_ms,
        score,
        lossless,
        tmp_path: out.tmp_path,
    })
}

/// Quality ladder per output format. Cross-format candidates included —
/// rubric §5: all candidates compete under the same gates.
fn qualities_for(ext: ImageExt) -> &'static [u32] {
    match ext {
        ImageExt::Jpeg => &[50, 65, 75, 85, 90],
        ImageExt::Png => &[60, 75, 85, 100],
        ImageExt::Webp => &[50, 65, 75, 85, 90, 100],
        ImageExt::Avif => &[40, 50, 60, 75, 85],
    }
}

fn sweep_fixtures() -> Vec<(&'static str, ImageExt)> {
    let mut v = vec![
        ("landscape.jpg", ImageExt::Jpeg),
        ("portrait_exif_rotated.jpg", ImageExt::Jpeg),
        ("screenshot.png", ImageExt::Png),
        ("transparent.png", ImageExt::Png),
        ("tiny.png", ImageExt::Png),
    ];
    if fixture("large_photo.jpg").exists() {
        v.push(("large_photo.jpg", ImageExt::Jpeg));
    }
    v
}

fn sweep() {
    println!("# Hando bench sweep\n");
    println!("| fixture | candidate | bytes | ratio | ssimulacra2 | encode ms | lossless |");
    println!("|---|---|---:|---:|---:|---:|---|");

    for (name, src_ext) in sweep_fixtures() {
        let path = fixture(name);
        let src_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let baseline = match decode::decode(&path, src_ext) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skip {name}: {e}");
                continue;
            }
        };
        let bpp = judge::bits_per_pixel(src_bytes, baseline.width, baseline.height);
        eprintln!(
            "{name}: {}x{} {src_bytes}B bpp={bpp:.2}",
            baseline.width, baseline.height
        );

        for out_ext in [ImageExt::Jpeg, ImageExt::Png, ImageExt::Webp, ImageExt::Avif] {
            // JPEG can't carry alpha; don't offer it for transparent sources
            if out_ext == ImageExt::Jpeg
                && baseline.rgba.chunks_exact(4).any(|p| p[3] < 255)
            {
                continue;
            }
            for &q in qualities_for(out_ext) {
                match encode_candidate(&baseline, out_ext, q) {
                    Ok(c) => {
                        let ratio = 1.0 - (c.bytes as f64 / src_bytes.max(1) as f64);
                        println!(
                            "| {name} | {} | {} | {:+.1}% | {:.2} | {} | {} |",
                            c.label,
                            c.bytes,
                            ratio * 100.0,
                            c.score,
                            c.encode_ms,
                            if c.lossless { "yes" } else { "" }
                        );
                        let _ = std::fs::remove_file(&c.tmp_path);
                    }
                    Err(e) => eprintln!("  {name} {out_ext:?}@q{q} failed: {e}"),
                }
            }
        }
    }
}

/// §8.2 — write quality ladders for the two hardest content classes
/// (photo + text/UI screenshot) so a human can eyeball where "starts to
/// look different" sits, then pin S in docs/rubric.md §6.
fn calibrate(outdir: &Path) {
    std::fs::create_dir_all(outdir).expect("create outdir");
    let subjects: &[(&str, ImageExt)] = &[
        ("landscape.jpg", ImageExt::Jpeg),
        ("screenshot.png", ImageExt::Png),
    ];
    let ladder: &[u32] = &[95, 90, 85, 80, 75, 70, 60, 50];

    println!("| subject | candidate | ssimulacra2 | bytes | file |");
    println!("|---|---|---:|---:|---|");

    for (name, src_ext) in subjects {
        let path = fixture(name);
        let baseline = decode::decode(&path, *src_ext).expect("decode fixture");
        let stem = name.split('.').next().unwrap();
        std::fs::copy(&path, outdir.join(format!("{stem}_ORIGINAL.{}", name.split('.').last().unwrap())))
            .expect("copy original");

        for out_ext in [ImageExt::Jpeg, ImageExt::Webp, ImageExt::Avif] {
            if *src_ext == ImageExt::Png && out_ext == ImageExt::Jpeg {
                continue; // screenshots: judge webp/avif lossy edges instead
            }
            for &q in ladder {
                match encode_candidate(&baseline, out_ext, q) {
                    Ok(c) => {
                        let fname = format!(
                            "{stem}_{}_q{q}_s{:.1}{}",
                            out_ext.dotted().trim_start_matches('.'),
                            c.score,
                            c.ext.dotted()
                        );
                        let dest = outdir.join(&fname);
                        if std::fs::rename(&c.tmp_path, &dest).is_err() {
                            std::fs::copy(&c.tmp_path, &dest).expect("place sample");
                            let _ = std::fs::remove_file(&c.tmp_path);
                        }
                        println!("| {name} | {} | {:.2} | {} | {fname} |", c.label, c.score, c.bytes);
                    }
                    Err(e) => eprintln!("  {name} {out_ext:?}@q{q} failed: {e}"),
                }
            }
        }
    }
    println!("\nSamples written to {} — view side-by-side with the _ORIGINAL files.", outdir.display());
}

/// §8.5 parameter golf — sweep the effort knobs the encoders now expose and
/// print size/score/time per combination so defaults can be chosen from data.
fn grid() {
    let subjects: &[(&str, ImageExt)] = &[
        ("landscape.jpg", ImageExt::Jpeg),
        ("screenshot.png", ImageExt::Png),
        ("transparent.png", ImageExt::Png),
    ];

    println!("# Hando knob grid\n");
    println!("| subject | knob combo | bytes | ssimulacra2 | encode ms |");
    println!("|---|---|---:|---:|---:|");

    for (name, src_ext) in subjects {
        let baseline = decode::decode(&fixture(name), *src_ext).expect("decode fixture");
        let has_alpha = baseline.rgba.chunks_exact(4).any(|p| p[3] < 255);

        // AVIF: speed × quality
        for &speed in &[4u8, 6, 8, 10] {
            for &q in &[50u32, 75] {
                if let Ok(r) = run_judged(&baseline, ImageExt::Avif, || avif::encode(&baseline, q, speed)) {
                    println!("| {name} | avif q{q} speed={speed} | {} | {:.2} | {} |", r.bytes, r.score, r.encode_ms);
                }
            }
        }
        // WebP: method × quality (incl. lossless)
        for &method in &[0u8, 2, 4, 6] {
            for &q in &[75u32, 100] {
                if let Ok(r) = run_judged(&baseline, ImageExt::Webp, || webp::encode(&baseline, q, method)) {
                    println!("| {name} | webp q{q} method={method} | {} | {:.2} | {} |", r.bytes, r.score, r.encode_ms);
                }
            }
        }
        // PNG: oxipng level × quality
        for &level in &[2u8, 4, 6] {
            for &q in &[75u32, 100] {
                if let Ok(r) = run_judged(&baseline, ImageExt::Png, || png::encode(&baseline, q, level)) {
                    println!("| {name} | png q{q} oxipng={level} | {} | {:.2} | {} |", r.bytes, r.score, r.encode_ms);
                }
            }
        }
        // JPEG: progressive on/off
        if !has_alpha {
            for &prog in &[true, false] {
                if let Ok(r) = run_judged(&baseline, ImageExt::Jpeg, || jpeg::encode(&baseline, 75, prog)) {
                    println!("| {name} | jpg q75 progressive={prog} | {} | {:.2} | {} |", r.bytes, r.score, r.encode_ms);
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("calibrate") => {
            let dir = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("../docs/calibration"));
            calibrate(&dir);
        }
        Some("grid") => grid(),
        _ => sweep(),
    }
}
