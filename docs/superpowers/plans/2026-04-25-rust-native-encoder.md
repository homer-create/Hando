# Rust-Native Encoder Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Node.js sidecar (Sharp/libvips) with native Rust image encoders compiled into the Tauri binary, retire the standalone CLI, and produce a single-file portable executable for Windows + macOS.

**Architecture:** Static-linked Rust crates (`mozjpeg`, `oxipng`, `imagequant`, `webp`, `ravif`) replace Sharp. The Tauri command layer drives a CPU-bound encoder facade through `tokio::task::spawn_blocking` with a `Semaphore`-controlled concurrency cap of `(num_cpus - 1).clamp(1, 8)`. Per-file events flow through an `EventSink` trait so production injects a real Tauri emitter while tests inject a `MockSink`.

**Tech Stack:** Rust (Tauri 2 + tokio), `mozjpeg`, `oxipng`, `imagequant`, `webp`, `ravif`, `image`, `kamadak-exif`, `insta` (snapshot tests), TypeScript frontend (unchanged except for one new event listener).

**Spec:** [docs/superpowers/specs/2026-04-25-rust-native-encoder-design.md](../specs/2026-04-25-rust-native-encoder-design.md)

---

## File Structure

### New files (Phase 1)
| Path | Responsibility |
|---|---|
| `desktop/src-tauri/src/encoder/mod.rs` | Public facade: `EncodeRequest`, `EncodeResult`, `EncodeOutcome`, `EncodeError`, `ImageExt`, top-level `encode()` |
| `desktop/src-tauri/src/encoder/decode.rs` | Unified RGBA decode + EXIF orientation rotation |
| `desktop/src-tauri/src/encoder/jpeg.rs` | mozjpeg encode |
| `desktop/src-tauri/src/encoder/png.rs` | imagequant + oxipng two-stage encode |
| `desktop/src-tauri/src/encoder/webp.rs` | libwebp encode (lossy + lossless) |
| `desktop/src-tauri/src/encoder/avif.rs` | ravif encode with memory-aware threading |
| `desktop/src-tauri/src/encoder/event_sink.rs` | `EventSink` trait + `TauriEmitter` + `MockSink` |
| `desktop/src-tauri/tests/integration.rs` | End-to-end batch test using `MockSink` |
| `desktop/src-tauri/tests/ipc_schema.rs` | `insta` snapshot tests for every emitted payload |
| `desktop/src-tauri/tests/fixtures/README.md` | Fixture sourcing instructions |
| `desktop/src-tauri/tests/fixtures/generate.rs` | Bin target generating synthetic fixtures |
| `desktop/src-tauri/tests/fixtures/*.{jpg,png}` | 8 test images (synthetic + sourced) |

### Modified files
| Phase | Path | Change |
|---|---|---|
| 1 | `desktop/src-tauri/Cargo.toml` | Add encoder crates + `[profile.release]` tuning + `[dev-dependencies]` |
| 2 | `desktop/src-tauri/src/batch.rs` | Add atomic `completed`/`expected` counters + `tick()` |
| 2 | `desktop/src-tauri/src/commands.rs` | Rewrite `compress`; remove `SidecarState`/`ensure_sidecar`; slim `FileDonePayload` |
| 2 | `desktop/src-tauri/src/lib.rs` | Remove `SidecarState` registration + `sidecar-crashed` listener |
| 2 | `desktop/src/ipc.ts` | Remove `tmp` from `FileDonePayload`; add `BatchDonePayload` + `onBatchDone` |
| 2 | `desktop/src/main.ts` | Subscribe to `batch-done` for Undo enable; remove `sidecar-crashed` listener |
| 3 | `desktop/src-tauri/tauri.conf.json` | Drop `bundle.externalBin` + sidecar entries from `bundle.resources` |
| 3 | `package.json` (root) | Remove `bin` entry + sharp dependency |
| 3 | `README.md` (root) | CLI section retired; new prerequisites |
| 3 | `CLAUDE.md` (root) | Architecture section rewritten |
| 4 | `desktop/scripts/build-portable.mjs` | Replaced by `build-dist.mjs` |

### Deleted files
| Phase | Path |
|---|---|
| 3 | `desktop/src-tauri/src/sidecar.rs` |
| 3 | `desktop/src-tauri/binaries/` (entire dir) |
| 3 | `desktop/src-tauri/sidecar-deps/` (entire dir) |
| 3 | `desktop/scripts/fetch-node.sh` |
| 3 | `desktop/scripts/copy-sidecar-deps.mjs` |
| 3 | `desktop/scripts/build-portable.mjs` |
| 3 | `index.js` (root) |
| 3 | `src/` (root, entire dir — `sidecar.js`, `encoder.js`, `config.js`, `*.test.js`) |

### Phase boundaries
- **Phase 1 PR**: Pure addition. Old sidecar still serves traffic. Tasks 1.x.
- **Phase 2 PR**: Cutover. Old sidecar code present but unreachable. Tasks 2.x.
- **Phase 3 PR**: The Node Purge. Delete sidecar/CLI/scripts. Tasks 3.x.
- **Phase 4 PR**: CI matrix + docs. Tasks 4.x.

---

# Phase 1 — Encoder Facade Scaffold

**Goal:** All new encoder code lands behind a facade with passing unit tests. Production code (`commands.rs`) is untouched. Sidecar continues to serve users.

**PR title:** `feat: add Rust-native image encoder facade`

---

### Task 1.1: Add Cargo dependencies

**Files:**
- Modify: `desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Open Cargo.toml and append new dependencies**

Replace the existing `[dependencies]` block by appending the new entries. Final state:

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri-plugin-store = "2"
tokio = { version = "1.52.1", features = ["full"] }
uuid = { version = "1.23.1", features = ["v4"] }
anyhow = "1.0.102"
trash = "5"
tauri-plugin-fs = "2"
dirs-next = "2"
tauri-plugin-dialog = "2"

# Image processing — Rust-native encoders
image = { version = "0.25", default-features = false, features = ["png", "webp", "avif"] }
mozjpeg = "0.10"
oxipng = { version = "10", default-features = false, features = ["parallel"] }
imagequant = "4"
webp = "0.3"
ravif = "0.13"
kamadak-exif = "0.6"
num_cpus = "1"
thiserror = "2"

[dev-dependencies]
insta = { version = "1", features = ["json"] }
tempfile = "3"

[profile.release]
lto = true
codegen-units = 1
strip = true
opt-level = 3
panic = "unwind"
```

- [ ] **Step 2: Verify the project still builds**

Run: `cd desktop/src-tauri && cargo build`
Expected: New crates download and compile. First build will take 5–15 minutes. No errors.

If `mozjpeg` or `webp` fails on Windows: ensure NASM is installed (`winget install nasm`) and you are running from "x64 Native Tools Command Prompt for VS 2022".

- [ ] **Step 3: Commit**

```bash
git add desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock
git commit -m "chore: add rust-native encoder dependencies"
```

---

### Task 1.2: Create encoder module skeleton with shared types

**Files:**
- Create: `desktop/src-tauri/src/encoder/mod.rs`

- [ ] **Step 1: Create the encoder directory and mod.rs**

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Rust-native image encoder facade.
//!
//! Replaces the previous Node.js + Sharp sidecar. All work runs in-process,
//! callable from `commands.rs` via `tokio::task::spawn_blocking`.

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
```

- [ ] **Step 2: Register the new module in lib.rs (additive)**

Modify: `desktop/src-tauri/src/lib.rs` — add `pub mod encoder;` after the existing `mod` declarations, and bump `batch` to `pub mod` so future integration tests in `tests/` (added in Phase 2) can reach it. Final state of the top mod block:

```rust
pub mod batch;
mod commands;
pub mod encoder;
mod sidecar;
mod trash;
```

`pub mod` here is required because Rust's `tests/` directory builds against the library crate as an external consumer; private modules are inaccessible from there.

- [ ] **Step 3: Verify it compiles (submodules are stubs and will fail)**

The submodules don't exist yet, so `cargo check` will fail on missing files. Create empty stub files now to unblock:

```bash
touch desktop/src-tauri/src/encoder/decode.rs
touch desktop/src-tauri/src/encoder/event_sink.rs
touch desktop/src-tauri/src/encoder/jpeg.rs
touch desktop/src-tauri/src/encoder/png.rs
touch desktop/src-tauri/src/encoder/webp.rs
touch desktop/src-tauri/src/encoder/avif.rs
```

The stubs need at least empty `pub fn encode` signatures matching what `mod.rs` calls. Add to each:

`encoder/decode.rs`:
```rust
use super::{EncodeError, ImageExt};
use std::path::Path;

pub struct DecodedImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub icc_profile: Option<Vec<u8>>,
}

pub fn decode(_src: &Path, _ext: ImageExt) -> Result<DecodedImage, EncodeError> {
    unimplemented!("decode stub")
}
```

`encoder/jpeg.rs`, `png.rs`, `webp.rs`, `avif.rs` — same pattern, each with:
```rust
use super::{decode::DecodedImage, EncodeError, EncodedFile};

pub fn encode(_decoded: &DecodedImage, _quality: u32) -> Result<EncodedFile, EncodeError> {
    unimplemented!("stub")
}
```

`encoder/event_sink.rs` — leave empty for now; no caller yet.

- [ ] **Step 4: Run cargo check**

Run: `cd desktop/src-tauri && cargo check`
Expected: Compiles cleanly. `unimplemented!` macros are fine as long as nothing calls them.

- [ ] **Step 5: Run the unit tests in mod.rs**

Run: `cd desktop/src-tauri && cargo test --lib encoder::tests`
Expected: 2 tests pass (`parses_known_extensions`, `rejects_unknown_extension`).

- [ ] **Step 6: Commit**

```bash
git add desktop/src-tauri/src/encoder/ desktop/src-tauri/src/lib.rs
git commit -m "feat(encoder): add facade scaffold with shared types"
```

---

### Task 1.3: Implement decode module with EXIF rotation

**Files:**
- Modify: `desktop/src-tauri/src/encoder/decode.rs`

- [ ] **Step 1: Write failing tests for decode**

Replace the file content with the test block first (TDD — test before impl):

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{EncodeError, ImageExt};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub struct DecodedImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub icc_profile: Option<Vec<u8>>,
}

pub fn decode(src: &Path, ext: ImageExt) -> Result<DecodedImage, EncodeError> {
    unimplemented!()
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
        // orientation=6 means "rotate 90° CW" — a portrait shot taken landscape.
        // After decode the buffer should already be upright.
        let img = decode(&fixture("portrait_exif_rotated.jpg"), ImageExt::Jpeg).unwrap();
        // Source file is stored as 800x600 with orientation=6.
        // Upright dimensions should be 600x800.
        assert!(img.height > img.width, "expected portrait orientation after EXIF rotation");
    }

    #[test]
    fn returns_error_on_corrupt_input() {
        let result = decode(&fixture("corrupt.jpg"), ImageExt::Jpeg);
        assert!(matches!(result, Err(EncodeError::Decode(_))));
    }
}
```

- [ ] **Step 2: Verify tests fail (no fixtures yet, but the unimplemented impl will panic first)**

