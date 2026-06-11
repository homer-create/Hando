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
use std::str::FromStr;
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
        ("landscape2.jpg", ImageExt::Jpeg),
        ("realphoto.png", ImageExt::Png),
        ("screenshot.png", ImageExt::Png),
        ("web-section.png", ImageExt::Png),
        ("jpg-as-png.png", ImageExt::Png),
        ("compressed.jpg", ImageExt::Jpeg),
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

/// Minimal JSON string escaping for the `input`/`error`/`path` fields.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Parse a file's format from its extension (the *source* gate for §1).
fn ext_of(path: &Path) -> Result<ImageExt, String> {
    let s = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| format!("no extension: {}", path.display()))?;
    ImageExt::from_str(s).map_err(|e| e.to_string())
}

/// §verifier — score ONE candidate (format × quality × knob) against a source
/// image and print the rubric numbers as a single JSON line. Exit 0 on a clean
/// measurement, exit 1 (with `"ok":false`) on decode/encode failure or an
/// impossible request (e.g. JPEG output for a transparent source). The loop's
/// orchestrator proposes the candidate; this is the fresh-context verifier.
///
///   eval <input> <out_ext> <quality> [knob]
///     knob: jpeg→progressive(0|1, dflt 1) png→oxipng(0..=6, dflt 4)
///           webp→method(0..=6, dflt 4)     avif→speed(1..=10, dflt 8)
fn eval(args: &[String]) -> i32 {
    let usage = "usage: eval <input> <out_ext:jpg|png|webp|avif> <quality> [knob]";
    let input = match args.first() {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("{usage}");
            return 1;
        }
    };
    let fail = |reason: String| -> i32 {
        println!(
            "{{\"input\":\"{}\",\"ok\":false,\"error\":\"{}\"}}",
            esc(&input.display().to_string()),
            esc(&reason)
        );
        1
    };

    let out_ext = match args.get(1).map(|s| ImageExt::from_str(s)) {
        Some(Ok(e)) => e,
        _ => return fail(format!("bad/missing out_ext. {usage}")),
    };
    let quality: u32 = match args.get(2).and_then(|s| s.parse().ok()) {
        Some(q) => q,
        None => return fail(format!("bad/missing quality. {usage}")),
    };
    // Knob: explicit if given, else the encoder default for that format.
    let knob: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(match out_ext {
        ImageExt::Jpeg => 1, // progressive on
        ImageExt::Png => 4,  // oxipng level
        ImageExt::Webp => 4, // method
        ImageExt::Avif => 8, // speed
    });

    let src_ext = match ext_of(&input) {
        Ok(e) => e,
        Err(e) => return fail(e),
    };
    let src_bytes = std::fs::metadata(&input).map(|m| m.len()).unwrap_or(0);
    let baseline = match decode::decode(&input, src_ext) {
        Ok(b) => b,
        Err(e) => return fail(format!("decode: {e}")),
    };
    let (w, h) = (baseline.width, baseline.height);
    let bpp = judge::bits_per_pixel(src_bytes, w, h);

    if out_ext == ImageExt::Jpeg && baseline.rgba.chunks_exact(4).any(|p| p[3] < 255) {
        return fail("jpeg output cannot carry alpha".into());
    }

    let t0 = Instant::now();
    let encoded = match out_ext {
        ImageExt::Jpeg => jpeg::encode(&baseline, quality, knob != 0),
        ImageExt::Png => png::encode(&baseline, quality, knob as u8),
        ImageExt::Webp => webp::encode(&baseline, quality, knob as u8),
        ImageExt::Avif => avif::encode(&baseline, quality, knob as u8),
    };
    let out = match encoded {
        Ok(o) => o,
        Err(e) => return fail(format!("encode: {e}")),
    };
    let encode_ms = t0.elapsed().as_millis();

    let decoded = match decode::decode(&out.tmp_path, out_ext) {
        Ok(d) => d,
        Err(e) => {
            let _ = std::fs::remove_file(&out.tmp_path);
            return fail(format!("re-decode: {e}"));
        }
    };
    let lossless = judge::pixels_identical(&baseline, &decoded);
    let score = if lossless {
        100.0
    } else {
        match judge::ssimulacra2_score(&baseline, &decoded) {
            Ok(s) => s,
            Err(e) => {
                let _ = std::fs::remove_file(&out.tmp_path);
                return fail(format!("ssimulacra2: {e}"));
            }
        }
    };
    let out_bytes = out.bytes;
    let _ = std::fs::remove_file(&out.tmp_path);

    let ratio = 1.0 - (out_bytes as f64 / src_bytes.max(1) as f64);
    println!(
        "{{\"input\":\"{}\",\"out\":\"{}\",\"quality\":{},\"knob\":{},\
\"src_bytes\":{},\"out_bytes\":{},\"ratio\":{:.4},\"width\":{},\"height\":{},\
\"bpp\":{:.4},\"ssimulacra2\":{:.2},\"lossless\":{},\"encode_ms\":{},\"ok\":true}}",
        esc(&input.display().to_string()),
        out_ext.dotted().trim_start_matches('.'),
        quality,
        knob,
        src_bytes,
        out_bytes,
        ratio,
        w,
        h,
        bpp,
        score,
        lossless,
        encode_ms,
    );
    0
}

/// Enumerate a corpus directory: one JSON line per supported image carrying the
/// §1 input-gate signals (format, dims, bpp, jpeg 8×8 blockiness). `class_hint`
/// is ADVISORY — the loop + rubric make the final A/B call. Lets the
/// orchestrator route each image to the right rubric branch before searching.
fn corpus(dir: &Path) {
    let mut entries: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(e) => {
            eprintln!("read_dir {}: {e}", dir.display());
            return;
        }
    };
    entries.sort();

    for path in entries {
        let Ok(src_ext) = ext_of(&path) else { continue }; // skip non-images
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let baseline = match decode::decode(&path, src_ext) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skip {}: {e}", path.display());
                continue;
            }
        };
        let bpp = judge::bits_per_pixel(bytes, baseline.width, baseline.height);
        let blockiness = judge::jpeg_blockiness(&baseline);
        // §1: lossy format → B; lossless container with a JPEG grid fingerprint
        // (blockiness ≥ 1.25) → B (disguised lossy); otherwise A.
        let lossy_format = matches!(src_ext, ImageExt::Jpeg | ImageExt::Avif);
        let (class_hint, reason) = if lossy_format {
            ("B", "lossy format")
        } else if blockiness >= 1.25 {
            ("B", "disguised lossy (jpeg grid)")
        } else {
            ("A", "lossless, clean")
        };
        println!(
            "{{\"path\":\"{}\",\"format\":\"{}\",\"width\":{},\"height\":{},\
\"bytes\":{},\"bpp\":{:.4},\"blockiness\":{:.2},\"class_hint\":\"{}\",\"reason\":\"{}\"}}",
            esc(&path.display().to_string()),
            src_ext.dotted().trim_start_matches('.'),
            baseline.width,
            baseline.height,
            bytes,
            bpp,
            blockiness,
            class_hint,
            reason,
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("eval") => std::process::exit(eval(&args[1..])),
        Some("corpus") => {
            let dir = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("tests/fixtures"));
            corpus(&dir);
        }
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