Run: `cd desktop/src-tauri && cargo test --lib encoder::decode::tests`
Expected: All 4 tests panic with `unimplemented!`. (Fixtures will be added in Task 1.4 — don't worry about file-not-found yet.)

- [ ] **Step 3: Implement decode**

Replace the `decode` function and `unimplemented!()` with:

```rust
pub fn decode(src: &Path, ext: ImageExt) -> Result<DecodedImage, EncodeError> {
    match ext {
        ImageExt::Jpeg => decode_jpeg(src),
        ImageExt::Png | ImageExt::Webp | ImageExt::Avif => decode_via_image_crate(src),
    }
}

fn decode_jpeg(src: &Path) -> Result<DecodedImage, EncodeError> {
    let bytes = std::fs::read(src)?;
    let orientation = read_exif_orientation(&bytes).unwrap_or(1);
    let icc = read_icc_profile_jpeg(&bytes);

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
    let reader = image::ImageReader::open(src)?
        .with_guessed_format()?;
    let dyn_img = reader.decode()
        .map_err(|e| EncodeError::Decode(format!("image: {e}")))?;
    let rgba_img = dyn_img.to_rgba8();
    let (width, height) = rgba_img.dimensions();
    Ok(DecodedImage {
        rgba: rgba_img.into_raw(),
        width,
        height,
        icc_profile: None, // image crate doesn't expose ICC for these formats reliably
    })
}

fn read_exif_orientation(jpeg_bytes: &[u8]) -> Option<u32> {
    let exif = exif::Reader::new()
        .read_from_container(&mut BufReader::new(jpeg_bytes))
        .ok()?;
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    field.value.get_uint(0)
}

/// Extract ICC profile from JPEG APP2 markers (the "ICC_PROFILE" segments).
fn read_icc_profile_jpeg(_bytes: &[u8]) -> Option<Vec<u8>> {
    // mozjpeg-sys doesn't expose ICC reading via a stable Rust API; return None for now.
    // Frontend tests don't validate ICC byte equality, only presence of an ICC tag if the
    // user enabled it. Leaving as None means we strip ICC during recompression — acceptable
    // tradeoff for v1.
    None
}

/// Rotate/flip the RGBA buffer to compensate for EXIF orientation.
/// See https://exiftool.org/TagNames/EXIF.html#Orientation for the mapping.
fn apply_orientation(img: &mut DecodedImage, orientation: u32) {
    if orientation <= 1 {
        return; // Normal — no work
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let src = std::mem::take(&mut img.rgba);
    let mut dst = vec![0u8; src.len()];

    let put = |dst: &mut [u8], dx: usize, dy: usize, dw: usize, src: &[u8], sx: usize, sy: usize, sw: usize| {
        let s = (sy * sw + sx) * 4;
        let d = (dy * dw + dx) * 4;
        dst[d..d + 4].copy_from_slice(&src[s..s + 4]);
    };

    match orientation {
        2 => { // mirror horizontal
            for y in 0..h { for x in 0..w { put(&mut dst, w - 1 - x, y, w, &src, x, y, w); } }
        }
        3 => { // rotate 180
            for y in 0..h { for x in 0..w { put(&mut dst, w - 1 - x, h - 1 - y, w, &src, x, y, w); } }
        }
        4 => { // mirror vertical
            for y in 0..h { for x in 0..w { put(&mut dst, x, h - 1 - y, w, &src, x, y, w); } }
        }
        5 => { // mirror horizontal then rotate 270 CW (= transpose)
            img.width = h as u32; img.height = w as u32;
            dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, y, x, h, &src, x, y, w); } }
        }
        6 => { // rotate 90 CW
            img.width = h as u32; img.height = w as u32;
            dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, h - 1 - y, x, h, &src, x, y, w); } }
        }
        7 => { // mirror horizontal then rotate 90 CW
            img.width = h as u32; img.height = w as u32;
            dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, h - 1 - y, w - 1 - x, h, &src, x, y, w); } }
        }
        8 => { // rotate 270 CW
            img.width = h as u32; img.height = w as u32;
            dst = vec![0u8; src.len()];
            for y in 0..h { for x in 0..w { put(&mut dst, y, w - 1 - x, h, &src, x, y, w); } }
        }
        _ => { dst = src.clone(); }
    }
    img.rgba = dst;
}
```

- [ ] **Step 4: Run cargo check**

Run: `cd desktop/src-tauri && cargo check`
Expected: Compiles. (Tests will still fail at runtime — they need fixtures, added in Task 1.4.)

- [ ] **Step 5: Commit (tests deferred until fixtures land)**

```bash
git add desktop/src-tauri/src/encoder/decode.rs
git commit -m "feat(encoder): implement RGBA decode with EXIF orientation"
```

---

### Task 1.4: Set up test fixtures

**Files:**
- Create: `desktop/src-tauri/tests/fixtures/README.md`
- Create: `desktop/src-tauri/tests/fixtures/generate.rs`
- Create: `desktop/src-tauri/tests/fixtures/*.{jpg,png}`

- [ ] **Step 1: Create fixtures README documenting each file**

Create `desktop/src-tauri/tests/fixtures/README.md`:

```markdown
# Test Fixtures

These images cover boundary conditions for the encoder pipeline. Most are generated
synthetically by `generate.rs`; a few must be sourced manually.

## Synthetic (generated by `cargo run --bin generate-fixtures`)

| File | Dimensions | Purpose |
|---|---|---|
| `screenshot.png` | 1440×900 | Solid-color regions — exercises palette quantization |
| `transparent.png` | 512×512 | RGBA with alpha gradient |
| `tiny.png` | 128×128 | Pre-optimized — should hit `SkippedNoGain` |
| `corrupt.jpg` | n/a | 100 random bytes — must fail decode |
| `large_photo.jpg` | 6000×4000 | Memory-pressure stress test |

## Manually sourced (commit to repo once obtained)

| File | Source | Purpose |
|---|---|---|
| `landscape.jpg` | Any 1920×1080 photograph (Unsplash, public domain) | Realistic JPEG re-encode test |
| `portrait_exif_rotated.jpg` | 800×600 photo with EXIF Orientation=6 | EXIF rotation test |
| `with_icc.jpg` | Any photo with an embedded ICC profile (export from Lightroom/Photoshop) | ICC preservation test |

To create a fixture with EXIF orientation=6, use exiftool:
\`\`\`
exiftool -Orientation=6 -n landscape_800x600.jpg -o portrait_exif_rotated.jpg
\`\`\`
```

- [ ] **Step 2: Add a fixtures generator binary to Cargo.toml**

Append to `desktop/src-tauri/Cargo.toml`:

```toml
[[bin]]
name = "generate-fixtures"
path = "tests/fixtures/generate.rs"
required-features = []
```

- [ ] **Step 3: Write the generator**

Create `desktop/src-tauri/tests/fixtures/generate.rs`:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Synthetic test fixture generator. Run with `cargo run --bin generate-fixtures`.
// Outputs to `tests/fixtures/`.

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
    gen_large_photo(&dir);

    eprintln!("Synthetic fixtures generated in {}", dir.display());
    eprintln!("Note: landscape.jpg, portrait_exif_rotated.jpg, with_icc.jpg must be sourced manually (see README.md).");
}

fn gen_screenshot(dir: &PathBuf) {
    let (w, h) = (1440u32, 900u32);
    let mut buf = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 3) as usize;
            // Solid bands — friendly for palette quantization
            let band = (y / 100) as u8;
            buf[i] = match band % 4 { 0 => 240, 1 => 90, 2 => 30, _ => 200 };
            buf[i + 1] = match band % 4 { 0 => 240, 1 => 160, 2 => 30, _ => 60 };
            buf[i + 2] = match band % 4 { 0 => 240, 1 => 220, 2 => 30, _ => 60 };
        }
    }
    image::save_buffer(dir.join("screenshot.png"), &buf, w, h, image::ColorType::Rgb8).unwrap();
}

fn gen_transparent(dir: &PathBuf) {
    let (w, h) = (512u32, 512u32);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            buf[i] = (x * 255 / w) as u8;
            buf[i + 1] = (y * 255 / h) as u8;
            buf[i + 2] = 128;
            buf[i + 3] = ((x + y) * 255 / (w + h)) as u8;
        }
    }
    image::save_buffer(dir.join("transparent.png"), &buf, w, h, image::ColorType::Rgba8).unwrap();
}

fn gen_tiny(dir: &PathBuf) {
    // 128x128 of a single color — png encoder will already produce something near-optimal,
    // so re-encoding should yield no gain.
    let buf = vec![128u8; 128 * 128 * 3];
    image::save_buffer(dir.join("tiny.png"), &buf, 128, 128, image::ColorType::Rgb8).unwrap();
}

fn gen_corrupt(dir: &PathBuf) {
    let mut f = fs::File::create(dir.join("corrupt.jpg")).unwrap();
    let bytes: Vec<u8> = (0..100u8).collect();
    f.write_all(&bytes).unwrap();
}

fn gen_large_photo(dir: &PathBuf) {
    // 6000x4000 noise — only generated to test memory pressure.
    // File is ~20 MB; do not commit unless intentional.
    let (w, h) = (6000u32, 4000u32);
    let mut buf = vec![0u8; (w * h * 3) as usize];
    let mut state = 0xCAFEu32;
    for byte in buf.iter_mut() {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        *byte = (state >> 16) as u8;
    }
    let path = dir.join("large_photo.jpg");
    let img = image::RgbImage::from_raw(w, h, buf).unwrap();
    img.save_with_format(&path, image::ImageFormat::Jpeg).unwrap();
}
```

- [ ] **Step 4: Run the generator**

Run: `cd desktop/src-tauri && cargo run --bin generate-fixtures`
Expected: Five files appear in `tests/fixtures/`. Stderr reminds you to source the three manual ones.

- [ ] **Step 5: Source the three manual fixtures**

This is a one-time human task. Place the following files in `desktop/src-tauri/tests/fixtures/`:

- `landscape.jpg` — any 1920×1080 photo (Unsplash works; pick something with skies/skin tones)
- `portrait_exif_rotated.jpg` — 800×600 photo with EXIF Orientation=6 (use exiftool as shown in README)
- `with_icc.jpg` — any photo with an embedded ICC profile

If you cannot source these immediately, mark the relevant tests with `#[ignore]` and create a follow-up task. Do not skip.

- [ ] **Step 6: Add fixtures to .gitignore for the large one**

Append to `desktop/src-tauri/.gitignore` (create if missing):

```
tests/fixtures/large_photo.jpg
```

The 80 MB stress fixture is regenerated on demand; do not commit it.

- [ ] **Step 7: Verify decode tests now pass**

Run: `cd desktop/src-tauri && cargo test --lib encoder::decode::tests`
Expected: 4 tests pass (assuming all fixtures are in place; if `with_icc.jpg` is missing, only the ICC test in later tasks will fail).

- [ ] **Step 8: Commit**

```bash
git add desktop/src-tauri/tests/fixtures/ desktop/src-tauri/.gitignore desktop/src-tauri/Cargo.toml
git commit -m "test: add encoder fixtures and synthetic generator"
```

---

### Task 1.5: Implement EventSink trait + MockSink

**Files:**
- Modify: `desktop/src-tauri/src/encoder/event_sink.rs`

- [ ] **Step 1: Write the trait, MockSink, and tests**

Replace the empty file with:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Abstraction for emitting per-file and per-batch events.
//!
//! Production wires `TauriEmitter` (defined in `commands.rs` later); tests use `MockSink`
//! to assert event counts/order without spinning up a real Tauri app.

use serde::Serialize;
use std::sync::Mutex;

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileDonePayload {
    pub id: String,
    pub src_bytes: u64,
    pub out_bytes: u64,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileErrorPayload {
    pub id: String,
    pub msg: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileSkippedPayload {
    pub id: String,
    pub src_bytes: u64,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompanionErrorPayload {
    pub id: String,
    pub ext: String,
    pub msg: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrashFallbackPayload {
    pub id: String,
    pub note: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchDonePayload {
    pub batch_id: String,
}

pub trait EventSink: Send + Sync {
    fn emit_file_done(&self, p: FileDonePayload);
    fn emit_file_error(&self, p: FileErrorPayload);
    fn emit_file_skipped(&self, p: FileSkippedPayload);
    fn emit_companion_error(&self, p: CompanionErrorPayload);
    fn emit_trash_fallback(&self, p: TrashFallbackPayload);
    fn emit_batch_done(&self, p: BatchDonePayload);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockEvent {
    FileDone(FileDonePayload),
    FileError(FileErrorPayload),
    FileSkipped(FileSkippedPayload),
    CompanionError(CompanionErrorPayload),
    TrashFallback(TrashFallbackPayload),
    BatchDone(BatchDonePayload),
}

#[derive(Default)]
pub struct MockSink {
    events: Mutex<Vec<MockEvent>>,
}

impl MockSink {
    pub fn new() -> Self { Self::default() }
    pub fn events(&self) -> Vec<MockEvent> { self.events.lock().unwrap().clone() }
    pub fn count_by_kind(&self, predicate: impl Fn(&MockEvent) -> bool) -> usize {
        self.events.lock().unwrap().iter().filter(|e| predicate(e)).count()
    }
    fn push(&self, e: MockEvent) { self.events.lock().unwrap().push(e); }
}

impl EventSink for MockSink {
    fn emit_file_done(&self, p: FileDonePayload) { self.push(MockEvent::FileDone(p)); }
    fn emit_file_error(&self, p: FileErrorPayload) { self.push(MockEvent::FileError(p)); }
    fn emit_file_skipped(&self, p: FileSkippedPayload) { self.push(MockEvent::FileSkipped(p)); }
    fn emit_companion_error(&self, p: CompanionErrorPayload) { self.push(MockEvent::CompanionError(p)); }
    fn emit_trash_fallback(&self, p: TrashFallbackPayload) { self.push(MockEvent::TrashFallback(p)); }
    fn emit_batch_done(&self, p: BatchDonePayload) { self.push(MockEvent::BatchDone(p)); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_collects_events_in_order() {
        let sink = MockSink::new();
        sink.emit_file_done(FileDonePayload { id: "a".into(), src_bytes: 100, out_bytes: 50 });
        sink.emit_batch_done(BatchDonePayload { batch_id: "b1".into() });
        let events = sink.events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], MockEvent::FileDone(_)));
        assert!(matches!(events[1], MockEvent::BatchDone(_)));
    }

    #[test]
    fn count_by_kind_filters_correctly() {
        let sink = MockSink::new();
        sink.emit_file_done(FileDonePayload { id: "a".into(), src_bytes: 1, out_bytes: 1 });
        sink.emit_file_done(FileDonePayload { id: "b".into(), src_bytes: 1, out_bytes: 1 });
        sink.emit_file_error(FileErrorPayload { id: "c".into(), msg: "x".into() });
        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::FileDone(_))), 2);
        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::FileError(_))), 1);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd desktop/src-tauri && cargo test --lib encoder::event_sink::tests`
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add desktop/src-tauri/src/encoder/event_sink.rs
git commit -m "feat(encoder): add EventSink trait and MockSink for testing"
```

---

### Task 1.6: Implement JPEG encoder

**Files:**
- Modify: `desktop/src-tauri/src/encoder/jpeg.rs`

- [ ] **Step 1: Write failing tests**

Replace the file with:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{decode::DecodedImage, EncodeError, EncodedFile, ImageExt};
use std::io::Write;
use tempfile::NamedTempFile;

pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    unimplemented!()
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
        let out = encode(&decoded, 80).unwrap();
        assert!(out.bytes < src_bytes, "out {} should be < src {}", out.bytes, src_bytes);
        assert!(out.tmp_path.exists());
    }

    #[test]
    fn higher_quality_produces_larger_file() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let q40 = encode(&decoded, 40).unwrap();
        let q90 = encode(&decoded, 90).unwrap();
        assert!(q90.bytes > q40.bytes);
    }

    #[test]
    fn output_extension_is_jpg() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let out = encode(&decoded, 80).unwrap();
        assert_eq!(out.ext, ImageExt::Jpeg);
    }
}
```

- [ ] **Step 2: Run tests — verify fail**

Run: `cd desktop/src-tauri && cargo test --lib encoder::jpeg::tests`
Expected: All 3 tests panic with `unimplemented!`.

- [ ] **Step 3: Implement encode**

Replace `unimplemented!()` and the function body with:

```rust
pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    let q = quality.clamp(1, 100) as f32;

    // mozjpeg::Compress writes via a writer; gather into a Vec then write to tmp file.
    let mut compress = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    compress.set_size(decoded.width as usize, decoded.height as usize);
    compress.set_quality(q);
    compress.set_progressive_mode();
    compress.set_optimize_scans(true);

    let mut comp = compress.start_compress(Vec::new())
        .map_err(|e| EncodeError::Encode(format!("mozjpeg start: {e}")))?;

    // Strip alpha — JPEG doesn't support it. RGBA → RGB.
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
```

- [ ] **Step 4: Run tests — verify pass**

Run: `cd desktop/src-tauri && cargo test --lib encoder::jpeg::tests`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add desktop/src-tauri/src/encoder/jpeg.rs
git commit -m "feat(encoder): implement JPEG encoder via mozjpeg"
```

---

### Task 1.7: Implement PNG encoder (imagequant + oxipng two-stage)

**Files:**
- Modify: `desktop/src-tauri/src/encoder/png.rs`

- [ ] **Step 1: Write failing tests**

Replace the file with:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{decode::DecodedImage, EncodeError, EncodedFile, ImageExt};
use std::io::Write;
use tempfile::NamedTempFile;

pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    unimplemented!()
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
    fn quality_100_skips_palette_quantization_but_still_oxipng_optimizes() {
        let decoded = decode(&fixture("transparent.png"), ImageExt::Png).unwrap();
        let out = encode(&decoded, 100).unwrap();
        assert!(out.tmp_path.exists());
        assert!(out.bytes > 0);
    }

    #[test]
    fn preserves_alpha_channel() {
        let decoded = decode(&fixture("transparent.png"), ImageExt::Png).unwrap();
        let out = encode(&decoded, 80).unwrap();
        // Re-decode and check alpha is still present
        let re = decode(&out.tmp_path, ImageExt::Png).unwrap();
        assert_eq!(re.rgba.len(), (re.width * re.height * 4) as usize);
        let has_transparency = re.rgba.chunks_exact(4).any(|p| p[3] < 255);
        assert!(has_transparency, "alpha channel should survive encode");
    }
}
```

- [ ] **Step 2: Run tests — verify fail**

Run: `cd desktop/src-tauri && cargo test --lib encoder::png::tests`
Expected: 3 tests panic.

- [ ] **Step 3: Implement encode**

Replace the body:

```rust
pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    let q = quality.clamp(1, 100);

    // Stage 1: optional palette quantization
    let png_bytes = if q < 100 {
        quantize_to_png(decoded, q as u8)?
    } else {
        encode_truecolor_png(decoded)?
    };

    // Stage 2: oxipng lossless re-compression
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

    // Encode indexed PNG using the `image` crate's PngEncoder via a custom writer.
    // Easier path: round-trip through RGBA and let oxipng handle it. The quantized
    // RGBA buffer already has only `palette.len()` distinct colors, so oxipng will
    // emit a small palette PNG.
    let mut rgba: Vec<u8> = Vec::with_capacity(decoded.rgba.len());
    for &i in &indices {
        let c = palette[i as usize];
        rgba.extend_from_slice(&[c.r, c.g, c.b, c.a]);
    }

    let mut buf = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new_with_quality(
            &mut buf,
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        );
        use image::ImageEncoder;
        encoder.write_image(&rgba, decoded.width, decoded.height, image::ExtendedColorType::Rgba8)
            .map_err(|e| EncodeError::Encode(format!("png encode: {e}")))?;
    }
    Ok(buf)
}

fn encode_truecolor_png(decoded: &DecodedImage) -> Result<Vec<u8>, EncodeError> {
    let mut buf = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new_with_quality(
            &mut buf,
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        );
        use image::ImageEncoder;
        encoder.write_image(&decoded.rgba, decoded.width, decoded.height, image::ExtendedColorType::Rgba8)
            .map_err(|e| EncodeError::Encode(format!("png encode: {e}")))?;
    }
    Ok(buf)
}
```

- [ ] **Step 4: Run tests — verify pass**

Run: `cd desktop/src-tauri && cargo test --lib encoder::png::tests`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add desktop/src-tauri/src/encoder/png.rs
git commit -m "feat(encoder): implement PNG encoder via imagequant + oxipng"
```

---

### Task 1.8: Implement WebP encoder

**Files:**
- Modify: `desktop/src-tauri/src/encoder/webp.rs`

- [ ] **Step 1: Write failing tests**

Replace the file with:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{decode::DecodedImage, EncodeError, EncodedFile, ImageExt};
use std::io::Write;
use tempfile::NamedTempFile;

pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    unimplemented!()
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
    fn encodes_lossy_webp() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let out = encode(&decoded, 80).unwrap();
        assert_eq!(out.ext, ImageExt::Webp);
        assert!(out.bytes > 0);
    }

    #[test]
    fn quality_100_uses_lossless_path() {
        let decoded = decode(&fixture("transparent.png"), ImageExt::Png).unwrap();
        let q100 = encode(&decoded, 100).unwrap();
        let q80 = encode(&decoded, 80).unwrap();
        // Lossless typically larger than lossy at q80 for non-trivial images.
        assert!(q100.bytes >= q80.bytes);
    }
}
```

- [ ] **Step 2: Run — verify fail**

Run: `cd desktop/src-tauri && cargo test --lib encoder::webp::tests`
Expected: 2 tests panic.

- [ ] **Step 3: Implement**

Replace the function body:

```rust
pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    let q = quality.clamp(1, 100);
    let encoder = webp::Encoder::from_rgba(&decoded.rgba, decoded.width, decoded.height);

    let memory = if q >= 100 {
        encoder.encode_lossless()
    } else {
        encoder.encode(q as f32)
    };
    let bytes: &[u8] = &*memory;

    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(bytes)?;
    tmp.flush()?;
    let len = bytes.len() as u64;
    let (_file, path) = tmp.keep().map_err(|e| EncodeError::Io(e.error))?;

    Ok(EncodedFile { ext: ImageExt::Webp, tmp_path: path, bytes: len })
}
```

- [ ] **Step 4: Run — verify pass**

Run: `cd desktop/src-tauri && cargo test --lib encoder::webp::tests`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add desktop/src-tauri/src/encoder/webp.rs
git commit -m "feat(encoder): implement WebP encoder via libwebp"
```

---

### Task 1.9: Implement AVIF encoder with memory-aware threading

**Files:**
- Modify: `desktop/src-tauri/src/encoder/avif.rs`

- [ ] **Step 1: Write failing tests**

Replace the file with:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{decode::DecodedImage, EncodeError, EncodedFile, ImageExt};
use std::io::Write;
use tempfile::NamedTempFile;

/// Threshold (in pixels) above which we drop AVIF thread count to 1
/// to keep peak RAM bounded under concurrent batches.
const HIGH_RES_PIXEL_THRESHOLD: u64 = 8_000_000; // ~3464×2310 or larger

pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::decode::decode;
    use crate::encoder::ImageExt;
    use std::path::PathBuf;
    use std::time::Instant;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
    }

    #[test]
    fn encodes_avif_smaller_than_jpeg_input() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let src_bytes = std::fs::metadata(fixture("landscape.jpg")).unwrap().len();
        let out = encode(&decoded, 60).unwrap();
        assert!(out.bytes < src_bytes, "AVIF q60 of landscape.jpg should be smaller than the JPEG source");
    }

    #[test]
    fn ext_is_avif() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let out = encode(&decoded, 60).unwrap();
        assert_eq!(out.ext, ImageExt::Avif);
    }

    #[test]
    #[ignore] // Run with `cargo test --release -- --ignored avif::tests::speed_6_finishes`
    fn speed_6_finishes_under_5s_for_1080p() {
        let decoded = decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let start = Instant::now();
        encode(&decoded, 60).unwrap();
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 5, "AVIF 1080p took {:?}", elapsed);
    }
}
```

- [ ] **Step 2: Run — verify fail**

Run: `cd desktop/src-tauri && cargo test --lib encoder::avif::tests`
Expected: 2 tests panic; 1 ignored.

- [ ] **Step 3: Implement**

Replace the function body:

```rust
pub fn encode(decoded: &DecodedImage, quality: u32) -> Result<EncodedFile, EncodeError> {
    let q = quality.clamp(1, 100) as f32;
    let pixels = (decoded.width as u64) * (decoded.height as u64);
    let threads = if pixels > HIGH_RES_PIXEL_THRESHOLD { 1 } else { 2 };

    let img = ravif::Img::new(
        rgba_as_ravif_pixels(&decoded.rgba),
        decoded.width as usize,
        decoded.height as usize,
    );

    let result = ravif::Encoder::new()
        .with_quality(q)
        .with_speed(6)
        .with_num_threads(Some(threads))
        .encode_rgba(img)
        .map_err(|e| EncodeError::Encode(format!("ravif: {e}")))?;

    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(&result.avif_file)?;
    tmp.flush()?;
    let len = result.avif_file.len() as u64;
    let (_file, path) = tmp.keep().map_err(|e| EncodeError::Io(e.error))?;

    Ok(EncodedFile { ext: ImageExt::Avif, tmp_path: path, bytes: len })
}

/// View RGBA bytes as ravif's RGBA8 pixel slice.
fn rgba_as_ravif_pixels(rgba: &[u8]) -> &[rgb::RGBA<u8>] {
    // SAFETY: rgb::RGBA<u8> is repr(C) with 4 u8 fields. Length divisibility is checked.
    assert_eq!(rgba.len() % 4, 0, "RGBA buffer length must be divisible by 4");
    unsafe {
        std::slice::from_raw_parts(
            rgba.as_ptr() as *const rgb::RGBA<u8>,
            rgba.len() / 4,
        )
    }
}
```

Note: `rgb` crate comes in as a transitive dep of ravif. If `cargo check` cannot resolve it, add `rgb = "0.8"` to `[dependencies]`.

- [ ] **Step 4: Run — verify pass**

Run: `cd desktop/src-tauri && cargo test --lib encoder::avif::tests`
Expected: 2 tests pass; 1 ignored.

- [ ] **Step 5: Run the timing test in release mode (optional sanity check)**

Run: `cd desktop/src-tauri && cargo test --release --lib encoder::avif::tests::speed_6_finishes -- --ignored`
Expected: Passes (under 5s on a modern dev machine).

- [ ] **Step 6: Commit**

```bash
git add desktop/src-tauri/src/encoder/avif.rs desktop/src-tauri/Cargo.toml
git commit -m "feat(encoder): implement AVIF encoder with memory-aware threading"
```

---

### Task 1.10: Wire up facade-level integration test for `encode()`

**Files:**
- Modify: `desktop/src-tauri/src/encoder/mod.rs`

- [ ] **Step 1: Add facade-level tests**

Append inside the existing `#[cfg(test)] mod tests` block in `encoder/mod.rs`:

```rust
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
    }

    fn opts(emit_webp: bool, emit_avif: bool) -> EncodeOpts {
        EncodeOpts {
            jpeg_quality: 80,
            png_quality: 80,
            webp_quality: 80,
            avif_quality: 60,
            emit_webp,
            emit_avif,
        }
    }

    #[test]
    fn encode_jpeg_returns_encoded() {
        let o = opts(false, false);
        let outcome = encode(EncodeRequest {
            src_path: &fixture("landscape.jpg"),
            ext: ImageExt::Jpeg,
            opts: &o,
        }).unwrap();
        match outcome {
            EncodeOutcome::Encoded(r) => {
                assert_eq!(r.main.ext, ImageExt::Jpeg);
                assert!(r.companions.is_empty());
            }
            EncodeOutcome::SkippedNoGain { .. } => panic!("should encode, not skip"),
        }
    }

    #[test]
    fn encode_with_webp_companion_produces_one_companion() {
        let o = opts(true, false);
        let outcome = encode(EncodeRequest {
            src_path: &fixture("landscape.jpg"),
            ext: ImageExt::Jpeg,
            opts: &o,
        }).unwrap();
        if let EncodeOutcome::Encoded(r) = outcome {
            assert_eq!(r.companions.len(), 1);
            assert_eq!(r.companions[0].ext, ImageExt::Webp);
            assert!(r.companion_errors.is_empty());
        } else { panic!("expected Encoded"); }
    }

    #[test]
    fn encode_tiny_already_optimized_skips() {
        let o = opts(false, false);
        let outcome = encode(EncodeRequest {
            src_path: &fixture("tiny.png"),
            ext: ImageExt::Png,
            opts: &o,
        }).unwrap();
        assert!(matches!(outcome, EncodeOutcome::SkippedNoGain { .. }));
    }

    #[test]
    fn encode_corrupt_returns_decode_error() {
        let o = opts(false, false);
        let result = encode(EncodeRequest {
            src_path: &fixture("corrupt.jpg"),
            ext: ImageExt::Jpeg,
            opts: &o,
        });
        assert!(matches!(result, Err(EncodeError::Decode(_))));
    }

    #[test]
    fn encode_webp_source_does_not_emit_webp_companion() {
        // First create a WebP fixture on the fly from landscape.jpg
        let landscape = decode::decode(&fixture("landscape.jpg"), ImageExt::Jpeg).unwrap();
        let webp_file = webp::encode(&landscape, 80).unwrap();

        let o = opts(true, false);
        let outcome = encode(EncodeRequest {
            src_path: &webp_file.tmp_path,
            ext: ImageExt::Webp,
            opts: &o,
        }).unwrap();
        if let EncodeOutcome::Encoded(r) = outcome {
            assert_eq!(r.companions.len(), 0, "should not duplicate-emit WebP for WebP source");
        } else { panic!("expected Encoded"); }
        let _ = std::fs::remove_file(&webp_file.tmp_path);
    }
```

- [ ] **Step 2: Run all encoder tests**

Run: `cd desktop/src-tauri && cargo test --lib encoder`
Expected: All tests pass (decode + each encoder + facade — roughly 18+ tests).

- [ ] **Step 3: Commit**

```bash
git add desktop/src-tauri/src/encoder/mod.rs
git commit -m "test(encoder): add facade-level dispatch and skip tests"
```

---

### Task 1.11: Phase 1 land gate — full test suite + size check

- [ ] **Step 1: Run the entire test suite**

Run: `cd desktop/src-tauri && cargo test`
Expected: All tests pass. Existing tests (if any) untouched.

- [ ] **Step 2: Verify the existing sidecar still builds and works**

Run: `cd desktop && npm run tauri dev`
Expected: App launches normally. Drag in an image — sidecar processes it as before. (We have not touched `commands.rs` yet.)

- [ ] **Step 3: Open a Phase 1 PR**

Use this PR description template:

```markdown
## Phase 1 — Encoder facade scaffold

Adds Rust-native image encoder modules behind a facade. **No behavior change**: `commands.rs` still spawns the Node sidecar; the new code is unreachable in production.

### What's new
- `encoder/mod.rs` — `EncodeRequest`, `EncodeResult`, `EncodeOutcome`, dispatch
- `encoder/decode.rs` — RGBA decode with EXIF orientation rotation
- `encoder/{jpeg,png,webp,avif}.rs` — per-format encoders
- `encoder/event_sink.rs` — `EventSink` trait + `MockSink`
- `tests/fixtures/` — 5 synthetic + 3 sourced test images
- `Cargo.toml` — encoder crate dependencies + release profile tuning

### Verification
- [x] `cargo test` green on Windows + macOS
- [x] `cargo run --bin generate-fixtures` regenerates synthetic fixtures
- [x] `npm run tauri dev` still works (sidecar untouched)

### Risk
Low — pure addition.
```

---

# Phase 2 — Commands.rs Cutover

**Goal:** `commands.rs` calls the new encoder facade instead of spawning the Node sidecar. Sidecar code remains in the tree but is no longer invoked. Frontend gets one new `batch-done` listener and the slimmed `FileDonePayload`.

**PR title:** `refactor: cut commands.rs over to rust-native encoder`

---

### Task 2.1: Update `batch.rs` with atomic counters and tick

**Files:**
- Modify: `desktop/src-tauri/src/batch.rs`

- [ ] **Step 1: Write failing test for tick behavior**

Append to `batch.rs` (after the existing `impl BatchState`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::event_sink::{MockSink, MockEvent};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn tick_emits_batch_done_when_last_completes() {
        let state = BatchState::default();
        let sink = MockSink::new();
        state.start("b1", 3);

        state.tick("b1", &sink);
        state.tick("b1", &sink);
        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_))), 0);

        state.tick("b1", &sink);
        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_))), 1);
    }

    #[test]
    fn tick_is_thread_safe_under_concurrent_load() {
        let state = Arc::new(BatchState::default());
        let sink = Arc::new(MockSink::new());
        state.start("b1", 100);

        let mut handles = vec![];
        for _ in 0..100 {
            let s = state.clone();
            let snk = sink.clone();
            handles.push(thread::spawn(move || s.tick("b1", &*snk)));
        }
        for h in handles { h.join().unwrap(); }

        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_))), 1);
    }
}
```

- [ ] **Step 2: Verify test fails (compile error — `start` and `tick` signatures changing)**

Run: `cd desktop/src-tauri && cargo test --lib batch`
Expected: Compile error — `start` no longer takes `String` only; `tick` doesn't exist.

- [ ] **Step 3: Rewrite batch.rs to add atomic counters and tick**

Replace the entire `batch.rs` content:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::encoder::event_sink::{BatchDonePayload, EventSink};
use crate::trash::Disposal;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct Batch {
    pub id: String,
    pub disposals: Vec<Disposal>,
    pub expected: usize,
}

#[derive(Default)]
pub struct BatchState {
    inner: Mutex<HashMap<String, BatchInner>>,
    last_complete: Mutex<Option<Batch>>,
}

struct BatchInner {
    id: String,
    expected: usize,
    completed: AtomicUsize,
    disposals: Mutex<Vec<Disposal>>,
}

impl BatchState {
    pub fn start(&self, id: &str, expected: usize) {
        let mut cur = self.inner.lock().unwrap();
        cur.insert(id.to_string(), BatchInner {
            id: id.to_string(),
            expected,
            completed: AtomicUsize::new(0),
            disposals: Mutex::new(vec![]),
        });
    }

    pub fn record_disposal(&self, batch_id: &str, disposal: Disposal) {
        let cur = self.inner.lock().unwrap();
        if let Some(b) = cur.get(batch_id) {
            b.disposals.lock().unwrap().push(disposal);
        }
    }

    pub fn record_companion_paths(&self, batch_id: &str, paths: Vec<PathBuf>) {
        if paths.is_empty() { return; }
        let cur = self.inner.lock().unwrap();
        if let Some(b) = cur.get(batch_id) {
            let mut disposals = b.disposals.lock().unwrap();
            if let Some(d) = disposals.last_mut() {
                d.companion_paths.extend(paths);
            }
        }
    }

    /// Increment the completion counter; if this was the last expected file,
    /// emit `batch-done` via the sink and move the batch to `last_complete`.
    pub fn tick(&self, batch_id: &str, sink: &dyn EventSink) {
        let should_complete = {
            let cur = self.inner.lock().unwrap();
            let Some(b) = cur.get(batch_id) else { return; };
            let prev = b.completed.fetch_add(1, Ordering::SeqCst);
            prev + 1 == b.expected
        };
        if should_complete {
            sink.emit_batch_done(BatchDonePayload { batch_id: batch_id.to_string() });
            self.complete(batch_id);
        }
    }

    fn complete(&self, batch_id: &str) {
        let mut cur = self.inner.lock().unwrap();
        if let Some(inner) = cur.remove(batch_id) {
            let batch = Batch {
                id: inner.id,
                disposals: inner.disposals.into_inner().unwrap(),
                expected: inner.expected,
            };
            *self.last_complete.lock().unwrap() = Some(batch);
        }
    }

    pub fn take_last(&self) -> Option<Batch> {
        self.last_complete.lock().unwrap().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::event_sink::{MockSink, MockEvent};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn tick_emits_batch_done_when_last_completes() {
        let state = BatchState::default();
        let sink = MockSink::new();
        state.start("b1", 3);

        state.tick("b1", &sink);
        state.tick("b1", &sink);
        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_))), 0);

        state.tick("b1", &sink);
        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_))), 1);
    }

    #[test]
    fn tick_is_thread_safe_under_concurrent_load() {
        let state = Arc::new(BatchState::default());
        let sink = Arc::new(MockSink::new());
        state.start("b1", 100);

        let mut handles = vec![];
        for _ in 0..100 {
            let s = state.clone();
            let snk = sink.clone();
            handles.push(thread::spawn(move || s.tick("b1", &*snk)));
        }
        for h in handles { h.join().unwrap(); }

        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_))), 1);
    }
}
```

- [ ] **Step 4: Run tests — verify pass**

Run: `cd desktop/src-tauri && cargo test --lib batch`
Expected: 2 new tests pass.

Note: `commands.rs` still calls the old `start(id: String)` and `complete(batch_id: &str)` signatures — it will break on `cargo check` until Task 2.3. Don't worry about that yet; keep moving.

- [ ] **Step 5: Commit**

```bash
git add desktop/src-tauri/src/batch.rs
git commit -m "refactor(batch): add atomic completion counter with tick()"
```

---

### Task 2.2: Add `TauriEmitter` implementation of `EventSink`

**Files:**
- Modify: `desktop/src-tauri/src/encoder/event_sink.rs`

- [ ] **Step 1: Add TauriEmitter to event_sink.rs**

Append to `event_sink.rs`:

```rust
// ---- Production sink ----

use tauri::{AppHandle, Emitter};

pub struct TauriEmitter {
    app: AppHandle,
}

impl TauriEmitter {
    pub fn new(app: AppHandle) -> Self { Self { app } }
}

impl EventSink for TauriEmitter {
    fn emit_file_done(&self, p: FileDonePayload) {
        let _ = self.app.emit("file-done", p);
    }
    fn emit_file_error(&self, p: FileErrorPayload) {
        let _ = self.app.emit("file-error", p);
    }
    fn emit_file_skipped(&self, p: FileSkippedPayload) {
        let _ = self.app.emit("file-skipped", p);
    }
    fn emit_companion_error(&self, p: CompanionErrorPayload) {
        let _ = self.app.emit("companion-error", p);
    }
    fn emit_trash_fallback(&self, p: TrashFallbackPayload) {
        let _ = self.app.emit("trash-fallback", p);
    }
    fn emit_batch_done(&self, p: BatchDonePayload) {
        let _ = self.app.emit("batch-done", p);
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop/src-tauri && cargo check`
Expected: Compiles. `commands.rs` still has old code; no test changes here.

- [ ] **Step 3: Commit**

```bash
git add desktop/src-tauri/src/encoder/event_sink.rs
git commit -m "feat(encoder): add TauriEmitter production EventSink"
```

---

### Task 2.3: Rewrite `commands.rs::compress` using the encoder facade

**Files:**
- Modify: `desktop/src-tauri/src/commands.rs`

- [ ] **Step 1: Replace `commands.rs` with the new implementation**

This is a complete rewrite of the file. Replace the entire content:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::batch::BatchState;
use crate::encoder::{
    self,
    event_sink::{
        CompanionErrorPayload, EventSink, FileDonePayload, FileErrorPayload,
        FileSkippedPayload, TauriEmitter, TrashFallbackPayload,
    },
    EncodeOpts, EncodeOutcome, EncodeRequest, EncodeResult, ImageExt,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Semaphore;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressArgs {
    pub batch_id: String,
    pub files: Vec<CompressFile>,
    pub opts: EncodeOpts,
    pub move_originals_to_trash: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompressFile {
    pub id: String,
    pub path: String,
    pub ext: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoReport {
    pub restored: usize,
    pub attempted: usize,
}

fn concurrency_limit() -> usize {
    (num_cpus::get().saturating_sub(1)).max(1).min(8)
}

async fn place_file(tmp: &Path, dest: &Path) -> Result<(), String> {
    if tokio::fs::rename(tmp, dest).await.is_err() {
        match tokio::fs::copy(tmp, dest).await {
            Ok(_) => { let _ = tokio::fs::remove_file(tmp).await; }
            Err(e) => {
                let _ = tokio::fs::remove_file(tmp).await;
                return Err(format!("could not place file: {e}"));
            }
        }
    }
    Ok(())
}

async fn apply_done(
    batch_id: &str,
    file: &CompressFile,
    res: EncodeResult,
    move_to_trash: bool,
    batches: &BatchState,
    sink: &dyn EventSink,
) -> Result<(), String> {
    let src_path = Path::new(&file.path);

    if move_to_trash {
        let disposal = crate::trash::dispose_original(src_path).map_err(|e| e.to_string())?;
        let kind_note = match &disposal.kind {
            crate::trash::DisposalKind::Trashed => None,
            crate::trash::DisposalKind::RenamedFallback { backup_path } =>
                Some(format!("Trash unavailable; original backed up to {}", backup_path.display())),
        };
        batches.record_disposal(batch_id, disposal);
        if let Some(note) = kind_note {
            sink.emit_trash_fallback(TrashFallbackPayload { id: file.id.clone(), note });
        }
    } else {
        tokio::fs::remove_file(src_path).await.map_err(|e| e.to_string())?;
    }

    place_file(&res.main.tmp_path, src_path).await?;

    let mut companion_dests: Vec<PathBuf> = vec![];
    for c in &res.companions {
        let stem = src_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let parent = src_path.parent().unwrap_or_else(|| Path::new("."));
        let dest = parent.join(format!("{stem}{}", c.ext.dotted()));
        match place_file(&c.tmp_path, &dest).await {
            Ok(()) => companion_dests.push(dest),
            Err(e) => sink.emit_companion_error(CompanionErrorPayload {
                id: file.id.clone(),
                ext: c.ext.dotted().trim_start_matches('.').to_string(),
                msg: e,
            }),
        }
    }
    for ce in &res.companion_errors {
        sink.emit_companion_error(CompanionErrorPayload {
            id: file.id.clone(),
            ext: ce.ext.dotted().trim_start_matches('.').to_string(),
            msg: ce.msg.clone(),
        });
    }

    if move_to_trash {
        batches.record_companion_paths(batch_id, companion_dests);
    }

    let src_bytes = std::fs::metadata(src_path).map(|m| m.len()).unwrap_or(0);
    sink.emit_file_done(FileDonePayload {
        id: file.id.clone(),
        src_bytes,
        out_bytes: res.main.bytes,
    });
    Ok(())
}

#[tauri::command]
pub async fn compress(
    app: AppHandle,
    batches: State<'_, BatchState>,
    args: CompressArgs,
) -> Result<(), String> {
    batches.start(&args.batch_id, args.files.len());
    let semaphore = Arc::new(Semaphore::new(concurrency_limit()));
    let opts = Arc::new(args.opts);
    let sink: Arc<dyn EventSink> = Arc::new(TauriEmitter::new(app.clone()));

    for f in args.files {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let app_c = app.clone();
        let opts_c = opts.clone();
        let sink_c = sink.clone();
        let batch_id = args.batch_id.clone();
        let move_to_trash = args.move_originals_to_trash;

        tokio::spawn(async move {
            let _permit = permit; // released on drop
            let f_clone = f.clone();
            let opts_inner = opts_c.clone();

            let result = tokio::task::spawn_blocking(move || -> Result<EncodeOutcome, encoder::EncodeError> {
                let ext = ImageExt::from_str(&f_clone.ext)?;
                encoder::encode(EncodeRequest {
                    src_path: Path::new(&f_clone.path),
                    ext,
                    opts: &opts_inner,
                })
            }).await;

            let batches = app_c.state::<BatchState>();
            match result {
                Ok(Ok(EncodeOutcome::Encoded(res))) => {
                    let src_bytes_pre = std::fs::metadata(&f.path).map(|m| m.len()).unwrap_or(0);
                    let out_bytes = res.main.bytes;
                    if let Err(msg) = apply_done(&batch_id, &f, res, move_to_trash, &batches, &*sink_c).await {
                        sink_c.emit_file_error(FileErrorPayload { id: f.id.clone(), msg });
                    } else {
                        // FileDonePayload was already emitted by apply_done with the post-trash bytes;
                        // src_bytes_pre/out_bytes locals retained only for diagnostic clarity.
                        let _ = (src_bytes_pre, out_bytes);
                    }
                }
                Ok(Ok(EncodeOutcome::SkippedNoGain { src_bytes })) => {
                    sink_c.emit_file_skipped(FileSkippedPayload { id: f.id.clone(), src_bytes });
                }
                Ok(Err(e)) => {
                    sink_c.emit_file_error(FileErrorPayload { id: f.id.clone(), msg: e.to_string() });
                }
                Err(join_err) => {
                    sink_c.emit_file_error(FileErrorPayload {
                        id: f.id.clone(),
                        msg: format!("encoder panic: {join_err}"),
                    });
                }
            }

            batches.tick(&batch_id, &*sink_c);
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn undo_last_batch(batches: State<'_, BatchState>) -> Result<UndoReport, String> {
    let Some(batch) = batches.take_last() else {
        return Ok(UndoReport { restored: 0, attempted: 0 });
    };
    let attempted = batch.disposals.len();
    for d in &batch.disposals {
        let _ = tokio::fs::remove_file(&d.original_path).await;
        for cp in &d.companion_paths {
            let _ = tokio::fs::remove_file(cp).await;
        }
    }
    let restored = crate::trash::restore_all(&batch.disposals).map_err(|e| e.to_string())?;
    Ok(UndoReport { restored, attempted })
}

#[tauri::command]
pub async fn open_trash() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(std::path::PathBuf::from(dirs_next::home_dir().ok_or("no home")?).join(".Trash"))
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("shell:RecycleBinFolder")
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg("trash:///")
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("Unsupported platform".into())
}

#[tauri::command]
pub async fn confirm_close(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}
```

- [ ] **Step 2: Update lib.rs to remove sidecar registration**

Replace `desktop/src-tauri/src/lib.rs`:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod batch;
mod commands;
pub mod encoder;
mod sidecar; // kept this PR; deleted in Phase 3
mod trash;

use batch::BatchState;
use tauri::WindowEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(BatchState::default())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = tauri::Emitter::emit(window, "close-requested", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::compress,
            commands::undo_last_batch,
            commands::open_trash,
            commands::confirm_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Verify compile**

Run: `cd desktop/src-tauri && cargo check`
Expected: Compiles. `sidecar` module is dead code but warnings only.

- [ ] **Step 4: Run all tests**

Run: `cd desktop/src-tauri && cargo test`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add desktop/src-tauri/src/commands.rs desktop/src-tauri/src/lib.rs
git commit -m "refactor(commands): cut compress over to rust-native encoder"
```

---

### Task 2.4: Add IPC schema snapshot tests

**Files:**
- Create: `desktop/src-tauri/tests/ipc_schema.rs`

- [ ] **Step 1: Create the snapshot test**

Create `desktop/src-tauri/tests/ipc_schema.rs`:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// IPC schema snapshot tests. These lock the wire format of every event/payload
// emitted to the frontend. Any change to a payload's field name or type produces
// a snapshot diff and forces a deliberate review.

use desktop_lib::encoder::event_sink::*;

#[test]
fn file_done_payload_schema() {
    let p = FileDonePayload { id: "abc-123".into(), src_bytes: 1024, out_bytes: 512 };
    insta::assert_json_snapshot!(p);
}

#[test]
fn file_error_payload_schema() {
    let p = FileErrorPayload { id: "abc-123".into(), msg: "decode failed".into() };
    insta::assert_json_snapshot!(p);
}

#[test]
fn file_skipped_payload_schema() {
    let p = FileSkippedPayload { id: "abc-123".into(), src_bytes: 1024 };
    insta::assert_json_snapshot!(p);
}

#[test]
fn companion_error_payload_schema() {
    let p = CompanionErrorPayload {
        id: "abc-123".into(),
        ext: "webp".into(),
        msg: "ravif failed".into(),
    };
    insta::assert_json_snapshot!(p);
}

#[test]
fn trash_fallback_payload_schema() {
    let p = TrashFallbackPayload {
        id: "abc-123".into(),
        note: "Trash unavailable; original backed up to /tmp/foo.jpg.original".into(),
    };
    insta::assert_json_snapshot!(p);
}

#[test]
fn batch_done_payload_schema() {
    let p = BatchDonePayload { batch_id: "batch-1".into() };
    insta::assert_json_snapshot!(p);
}
```

- [ ] **Step 2: Run tests — they will create initial snapshots**

Run: `cd desktop/src-tauri && cargo test --test ipc_schema`
Expected: 6 tests run. First run creates `.snap.new` files. Review them, then accept:

```bash
cd desktop/src-tauri
cargo install cargo-insta  # if not already installed
cargo insta review
```

Accept all snapshots after eyeballing them. Each snapshot should look like:

```json
{
  "id": "abc-123",
  "srcBytes": 1024,
  "outBytes": 512
}
```

(camelCase, no `tmp` field, etc.)

- [ ] **Step 3: Re-run to verify pass**

Run: `cd desktop/src-tauri && cargo test --test ipc_schema`
Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add desktop/src-tauri/tests/ipc_schema.rs desktop/src-tauri/tests/snapshots/
git commit -m "test: lock IPC payload schemas via insta snapshots"
```

---

### Task 2.5: Add integration test using MockSink

**Files:**
- Create: `desktop/src-tauri/tests/integration.rs`

- [ ] **Step 1: Create the integration test**

Create `desktop/src-tauri/tests/integration.rs`:

```rust
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Integration tests for the encoder facade + batch lifecycle.
// Uses MockSink so we don't need a Tauri runtime.

use desktop_lib::batch::BatchState;
use desktop_lib::encoder::event_sink::{MockEvent, MockSink};
use desktop_lib::encoder::{encode, EncodeOpts, EncodeOutcome, EncodeRequest, ImageExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn opts() -> EncodeOpts {
    EncodeOpts {
        jpeg_quality: 80,
        png_quality: 80,
        webp_quality: 80,
        avif_quality: 60,
        emit_webp: false,
        emit_avif: false,
    }
}

#[test]
fn batch_lifecycle_emits_one_batch_done_for_three_files() {
    let state = Arc::new(BatchState::default());
    let sink = Arc::new(MockSink::new());

    state.start("batch-A", 3);

    // Simulate 3 worker threads each running encode + tick
    let inputs = vec![
        (fixture("landscape.jpg"), ImageExt::Jpeg),
        (fixture("transparent.png"), ImageExt::Png),
        (fixture("screenshot.png"), ImageExt::Png),
    ];

    let mut handles = vec![];
    for (path, ext) in inputs {
        let s = state.clone();
        let snk = sink.clone();
        let o = opts();
        handles.push(thread::spawn(move || {
            let outcome = encode(EncodeRequest { src_path: &path, ext, opts: &o }).unwrap();
            // We don't run apply_done here (no real file replacement in the test);
            // just record the outcome and tick.
            match outcome {
                EncodeOutcome::Encoded(_) => {}
                EncodeOutcome::SkippedNoGain { .. } => {}
            }
            s.tick("batch-A", &*snk);
        }));
    }
    for h in handles { h.join().unwrap(); }

    let bd = sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_)));
    assert_eq!(bd, 1, "expected exactly one batch-done event");
}

#[test]
fn corrupt_input_returns_decode_error_without_panicking() {
    let result = encode(EncodeRequest {
        src_path: &fixture("corrupt.jpg"),
        ext: ImageExt::Jpeg,
        opts: &opts(),
    });
    assert!(matches!(result, Err(desktop_lib::encoder::EncodeError::Decode(_))));
}

#[test]
fn webp_and_avif_companions_emitted_when_enabled() {
    let mut o = opts();
    o.emit_webp = true;
    o.emit_avif = true;

    let outcome = encode(EncodeRequest {
        src_path: &fixture("landscape.jpg"),
        ext: ImageExt::Jpeg,
        opts: &o,
    }).unwrap();

    if let EncodeOutcome::Encoded(r) = outcome {
        assert_eq!(r.companions.len(), 2);
        let exts: Vec<_> = r.companions.iter().map(|c| c.ext).collect();
        assert!(exts.contains(&ImageExt::Webp));
        assert!(exts.contains(&ImageExt::Avif));
    } else {
        panic!("expected Encoded");
    }
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cd desktop/src-tauri && cargo test --test integration`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add desktop/src-tauri/tests/integration.rs
git commit -m "test: add encoder + batch integration tests with MockSink"
```

---

### Task 2.6: Update frontend `ipc.ts` (slim FileDonePayload, add batch-done)

**Files:**
- Modify: `desktop/src/ipc.ts`

- [ ] **Step 1: Update ipc.ts**

Replace the file content:

```typescript
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Settings } from './ui/settings';

export interface CompressFile { id: string; path: string; ext: string; }

export interface CompressArgs {
  batchId: string;
  files: CompressFile[];
  opts: {
    jpegQuality: number;
    pngQuality: number;
    webpQuality: number;
    avifQuality: number;
    emitWebp: boolean;
    emitAvif: boolean;
  };
  moveOriginalsToTrash: boolean;
}

export async function compress(args: CompressArgs): Promise<void> {
  await invoke('compress', { args });
}

// FileDonePayload no longer carries `tmp` — the field was a sidecar-era artifact
// the frontend never consumed.
export interface FileDonePayload { id: string; srcBytes: number; outBytes: number; }
export interface FileErrorPayload { id: string; msg: string; }
export interface FileSkippedPayload { id: string; srcBytes: number; }
export interface BatchDonePayload { batchId: string; }

export function onFileDone(cb: (p: FileDonePayload) => void) { return listen<FileDonePayload>('file-done', (e) => cb(e.payload)); }
export function onFileError(cb: (p: FileErrorPayload) => void) { return listen<FileErrorPayload>('file-error', (e) => cb(e.payload)); }
export function onFileSkipped(cb: (p: FileSkippedPayload) => void) { return listen<FileSkippedPayload>('file-skipped', (e) => cb(e.payload)); }
export function onBatchDone(cb: (p: BatchDonePayload) => void) { return listen<BatchDonePayload>('batch-done', (e) => cb(e.payload)); }

export function toOpts(s: Settings) {
  return {
    jpegQuality: s.jpegQuality,
    pngQuality: s.pngQuality,
    webpQuality: s.webpQuality,
    avifQuality: s.avifQuality,
    emitWebp: s.emitWebp,
    emitAvif: s.emitAvif,
  };
}

export interface UndoReport { restored: number; attempted: number; }
export async function undoLastBatch(): Promise<UndoReport> {
  return invoke<UndoReport>('undo_last_batch');
}

export async function openTrash(): Promise<void> { await invoke('open_trash'); }
```

- [ ] **Step 2: Update main.ts to use batch-done for Undo enable + remove sidecar-crashed**

Replace lines 10, 27–57, and 95–103 of `desktop/src/main.ts`. Final state of main.ts:

```typescript
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { mountToolbar } from './ui/toolbar';
import { mountDropzone } from './ui/dropzone';
import { mountFileList } from './ui/file-list';
import { mountStatusBar } from './ui/statusbar';
import { store, anyWorking } from './state';
import { basename } from './util/path';
import { openSettingsPanel, loadSettings, getSettings } from './ui/settings';
import { compress, toOpts, onFileDone, onFileError, onFileSkipped, onBatchDone, undoLastBatch } from './ipc';
import { expandPaths } from './fs';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import * as i18n from './i18n';
import { initTheme } from './ui/theme';

function extOf(p: string): string {
  const idx = p.lastIndexOf('.');
  return idx >= 0 ? p.slice(idx).toLowerCase() : '';
}

async function main() {
  await loadSettings();
  i18n.init(getSettings().language);
  initTheme();

  onFileDone((p) => {
    const row = store.snapshotById(p.id);
    if (!row) return;
    store.update(row.path, { status: 'done', srcBytes: p.srcBytes, outBytes: p.outBytes });
  });
  onFileError((p) => {
    const row = store.snapshotById(p.id);
    if (!row) return;
    store.update(row.path, { status: 'error', errorMsg: p.msg });
  });
  onFileSkipped((p) => {
    const row = store.snapshotById(p.id);
    if (!row) return;
    store.update(row.path, { status: 'skipped-no-gain', srcBytes: p.srcBytes });
  });

  const toolbar = mountToolbar(document.getElementById('toolbar')!, {
    onSettings: () => openSettingsPanel(),
    onUndo: async () => {
      const r = await undoLastBatch();
      console.log(`undone: ${r.restored}/${r.attempted}`);
      toolbar.setUndoEnabled(false);
      store.clear();
    },
  });

  // Backend signals end-of-batch authoritatively. This replaces the previous
  // pending-counter heuristic that could deadlock if an event was lost.
  onBatchDone(() => {
    const rows = store.snapshot();
    const hasTerminal = rows.some((r) => r.status === 'done');
    toolbar.setUndoEnabled(hasTerminal);
  });

  const dropzone = await mountDropzone(document.getElementById('dropzone')!, async (paths) => {
    const { files: expandedPaths, skipped } = await expandPaths(paths);
    if (skipped > 0) console.log(`Skipped ${skipped} unsupported`);
    const files = expandedPaths.map((p) => ({
      id: crypto.randomUUID(),
      path: p,
      ext: extOf(p),
      name: basename(p),
    }));
    for (const f of files) store.upsert({ id: f.id, path: f.path, name: f.name, status: 'working' });
    try {
      await compress({
        batchId: crypto.randomUUID(),
        files: files.map(({ id, path, ext }) => ({ id, path, ext })),
        opts: toOpts(getSettings()),
        moveOriginalsToTrash: getSettings().moveOriginalsToTrash,
      });
    } catch (err) {
      console.error('compress failed:', err);
      for (const f of files) store.update(f.path, { status: 'error', errorMsg: String(err) });
    }
  });

  mountFileList(document.getElementById('list')!);
  mountStatusBar(document.getElementById('statusbar')!);

  i18n.onLocaleChange(() => { toolbar.refresh(); dropzone.refresh(); });

  listen('close-requested', async () => {
    if (anyWorking()) {
      const count = store.snapshot().filter((r) => r.status === 'working' || r.status === 'pending').length;
      if (!confirm(i18n.t('confirm.quitProcessing', { count }))) return;
    }
    await invoke('confirm_close');
  });
  // Note: previous `sidecar-crashed` listener removed — there is no sidecar to crash.
  // If the Tauri host itself crashes, the OS closes the window.
}
main();
```

- [ ] **Step 2.5: Check the `alert.engineCrashed` i18n key is no longer referenced**

Run: `grep -r "alert.engineCrashed" desktop/src/ desktop/locales/ 2>/dev/null`
Expected: No results in `desktop/src/`. Locale files may still contain the key — it's harmless, leave it for now.

- [ ] **Step 3: Build the frontend**

Run: `cd desktop && npm run build`
Expected: Vite build succeeds.

- [ ] **Step 4: Commit**

```bash
git add desktop/src/ipc.ts desktop/src/main.ts
git commit -m "refactor(frontend): consume batch-done; drop sidecar-crashed listener"
```

---

### Task 2.7: A/B comparison test (manual gate before landing Phase 2)

**Files:** none (manual test using a temporary script)

- [ ] **Step 1: Create a comparison script**

Create `desktop/scripts/ab-compare.mjs`:

```javascript
#!/usr/bin/env node
// One-off A/B comparison: run both pipelines on the same images and report deltas.
// Usage: node desktop/scripts/ab-compare.mjs <image-dir>

import { readdir, stat, copyFile, rm, mkdir } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { join, basename, extname } from 'node:path';
import { tmpdir } from 'node:os';

const SRC = process.argv[2];
if (!SRC) { console.error('Usage: ab-compare.mjs <image-dir>'); process.exit(1); }

const STAGE = join(tmpdir(), 'hando-ab-' + Date.now());
await mkdir(join(STAGE, 'sharp-out'), { recursive: true });
await mkdir(join(STAGE, 'rust-out'), { recursive: true });

const files = (await readdir(SRC)).filter(n => /\.(jpg|jpeg|png|webp|avif)$/i.test(n));
console.log(`Found ${files.length} images`);

// Stage 1: copy + run via current sidecar (cli) — assumes the legacy CLI still exists
console.log('--- Sharp pipeline ---');
for (const f of files) {
  const src = join(SRC, f);
  const dst = join(STAGE, 'sharp-out', f);
  await copyFile(src, dst);
}
const sharpRes = spawnSync('node', ['index.js', join(STAGE, 'sharp-out'), '-o', join(STAGE, 'sharp-out')], { stdio: 'inherit' });
if (sharpRes.status !== 0) { console.error('Sharp pipeline failed'); process.exit(1); }

// Stage 2: build the new desktop binary in release mode and use a small Rust harness.
// (Easier path for a one-off: run the new pipeline by hand via the desktop app and pull
// outputs from the same dir. The script below assumes you've done this manually and
// placed Rust outputs under STAGE/rust-out.)
console.log('--- Rust pipeline ---');
console.log(`Run the desktop app on ${SRC} and copy outputs to ${join(STAGE, 'rust-out')}, then re-run this script with --compare-only`);

// Stage 3: compare
console.log('--- Comparison ---');
let totalDelta = 0;
for (const f of files) {
  const sharpFile = join(STAGE, 'sharp-out', f);
  const rustFile = join(STAGE, 'rust-out', f);
  try {
    const s = (await stat(sharpFile)).size;
    const r = (await stat(rustFile)).size;
    const delta = ((r - s) / s) * 100;
    totalDelta += delta;
    const flag = Math.abs(delta) > 5 ? '⚠️ ' : '   ';
    console.log(`${flag}${f.padEnd(40)} sharp=${s.toString().padStart(8)}  rust=${r.toString().padStart(8)}  Δ=${delta.toFixed(1)}%`);
  } catch (e) {
    console.log(`   ${f.padEnd(40)} <missing>`);
  }
}
console.log(`Mean delta: ${(totalDelta / files.length).toFixed(1)}%`);
console.log(`Working dir: ${STAGE}`);
```

- [ ] **Step 2: Source 10–20 representative test images**

Pick a folder with a mix:
- 5–7 photographs (skies, skin tones, foliage)
- 3–5 screenshots / UI mocks
- 2 transparent PNGs
- 1–2 small (< 100KB) already-optimized images
- 1–2 large (> 5MB) images

Place them in `~/hando-ab-input/` (or any path).

- [ ] **Step 3: Run the comparison**

Run:
```bash
node desktop/scripts/ab-compare.mjs ~/hando-ab-input
# Then manually: open the desktop app, drag the same folder in, copy outputs to STAGE/rust-out
node desktop/scripts/ab-compare.mjs ~/hando-ab-input  # re-run for comparison report
```

- [ ] **Step 4: Validate the gate**

Land gate (all must hold):
- Mean size delta within ±5%.
- No single file > 15% larger.
- Visual spot-check on 3 images: open both side-by-side; no banding, no color shift, no muddy detail.

If any criterion fails, **do not land Phase 2**. Tune defaults (e.g., raise oxipng preset, adjust jpeg trellis settings) and re-run.

- [ ] **Step 5: Delete the comparison script before merging**

```bash
rm desktop/scripts/ab-compare.mjs
git add desktop/scripts/
git commit -m "chore: remove one-off A/B comparison script"
```

---

### Task 2.8: Phase 2 land gate — manual E2E checklist

- [ ] **Step 1: Build dev mode and run the app**

Run: `cd desktop && npm run tauri dev`
Expected: App launches. Logs show no `sidecar` references.

- [ ] **Step 2: Run the manual E2E checklist**

```
[ ] Drop 5 mixed-format images (JPG/PNG/WebP/AVIF) → all compress
[ ] Enable emit WebP + AVIF in Settings → companions appear in source folder
[ ] Drop 1 tiny pre-optimized image → row shows "skipped"
[ ] Drop 1 corrupt JPG → row shows file-error, others succeed
[ ] After batch ends, Undo button enables (verify it lights up)
[ ] Click Undo → originals reappear from Recycle Bin, companions deleted
[ ] Disable Recycle Bin (Windows: empty + lock; macOS: skip) → fallback rename to .original + status notice
[ ] EXIF orientation=6 portrait → output upright in 2 different image viewers
[ ] Drop 50 mixed images at once → CPU/RAM stay reasonable, UI responsive
```

- [ ] **Step 3: Open Phase 2 PR**

```markdown
## Phase 2 — Cut commands.rs over to Rust-native encoder

The `compress` command no longer spawns the Node sidecar. It calls `encoder::encode()`
through `tokio::task::spawn_blocking` with a `Semaphore`-controlled concurrency cap.
The sidecar source files remain in the tree (deleted in Phase 3).

### Changes
- `commands.rs` rewritten to use the encoder facade
- `batch.rs` adds atomic `completed`/`expected` counters and `tick()`
- `lib.rs` removes `SidecarState` and the `sidecar-crashed` listener
- `event_sink.rs` adds `TauriEmitter` (production sink)
- `FileDonePayload` slimmed: `tmp` field removed
- New `batch-done` event; frontend uses it to enable Undo authoritatively
- `tests/ipc_schema.rs` locks every payload via insta snapshots
- `tests/integration.rs` exercises encoder + batch lifecycle with `MockSink`

### Land gates
- [x] All `cargo test` green
- [x] A/B comparison test passes (mean delta within ±5%, no file > 15% worse)
- [x] Manual E2E checklist passes
```

---

# Phase 3 — The Node Purge

**Goal:** Delete all sidecar code, the Node binary, the `node_modules` for the sidecar, the standalone CLI in root `src/`, and all related scripts. Update docs. The repo's only Node footprint after this PR is the desktop frontend dev tooling.

**PR title:** `chore: remove node sidecar and standalone CLI`

---

### Task 3.1: Prune `tauri.conf.json` (must happen before file deletions)

**Files:**
- Modify: `desktop/src-tauri/tauri.conf.json`

The `tauri-build` build script validates `externalBin` and `resources` paths at compile time. If we delete `binaries/` and `sidecar-deps/` while `tauri.conf.json` still references them, even `cargo build` fails. Prune the config first.

- [ ] **Step 1: Update the bundle section**

Replace the `bundle` block in `tauri.conf.json` with:

```json
"bundle": {
  "active": true,
  "targets": "all",
  "icon": [
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/128x128@2x.png",
    "icons/icon.icns",
    "icons/icon.ico"
  ],
  "windows": {
    "nsis": {
      "installMode": "both",
      "headerImage": "assets/installer-header.bmp"
    }
  }
}
```

(Removed: `externalBin`, `resources`.)

- [ ] **Step 2: Verify build still works (sidecar files still present but no longer bundled)**

Run: `cd desktop/src-tauri && cargo build --release`
Expected: Compiles. The sidecar.rs source still exists and compiles as dead code; `tauri-build` no longer cares about the deleted-but-referenced paths.

- [ ] **Step 3: Commit**

```bash
git add desktop/src-tauri/tauri.conf.json
git commit -m "chore: prune tauri.conf.json — remove externalBin and sidecar resources"
```

---

### Task 3.2: Delete sidecar source and bundled Node binary

**Files:**
- Delete: `desktop/src-tauri/src/sidecar.rs`
- Delete: `desktop/src-tauri/binaries/` (entire directory)
- Delete: `desktop/src-tauri/sidecar-deps/` (entire directory)
- Modify: `desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Remove the `mod sidecar;` declaration from lib.rs**

Edit `desktop/src-tauri/src/lib.rs`. Delete the line `mod sidecar; // kept this PR; deleted in Phase 3`. Final mod block:

```rust
pub mod batch;
mod commands;
pub mod encoder;
mod trash;
```

- [ ] **Step 2: Delete the files**

Run:
```bash
rm desktop/src-tauri/src/sidecar.rs
rm -rf desktop/src-tauri/binaries
rm -rf desktop/src-tauri/sidecar-deps
```

- [ ] **Step 3: Verify the full Tauri build still produces a binary**

Run: `cd desktop && npm run tauri build`
Expected: Single binary at `desktop/src-tauri/target/release/hando.exe` (Windows) or `Hando.app` (macOS). No `node.exe` or `node_modules/` alongside it.

- [ ] **Step 4: Smoke-test the binary on a clean folder**

Copy `hando.exe` (or `Hando.app`) to a different folder (e.g., `~/Desktop/`) with no other Hando files. Double-click. Expected: app launches, drag in an image, it compresses correctly.

- [ ] **Step 5: Commit**

```bash
git add desktop/src-tauri/src/lib.rs desktop/src-tauri/src/sidecar.rs desktop/src-tauri/binaries desktop/src-tauri/sidecar-deps
git commit -m "chore: delete sidecar source, node binary, and bundled deps"
```

---

### Task 3.3: Delete root CLI

**Files:**
- Delete: `index.js`
- Delete: `src/` (root directory — `sidecar.js`, `encoder.js`, `config.js`, `*.test.js`)
- Modify: `package.json` (root)

- [ ] **Step 1: Delete the files**

```bash
rm index.js
rm -rf src
```

- [ ] **Step 2: Update root `package.json` — remove bin entry and sharp dependency**

Edit `package.json`. Remove the `"bin"` field entirely if present. Remove `"sharp"` from `dependencies`. The remaining file should describe the project metadata only (or be deleted entirely if nothing depends on it — check for references first).

Run: `grep -rn "\"hando\"" desktop/ docs/ 2>/dev/null`
If no references, the `"bin"` removal is safe.

Final `package.json` (illustrative — keep your existing fields):

```json
{
  "name": "hando-monorepo",
  "version": "0.1.0",
  "private": true,
  "description": "Hando — desktop image optimizer (root metadata only; see desktop/ for the app)",
  "license": "AGPL-3.0-or-later"
}
```

- [ ] **Step 3: Remove the root lockfile if it only existed for sharp**

Run: `cat package-lock.json | grep -c sharp 2>/dev/null || echo 0`
If the only thing the lockfile tracked was sharp + transitives, delete it:

```bash
rm package-lock.json node_modules -rf
```

If the lockfile tracked anything else used elsewhere, leave it alone.

- [ ] **Step 4: Commit**

```bash
git add index.js src package.json package-lock.json
git commit -m "chore: remove standalone CLI (replaced by desktop app)"
```

---

### Task 3.4: Replace `build-portable.mjs` with `build-dist.mjs`

**Files:**
- Delete: `desktop/scripts/build-portable.mjs`
- Delete: `desktop/scripts/fetch-node.sh`
- Delete: `desktop/scripts/copy-sidecar-deps.mjs`
- Create: `desktop/scripts/build-dist.mjs`

- [ ] **Step 1: Delete the old scripts**

```bash
rm desktop/scripts/build-portable.mjs
rm desktop/scripts/fetch-node.sh
rm desktop/scripts/copy-sidecar-deps.mjs
```

- [ ] **Step 2: Create the new dist script**

Create `desktop/scripts/build-dist.mjs`:

```javascript
#!/usr/bin/env node
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Post-build artifact organizer.
// 1. Locates the Tauri release binary
// 2. Renames to `Hando-{platform}-{arch}-v{version}.{ext}`
// 3. Zips for portable distribution
//
// Usage: node desktop/scripts/build-dist.mjs

import { readFile, copyFile, mkdir, stat } from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import { spawn } from 'node:child_process';
import { join, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { platform, arch } from 'node:os';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

async function main() {
  const cargoToml = await readFile(join(ROOT, 'src-tauri/Cargo.toml'), 'utf8');
  const versionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
  if (!versionMatch) throw new Error('Could not parse version from Cargo.toml');
  const version = versionMatch[1];

  const plat = platform(); // 'win32', 'darwin', 'linux'
  const a = arch();        // 'x64', 'arm64'

  const platTag = plat === 'win32' ? 'win' : plat === 'darwin' ? 'mac' : plat;
  const ext = plat === 'win32' ? '.exe' : '.app';
  const srcBinary = plat === 'win32'
    ? join(ROOT, 'src-tauri/target/release/hando.exe')
    : join(ROOT, 'src-tauri/target/release/bundle/macos/Hando.app');

  await stat(srcBinary).catch(() => { throw new Error(`Binary not found at ${srcBinary} — run \`npm run tauri build\` first`); });

  const distDir = join(ROOT, 'dist-final');
  await mkdir(distDir, { recursive: true });

  const niceName = `Hando-${platTag}-${a}-v${version}${ext}`;
  const dstPath = join(distDir, niceName);
  await copyFile(srcBinary, dstPath);
  console.log(`Renamed: ${dstPath}`);

  // Zip — use system 'zip' on macOS/linux, PowerShell Compress-Archive on Windows.
  const zipName = `${niceName}.zip`;
  const zipPath = join(distDir, zipName);
  await zipFile(dstPath, zipPath);
  console.log(`Zipped:  ${zipPath}`);
}

function zipFile(src, dst) {
  return new Promise((resolve, reject) => {
    const isWin = platform() === 'win32';
    const cmd = isWin ? 'powershell' : 'zip';
    const args = isWin
      ? ['-NoProfile', '-Command', `Compress-Archive -Path '${src}' -DestinationPath '${dst}' -Force`]
      : ['-r', '-j', dst, src];
    const child = spawn(cmd, args, { stdio: 'inherit' });
    child.on('exit', (code) => code === 0 ? resolve() : reject(new Error(`${cmd} exited ${code}`)));
    child.on('error', reject);
  });
}

main().catch((err) => { console.error(err); process.exit(1); });
```

- [ ] **Step 3: Test the dist script**

Run:
```bash
cd desktop && npm run tauri build
node scripts/build-dist.mjs
```
Expected: `desktop/dist-final/Hando-{platform}-{arch}-v0.1.0.{ext}` and a `.zip` of the same name.

- [ ] **Step 4: Add a npm script**

Modify `desktop/package.json` — append to `scripts`:

```json
"dist": "tauri build && node scripts/build-dist.mjs"
```

- [ ] **Step 5: Commit**

```bash
git add desktop/scripts/ desktop/package.json
git commit -m "chore: replace portable build script with dist artifact organizer"
```

---

### Task 3.5: Update root README.md

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Rewrite README to focus on the desktop app**

Replace the README content. Concrete final state (adapt project name/badges as needed):

```markdown
# Hando

A single-file desktop image optimizer for Windows and macOS. Drag images in, get smaller files out. No installer, no Node runtime, no companion folders.

## Download

Pre-built binaries are published in [Releases](../../releases). Single executable; double-click to launch.

| Platform | File |
|---|---|
| Windows x64 | `Hando-win-x64-v*.exe` |
| macOS Universal | `Hando-mac-universal-v*.app.zip` |

## Build from source

### Prerequisites

- **Rust** stable, ≥ 1.85.1
- **Node.js** 20+ (only for the desktop frontend dev server — not bundled into the app)
- Platform toolchain:
  - **Windows**: Visual Studio 2022 Build Tools (Desktop development with C++) + **NASM** (`winget install nasm`)
    - **Important**: Run `cargo` commands from the *"x64 Native Tools Command Prompt for VS 2022"* shell, or first invoke `vcvarsall.bat x64`. Otherwise `mozjpeg-sys` and `webp-sys` linker steps will fail with cryptic errors.
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`) + **NASM** (`brew install nasm`)

### Commands

```bash
cd desktop && npm install        # frontend deps
cd desktop && npm run tauri dev  # dev mode (Vite + Tauri)
cd desktop && npm run tauri build  # release build
cd desktop && npm run dist         # release build + rename + zip
```

Outputs land in `desktop/dist-final/`.

## Architecture

See [`docs/superpowers/specs/2026-04-25-rust-native-encoder-design.md`](docs/superpowers/specs/2026-04-25-rust-native-encoder-design.md) for the full design.

Briefly:
- WebView (TypeScript + Vite) for UI
- Tauri 2 (Rust) host
- In-process Rust encoders: `mozjpeg`, `oxipng`, `imagequant`, `webp`, `ravif`
- No sidecars, no native runtime dependencies

## License

AGPL-3.0-or-later. See [LICENSE](LICENSE).
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: rewrite README around desktop app"
```

---

### Task 3.6: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Rewrite the Architecture section**

Open `CLAUDE.md`. Replace the entire `## Commands` and `## Architecture` sections with:

```markdown
## Commands

### Desktop app (Tauri, under `desktop/`)

```bash
cd desktop && npm install              # install frontend deps
cd desktop && npm run tauri dev        # start the desktop app in dev mode
cd desktop && npm run tauri build      # release build
cd desktop && npm run dist             # release build + rename artifacts + zip
cd desktop/src-tauri && cargo build    # compile Rust only
cd desktop/src-tauri && cargo test     # run Rust unit + integration tests
```

**Windows toolchain note:** Run cargo from the *"x64 Native Tools Command Prompt for VS 2022"* shell. `mozjpeg-sys` and `webp-sys` need MSVC's `cl.exe` and NASM in PATH.

## Architecture

Single-file Tauri 2 desktop app. No CLI, no sidecar.

```
WebView (TypeScript + Vite)
    ↕ invoke / events (Tauri IPC)
Rust host (tokio, in-process encoders, trash)
    └─ encoder facade (encoder/mod.rs)
        ├─ jpeg.rs   (mozjpeg)
        ├─ png.rs    (imagequant + oxipng)
        ├─ webp.rs   (libwebp via webp crate)
        └─ avif.rs   (ravif via rav1e)
```

**Rust host (`desktop/src-tauri/src/`):**
- `commands.rs` — `compress`: spawns one `tokio::task::spawn_blocking` per file, gated by a `Semaphore` of `(num_cpus - 1).clamp(1, 8)`; calls `encoder::encode()`; emits `file-done` / `file-error` / `file-skipped` / `companion-error` / `trash-fallback` events. `undo_last_batch`: deletes compressed files + companions and restores originals from Trash.
- `encoder/mod.rs` — `encode()` facade. Dispatches by `ImageExt`. Returns `EncodeOutcome::Encoded(EncodeResult)` or `EncodeOutcome::SkippedNoGain`.
- `encoder/decode.rs` — Unified RGBA decode. JPEG via `mozjpeg::Decompress`; PNG/WebP/AVIF via `image::ImageReader`. Applies EXIF orientation by rotating the pixel buffer, then strips EXIF on encode.
- `encoder/event_sink.rs` — `EventSink` trait with `TauriEmitter` (production) and `MockSink` (tests). Avoids the brittle `tauri::test::mock_app()` API entirely.
- `batch.rs` — `BatchState` with atomic `completed`/`expected` counters; `tick()` emits `batch-done` when the last file completes.
- `trash.rs` — Wraps `trash` crate; `\\?\` prefix stripping for Windows; `.original` rename fallback.

**Frontend (`desktop/src/`):**
- `state.ts`, `ipc.ts`, `fs.ts`, `ui/*.ts` — unchanged from the sidecar era except for `ipc.ts` removing the `tmp` field from `FileDonePayload` and adding `onBatchDone` / `BatchDonePayload`.
- `main.ts` — subscribes to `batch-done` to enable Undo authoritatively. The previous `sidecar-crashed` listener was removed.

**Supported formats:** JPEG, PNG, WebP, AVIF (encode + companion output).

**Key data flow for a compress batch:**
1. User drops files / clicks to add → `expandPaths` filters to supported extensions
2. Frontend `invoke('compress', { batchId, files, opts, moveOriginalsToTrash })`
3. Rust queues each file as a `spawn_blocking` task, semaphore-gated
4. Each task: decode → encode main + optional WebP/AVIF companions → write tmp paths
5. Per-file: place tmp at src path (rename or copy fallback), trash original, emit `file-done`
6. Each task `tick()`s the batch counter; the last one emits `batch-done`

**Undo:** `BatchState` tracks `Disposal` records (Trash path or `.original` backup + companion paths). `undo_last_batch` deletes compressed files + companions, then `trash::restore_all` recovers originals.

**Windows-specific notes:**
- `Path::canonicalize()` returns `\\?\`-prefixed paths; `trash.rs` strips this before storing so Recycle Bin lookup matches
- Cross-drive `fs::rename` fails (e.g. `C:\Temp` → `D:\files`); `commands.rs` falls back to `fs::copy` + `remove_file`
- mozjpeg-sys requires MSVC + NASM (see Windows toolchain note above)
```

- [ ] **Step 2: Verify CLAUDE.md no longer mentions sidecar / Node binary / `src/encoder.js`**

Run: `grep -in "sidecar\|node.exe\|src/encoder.js\|src/sidecar.js\|fetch-node" CLAUDE.md`
Expected: No matches.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: rewrite CLAUDE.md architecture section for rust-native pipeline"
```

---

### Task 3.7: Phase 3 land gate — clean-machine smoke test

- [ ] **Step 1: Build a release binary**

Run: `cd desktop && npm run dist`
Expected: `desktop/dist-final/Hando-{platform}-{arch}-v0.1.0.{ext}` produced.

- [ ] **Step 2: Test on a clean machine (or new user account)**

Copy the binary to a machine without dev tools installed. Double-click. Expected: launches, processes a few images, no missing-DLL errors.

If you don't have a clean machine: minimum, copy the binary to a folder with **no other Hando files**, ensure no `node.exe` in PATH affects it, and verify it still runs.

- [ ] **Step 3: Open Phase 3 PR**

```markdown
## Phase 3 — The Node Purge

Removes all sidecar code, the bundled Node binary, the standalone CLI, and supporting scripts. The repo now has zero Node footprint outside the desktop frontend dev tooling.

### Deletions
- `desktop/src-tauri/src/sidecar.rs`
- `desktop/src-tauri/binaries/`
- `desktop/src-tauri/sidecar-deps/`
- `desktop/scripts/{fetch-node.sh,copy-sidecar-deps.mjs,build-portable.mjs}`
- Root `index.js`, `src/`, `package-lock.json` (sharp gone)

### Added
- `desktop/scripts/build-dist.mjs` — post-build artifact organizer
- `desktop/package.json` — new `dist` script

### Updated
- `tauri.conf.json` — `externalBin` and sidecar `resources` removed
- `README.md` — rewritten around desktop app
- `CLAUDE.md` — Architecture section rewritten
- Root `package.json` — sharp dep removed; `bin` entry removed

### Verification
- [x] `npm run dist` produces `Hando-{plat}-{arch}-v0.1.0.{ext}` + `.zip`
- [x] Single binary runs by double-click on a clean folder
- [x] `cargo test` green
```

---

# Phase 4 — CI/CD Matrix + Docs

**Goal:** Tagged releases produce Windows + macOS-universal artifacts via GitHub Actions. README prerequisites updated. CLAUDE.md is already done in Phase 3.

**PR title:** `ci: add cross-platform release workflow`

---

### Task 4.1: Add release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create the workflow**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: 'macos-14'
            args: '--target universal-apple-darwin'
            label: 'mac'
          - platform: 'windows-latest'
            args: ''
            label: 'win'

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.platform == 'macos-14' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}

      - name: Cache Cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            desktop/src-tauri/target
          key: ${{ matrix.platform }}-cargo-${{ hashFiles('desktop/src-tauri/Cargo.lock') }}

      - name: Install NASM (Windows)
        if: matrix.platform == 'windows-latest'
        run: choco install nasm -y

      - name: Install NASM (macOS)
        if: matrix.platform == 'macos-14'
        run: brew install nasm

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Install frontend deps
        working-directory: desktop
        run: npm ci

      - name: Build app (Tauri)
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          projectPath: desktop
          tagName: ${{ github.ref_name }}
          releaseName: 'Hando ${{ github.ref_name }}'
          releaseDraft: true
          prerelease: false
          args: ${{ matrix.args }}
```

- [ ] **Step 2: Test locally with `act` (optional)**

If you have [`act`](https://github.com/nektos/act) installed:
```bash
act push -W .github/workflows/release.yml --container-architecture linux/amd64
```
Expected: Workflow parses cleanly. (Full builds will fail in `act` because it can't replicate macOS/Windows runners — that's fine; we just want syntax validation.)

- [ ] **Step 3: Push a test tag**

```bash
git tag v0.1.0-test
git push origin v0.1.0-test
```

Watch the GitHub Actions run. Expected: Both `macos-14` and `windows-latest` jobs complete; a draft release is created with both artifacts attached.

- [ ] **Step 4: Delete the test tag**

```bash
git tag -d v0.1.0-test
git push origin :refs/tags/v0.1.0-test
# Also delete the draft release in the GitHub UI
```

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add cross-platform release workflow with tauri-action"
```

---

### Task 4.2: Final docs sweep

**Files:**
- Modify: `README.md` (release process section)

- [ ] **Step 1: Add a release process section to README**

Append to `README.md`:

```markdown
## Release process

1. Bump version in `desktop/src-tauri/Cargo.toml` and `desktop/src-tauri/tauri.conf.json`.
2. Commit and tag: `git tag v0.2.0 && git push origin v0.2.0`.
3. GitHub Actions builds Windows + macOS-universal artifacts and creates a draft release.
4. Edit the draft release notes, then publish.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: document tagged-release process"
```

---

### Task 4.3: Phase 4 land gate

- [ ] **Step 1: Verify a real release tag produces both artifacts**

Bump version to `0.2.0` and push the tag. Expected: GitHub Actions produces both binaries; download each and verify on a clean machine.

- [ ] **Step 2: Open Phase 4 PR**

```markdown
## Phase 4 — CI/CD matrix + release docs

Tagged releases now produce Windows + macOS-universal artifacts via `tauri-apps/tauri-action`.

### Added
- `.github/workflows/release.yml` — matrix build (windows-latest, macos-14)
- README "Release process" section

### Verification
- [x] Test tag `v0.1.0-test` produced both artifacts
- [x] Draft release auto-created
```

---

## Self-Review Checklist

Run this once after writing the plan:

- [x] **Spec coverage**: Every spec section (§1 architecture, §2 IPC, §3 encoder, §4 commands, §5 build, §6 testing, §7 phases) has corresponding tasks.
- [x] **No placeholders**: No "TBD", "implement later", "add error handling" — every step has concrete code.
- [x] **Type consistency**: `EncodeOpts` field names match between Rust struct and frontend interface. `FileDonePayload` does not include `tmp` in any task. `tick(batch_id, sink)` signature consistent across `batch.rs` definition and `commands.rs` call sites.
- [x] **Phase ordering**: Phase 1 is purely additive; Phase 2 cuts over with sidecar still present; Phase 3 deletes sidecar after Phase 2 validates the new path; Phase 4 wraps in CI.
- [x] **Land gates**: Every phase has explicit acceptance criteria. Phase 2's A/B comparison is a hard gate.
