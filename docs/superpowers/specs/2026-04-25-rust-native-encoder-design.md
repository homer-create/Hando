# Rust-Native Encoder Refactor — Design Spec

**Date:** 2026-04-25
**Status:** Approved (pending implementation plan)
**Owner:** homershie

## Problem

The current Hando desktop app's "portable" build is not portable in the spirit users expect. To run, it requires a folder containing:

```
hando.exe          ← Tauri binary (~7 MB)
node.exe           ← ~30 MB
node_modules/      ← sharp + libvips platform binary + deps (~10 MB+)
src/               ← sidecar.js, encoder.js, config.js
```

The goal is an ImageOptim-style **single executable** that users double-click and run — no folder, no installer, no external dependencies. Sharp's reliance on libvips (a native Node.js addon with platform-specific dynamic libraries) makes that goal architecturally impossible without a fundamental change.

This spec replaces the Node sidecar with native Rust image encoders, retiring the CLI in the process.

## Goals

- **G1.** Single-file desktop deliverable on Windows (`Hando.exe`) and macOS (`Hando.app` with universal binary).
- **G2.** Compression quality on par with the current Sharp pipeline (≤ 5% file-size delta vs. current at the same quality settings, no perceptible visual regression).
- **G3.** Drop the Node sidecar and the standalone CLI entirely; the desktop app becomes the sole product.
- **G4.** Preserve the current frontend IPC contract so the WebView code requires only a small additive change (a `batch-done` listener).
- **G5.** Build complexity that one developer can run on Windows + macOS without exotic tooling beyond standard C toolchains and NASM.

## Non-Goals

- Linux desktop support (out of scope for this refactor; can be added later by extending the CI matrix).
- A "Cancel" button for in-progress batches (YAGNI; current UI has none).
- Exposing AVIF speed/quality tradeoff in the UI (YAGNI; pick a sensible default).
- Adding Vitest or any frontend test framework (scope creep; defer to a separate PR).
- HDR / gain-map / XMP metadata handling (YAGNI).

## Architectural Decision: Why Rust-Native

Three options were evaluated:

| Option | Result | Verdict |
|---|---|---|
| **A. Rust-native encoders** (mozjpeg, oxipng, imagequant, webp, ravif) — static link into Tauri binary | True single-file binary, ~25–28 MB total | **Chosen** |
| B. Bundle Node + sidecar + Sharp via `pkg` / Node SEA | Sharp's libvips dynamic library cannot be truly static-embedded; binary ~80–120 MB; first-run unpacks to temp; macOS notarization complications | Rejected |
| C. Bundle native CLI tools (cjpeg, pngquant, cwebp, avifenc) as multiple sidecars | Still multi-file; 4 codecs × 2 platforms = 8 binaries to maintain | Rejected |

Option A is the only path that satisfies G1.

### Crate Selection (verified against 2026 ecosystem)

| Format | Crate | Notes |
|---|---|---|
| JPEG (encode) | `mozjpeg` 0.10 | MozJPEG with trellis quantization; same encoder as Squoosh, ImageOptim |
| JPEG (decode) | `mozjpeg::Decompress` | Faster than `image`'s decoder; preserves metadata for our pipeline |
| PNG lossless | `oxipng` 10.x | Pure Rust; no contest |
| PNG lossy palette | `imagequant` 4 | pngquant equivalent |
| WebP | `webp` 0.3 | Wraps libwebp (Google official); lossless when quality=100 |
| AVIF | `ravif` 0.13 | Pure Rust via rav1e |
| Decode (PNG/WebP/AVIF source) | `image` 0.25 | Unified RGBA decode for non-JPEG inputs |
| EXIF | `kamadak-exif` 0.6 | Read orientation; pixels rotated and EXIF stripped on encode |

`zenjpeg` and `zenwebp` were considered (pure-Rust alternatives that avoid the C toolchain). Rejected for now: too new (2025) for a quality-critical pipeline; we can revisit if maintenance becomes painful.

## §1 Architecture

### Before

```
WebView (TS)  ──invoke──▶  Rust (Tauri)  ──JSON-lines via stdin/stdout──▶  Node sidecar (Sharp)
```

### After

```
WebView (TS)  ──invoke──▶  Rust (Tauri)
                              ├─ encoder module (mozjpeg / oxipng / imagequant / webp / ravif)
                              ├─ tokio Semaphore for concurrency control
                              └─ EventSink trait for IPC emission (real or mock)
```

### Files removed

- `desktop/src-tauri/src/sidecar.rs`
- `desktop/src-tauri/binaries/node-*`
- `desktop/src-tauri/sidecar-deps/`
- `desktop/scripts/fetch-node.sh`
- `desktop/scripts/copy-sidecar-deps.mjs`
- `index.js`, `src/sidecar.js`, `src/encoder.js`, `src/config.js`, `src/*.test.js`
- `tauri.conf.json` keys: `bundle.externalBin`, sidecar entries under `bundle.resources`

### Files added

- `desktop/src-tauri/src/encoder/mod.rs` — facade + `EncodeRequest` / `EncodeResult` / `EncodeOutcome` / `EncodeError`
- `desktop/src-tauri/src/encoder/jpeg.rs`
- `desktop/src-tauri/src/encoder/png.rs`
- `desktop/src-tauri/src/encoder/webp.rs`
- `desktop/src-tauri/src/encoder/avif.rs`
- `desktop/src-tauri/src/encoder/event_sink.rs` — `EventSink` trait, `TauriEmitter`, `MockSink`
- `desktop/src-tauri/tests/integration.rs`
- `desktop/src-tauri/tests/ipc_schema.rs`
- `desktop/src-tauri/tests/fixtures/` (8 test images)
- `desktop/scripts/build-dist.mjs` (replaces `build-portable.mjs`)
- `.github/workflows/release.yml`

### Files heavily modified

- `desktop/src-tauri/Cargo.toml` — new dependencies + `[profile.release]` tuning
- `desktop/src-tauri/src/commands.rs` — `compress` rewritten; `SidecarState`/`ensure_sidecar` removed
- `desktop/src-tauri/src/batch.rs` — atomic `completed` counter + `batch-done` emission
- `desktop/src-tauri/src/lib.rs` — sidecar state/setup removed
- `desktop/src-tauri/tauri.conf.json` — bundle section pruned
- `desktop/src/ipc.ts` — add `batch-done` listener; consume new `FileDonePayload` shape
- Root `README.md`, `CLAUDE.md` — CLI section retired; prerequisites updated

### Frontend impact (minimal)

- IPC `invoke('compress', ...)` signature unchanged.
- `EncodeOpts` (jpegQuality / pngQuality / webpQuality / avifQuality / emitWebp / emitAvif) unchanged.
- `FileDonePayload` is **slightly leaner** — the `tmp` field is removed (frontend never read it).
- A new `batch-done` event must be listened for to unlock the Undo button reliably.
- No other UI changes.

## §2 IPC Contract

### Locked event payloads (camelCase, serialized via serde)

```rust
pub struct FileDonePayload {
    pub id: String,
    pub src_bytes: u64,    // serialized as srcBytes
    pub out_bytes: u64,    // serialized as outBytes
}

pub struct FileErrorPayload {
    pub id: String,
    pub msg: String,
}

// Ad-hoc (preserved from current behavior):
// "file-skipped"     → { id, srcBytes }
// "companion-error"  → { id, ext, msg }
// "trash-fallback"   → { id, note }
// "batch-done"       → { batchId }   ← NEW
```

`sidecar-crashed` event is **removed** (no sidecar exists). Frontend listener for it must be deleted.

### Schema enforcement

`tests/ipc_schema.rs` uses `insta::assert_json_snapshot!` to lock every emitted payload's shape. Any silent renaming or type change surfaces as a snapshot diff in PR review.

### Concurrency model

```rust
let concurrency = (num_cpus::get() - 1).max(1).min(8);
let semaphore = Arc::new(Semaphore::new(concurrency));
```

- Reserve 1 core for system + UI thread.
- Cap at 8 to prevent RAM thrashing on high-core machines (AVIF encoding is RAM-hungry).
- Not exposed in Settings (YAGNI; revisit if power users complain).

Each file is a `tokio::spawn` task that acquires a permit, then runs the CPU-bound work via `tokio::task::spawn_blocking`. Results flow back over the per-task path: place tmp file → trash original → emit `file-done`. Companions are processed sequentially **within** a single file's task to share the decoded RGBA buffer.

## §3 Encoder Module

### Facade

```rust
pub struct EncodeRequest<'a> {
    pub src_path: &'a Path,
    pub ext: ImageExt,        // Jpeg | Png | Webp | Avif
    pub opts: &'a EncodeOpts,
}

pub struct EncodeResult {
    pub main: EncodedFile,
    pub companions: Vec<EncodedFile>,
}

pub struct EncodedFile {
    pub ext: ImageExt,
    pub tmp_path: PathBuf,
    pub bytes: u64,
}

pub enum EncodeOutcome {
    Encoded(EncodeResult),
    SkippedNoGain { src_bytes: u64 },
}

pub fn encode(req: EncodeRequest) -> Result<EncodeOutcome, EncodeError>;
```

### Decode strategy

| Input | Decoder | Reason |
|---|---|---|
| JPEG | `mozjpeg::Decompress` | Faster than `image::jpeg`; integrates with EXIF read |
| PNG | `image::ImageReader` | Sufficient; preserves ICC |
| WebP | `image::ImageReader` (feature `webp`) | Re-compress case is rare but supported |
| AVIF | `image::ImageReader` (feature `avif`) | dav1d backend |

### Metadata handling

- **EXIF orientation**: read via `kamadak-exif`, rotate the decoded RGBA buffer to upright, then strip EXIF on encode. This guarantees consistent visual presentation across all viewers (some still ignore the EXIF orientation tag). The rotated buffer is shared by main + all companion encoders, so WebP/AVIF outputs are also pre-rotated and EXIF-free.
- **ICC profile**: read raw bytes from input, embed in output. No color-management transforms.
- **Other metadata** (XMP, IPTC, GPS): stripped. Reduces file size; users uploading to Hando expect compression, not archival.

### Per-format encode parameters

#### JPEG (`encoder/jpeg.rs`)

```rust
mozjpeg::Compress::new(ColorSpace::JCS_RGB)
    .set_quality(opts.jpeg_quality as f32)
    .set_progressive_mode()
    .set_optimize_scans(true)   // trellis quantization
```

#### PNG (`encoder/png.rs`) — two stages

1. If `png_quality < 100` (lossy): run `imagequant` palette quantization at the requested quality.
2. Always: pass output through `oxipng::optimize_from_memory(..., Options::from_preset(2))` for lossless DEFLATE optimization.

#### WebP (`encoder/webp.rs`)

```rust
let encoder = webp::Encoder::from_rgba(&rgba, w, h);
if opts.webp_quality >= 100 {
    encoder.encode_lossless()
} else {
    encoder.encode(opts.webp_quality as f32)
}
```

#### AVIF (`encoder/avif.rs`)

```rust
let pixels = (w as u64) * (h as u64);
let threads = if pixels > 8_000_000 { 1 } else { 2 };

ravif::Encoder::new()
    .with_quality(opts.avif_quality as f32)
    .with_speed(6)                  // see rationale below
    .with_num_threads(Some(threads))
    .encode_rgba(img)
```

**`speed=6` rationale**: speed=4 (Squoosh default) gives 5–10s for a 1080p image — users perceive it as a hang without granular progress UI. speed=6 finishes in 1.5–3s with files only ~5–10% larger; a better psychological fit. Not exposed in Settings.

**Memory-aware thread throttling**: ravif holds the entire decoded RGBA buffer plus working state in RAM, and the rav1e backend further multiplies allocations per thread. A worst-case scenario — 8 simultaneous 6000×4000 photos with 2 threads each — can push peak RSS past 4 GB and trigger OOM on entry-level machines. The per-task `num_threads` is dialed down to 1 for inputs above ~8 megapixels, trading a small per-file slowdown for predictable memory behavior under concurrent load. (A dedicated "heavy task" semaphore for AVIF specifically is the next escalation if this proves insufficient — deferred until evidence demands it.)

### "No gain" handling

If `encoded_bytes >= src_bytes` for the main file, return `SkippedNoGain` and do not write the tmp file. Frontend receives `file-skipped`. Companions are **always** emitted (they're net-new files, not replacements).

## §4 Command Flow & Error Handling

### `compress` command

```rust
#[tauri::command]
pub async fn compress(
    app: AppHandle,
    batches: State<'_, BatchState>,
    args: CompressArgs,
) -> Result<(), String> {
    batches.start(&args.batch_id, args.files.len());
    let semaphore = Arc::new(Semaphore::new((num_cpus::get() - 1).max(1).min(8)));
    let opts = Arc::new(args.opts);
    let sink = Arc::new(TauriEmitter::new(app.clone()));

    for f in args.files {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let app_c = app.clone();
        let opts_c = opts.clone();
        let sink_c = sink.clone();
        let batch_id = args.batch_id.clone();
        let move_to_trash = args.move_originals_to_trash;

        tokio::spawn(async move {
            let _permit = permit;
            let f_clone = f.clone();
            let result = tokio::task::spawn_blocking(move || {
                encoder::encode(EncodeRequest {
                    src_path: Path::new(&f_clone.path),
                    ext: ImageExt::from_str(&f_clone.ext)?,
                    opts: &opts_c,
                })
            }).await;

            match result {
                Ok(Ok(EncodeOutcome::Encoded(res))) => {
                    let batches = app_c.state::<BatchState>();
                    if let Err(msg) = apply_done(&app_c, &batch_id, &f, res, move_to_trash, &batches, &*sink_c).await {
                        sink_c.emit_file_error(&f.id, &msg);
                    }
                }
                Ok(Ok(EncodeOutcome::SkippedNoGain { src_bytes })) => {
                    sink_c.emit_file_skipped(&f.id, src_bytes);
                }
                Ok(Err(e)) => sink_c.emit_file_error(&f.id, &e.to_string()),
                Err(join_err) => sink_c.emit_file_error(&f.id, &format!("encoder panic: {join_err}")),
            }

            // Tick batch counter; emit batch-done if last
            let batches = app_c.state::<BatchState>();
            batches.tick(&batch_id, &*sink_c);
        });
    }

    Ok(())
}
```

The command returns immediately; results flow asynchronously via events. This matches existing behavior (frontend already listens, no logic change needed there).

### Batch-done detection

```rust
// batch.rs
pub struct BatchInner {
    expected: usize,
    completed: AtomicUsize,
    disposals: Mutex<Vec<Disposal>>,
}

impl BatchState {
    pub fn tick(&self, batch_id: &str, sink: &dyn EventSink) {
        if let Some(batch) = self.get(batch_id) {
            let prev = batch.completed.fetch_add(1, Ordering::SeqCst);
            if prev + 1 == batch.expected {
                sink.emit_batch_done(batch_id);
                self.complete(batch_id);
            }
        }
    }
}
```

`Ordering::SeqCst` is used deliberately; the batch-done transition is a one-shot global state change where simplicity beats micro-optimization.

### Error mapping

| Source | Variant | Frontend event |
|---|---|---|
| Decode failure | `EncodeError::Decode` | `file-error` |
| Encode failure | `EncodeError::Encode` | `file-error` |
| IO failure | `EncodeError::Io` | `file-error` |
| Unknown extension | `EncodeError::UnsupportedFormat` | `file-error` |
| No size gain | `EncodeOutcome::SkippedNoGain` | `file-skipped` |
| Companion failure (main OK) | (companion failure within Encoded result) | `file-done` + `companion-error` |
| Encoder panic | `JoinError` from `spawn_blocking` | `file-error` (app stays alive) |
| Trash unavailable | Existing `RenamedFallback` | `trash-fallback` |

### Cancellation

Not implemented. Future addition would inject a `CancellationToken` per batch and check it inside `spawn_blocking` between decode and encode stages.

## §5 Build, Packaging, Platform

### Toolchain prerequisites

| Platform | Components |
|---|---|
| Windows x64 | Rust stable (≥ 1.85.1), MSVC Build Tools 2022 (C++), **NASM** (`winget install nasm`) |
| macOS arm64 | Rust stable, Xcode Command Line Tools, **NASM** (`brew install nasm`) |
| macOS x64 | Same as arm64; universal2 build runs both then `lipo` (handled by `tauri-action`) |

**Windows shell note**: `mozjpeg-sys` invokes the C compiler and NASM through `cc-rs`, which expects MSVC environment variables (`INCLUDE`, `LIB`, `PATH` to `cl.exe`) to be set. The cleanest setup is to run `cargo build` from the **"x64 Native Tools Command Prompt for VS 2022"** shell. PowerShell or generic terminals work only if the developer has previously run `vcvarsall.bat x64` or installed the Build Tools with the "Add to PATH" option. README must call this out explicitly to spare the next person an hour of cryptic linker errors.

### Cargo dependencies (additions)

```toml
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

[profile.release]
lto = true                # use lto = "thin" if local release builds become painful
codegen-units = 1
strip = true
opt-level = 3
panic = "unwind"          # required for spawn_blocking panic isolation
```

`panic = "unwind"` is **mandatory** for the panic-isolation design in §4. The ~500 KB savings from `abort` is not worth losing graceful per-file failure handling.

### tauri.conf.json bundle section (post-cleanup)

```json
{
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [...],
    "windows": { "nsis": { "installMode": "both", "headerImage": "..." } }
  }
}
```

No more `externalBin`, no more sidecar `resources`.

### Expected binary size

| Component | Size |
|---|---|
| Tauri base | ~7 MB |
| mozjpeg static | ~2 MB |
| oxipng + flate2 | ~1 MB |
| imagequant | ~0.5 MB |
| libwebp static | ~1.5 MB |
| rav1e (largest) | ~12 MB |
| Other | ~1 MB |
| **Total** | **~25–28 MB single file** |

Compared to the current portable folder (~50 MB+ across multiple files), this is a net reduction.

### Scripts directory

Keep `desktop/scripts/`, repurpose for post-build:

- `build-dist.mjs`: rename artifacts to `Hando-{platform}-{arch}-v{version}.{ext}`, zip for portable distribution, prepare for code-signing/notarization.
- Old scripts (`fetch-node.sh`, `copy-sidecar-deps.mjs`, `build-portable.mjs`) deleted.

**Cross-platform implementation note**: keep all custom build logic in Node.js (`.mjs`) rather than shell scripts. Use `node:path`, `node:fs/promises`, and `child_process.spawn` instead of platform-specific commands. If a tool-presence check is unavoidable (e.g., verifying NASM on the runner), prefer probing via `spawn('nasm', ['-v'])` and catching `ENOENT` over invoking `which` / `where` — the former is portable, the latter is not. CI's Windows runners are particularly unforgiving about path separators and quoting.

### CI/CD

`.github/workflows/release.yml` using `tauri-apps/tauri-action`:

- Matrix: `windows-latest`, `macos-14` (universal2 via `--target universal-apple-darwin`).
- No cross-compile attempted (native runners only — required for C-binding crates).
- Artifacts uploaded with the naming scheme above.

## §6 Testing Strategy

### Layers

```
┌─────────────────────────────────────┐
│ Manual E2E checklist (PR template)  │
├─────────────────────────────────────┤
│ Integration tests (Rust, MockSink)  │
├─────────────────────────────────────┤
│ Unit tests (encoder/* + batch.rs)   │
└─────────────────────────────────────┘
```

### EventSink abstraction (key for testability)

```rust
pub trait EventSink: Send + Sync {
    fn emit_file_done(&self, payload: FileDonePayload);
    fn emit_file_error(&self, id: &str, msg: &str);
    fn emit_file_skipped(&self, id: &str, src_bytes: u64);
    fn emit_companion_error(&self, id: &str, ext: &str, msg: &str);
    fn emit_trash_fallback(&self, id: &str, note: &str);
    fn emit_batch_done(&self, batch_id: &str);
}

pub struct TauriEmitter { app: AppHandle }
pub struct MockSink { events: Mutex<Vec<MockEvent>> }
```

Production code injects `TauriEmitter`; integration tests inject `MockSink` and assert on collected events. This avoids the brittle `tauri::test::mock_app()` API entirely.

### Unit tests (per encoder module)

- `encodes_smaller_than_input`
- `respects_quality_setting` (low Q → smaller bytes)
- `skipped_no_gain_when_already_optimized`
- `decode_failure_returns_error` (random bytes input)
- `exif_orientation_applied` (input orientation=6 → output upright + EXIF stripped)
- `icc_profile_preserved`

AVIF additionally: `speed_6_finishes_under_5s_for_1080p` (release-mode CI gate).

`batch.rs`:
- `tick_completes_when_all_done`
- `concurrent_tick_safe` (100 threads tick simultaneously → counter correct)

### Integration tests (`tests/integration.rs`)

Exercise the encoder facade through a `MockSink`:

- Compress a 5-file batch → assert `file-done` count, `batch-done` count, ordering invariants.
- Mix of success / corrupt / no-gain inputs → correct event types in correct quantities.
- Companion emission paths (emitWebp + emitAvif both on).

### IPC schema snapshot tests (`tests/ipc_schema.rs`)

`insta::assert_json_snapshot!` on every payload variant. Snapshots committed to repo; PR diffs catch silent breakage.

### Fixtures (`tests/fixtures/`)

8 representative images covering boundary conditions:

- `landscape.jpg` — 1920×1080 photo
- `screenshot.png` — 1440×900 (good for palette quantization)
- `transparent.png` — alpha channel
- `portrait_exif_rotated.jpg` — orientation=6
- `with_icc.jpg` — embedded ICC profile
- `tiny.png` — 128×128, already optimized → expect SkippedNoGain
- `corrupt.jpg` — truncated bytes → expect decode error
- `large_photo.jpg` — 6000×4000 → tests memory pressure under concurrent load

### A/B comparison (Phase 2 land gate — **mandatory, manual**)

Before merging Phase 2, encode a representative set of 10–20 images with both pipelines (current Sharp sidecar vs. new Rust encoder) at identical quality settings. Validate:

- File-size delta ≤ ±5% per image.
- No perceptible visual regression (eyeball or SSIM).
- EXIF orientation differences are expected (new pipeline rotates pixels; old may pass through).

If results fail, tune quality defaults or encoder parameters before landing.

### Frontend tests

Not added in this refactor. The IPC contract is locked by Rust-side snapshot tests; the only frontend change is one new event listener. Manual E2E covers it.

### Manual E2E checklist (PR description)

```
[ ] Drop 5 mixed-format images (JPG/PNG/WebP/AVIF) → all compress
[ ] Enable emit WebP + AVIF → companions appear in source folder
[ ] Drop 1 tiny pre-optimized image → shows "skipped"
[ ] Drop 1 corrupt JPG → file-error shown, others succeed
[ ] Undo restores originals from Recycle Bin, deletes companions
[ ] With Recycle Bin disabled → fallback to .original rename + notice
[ ] EXIF orientation=6 portrait → output upright in multiple viewers
[ ] Drop 50 images at once → CPU/RAM stay reasonable, UI stays responsive
[ ] Single-file binary launches by double-click on a clean machine (no dev tools installed)
```

## §7 Phased Migration Plan

**Four PRs**, each independently shippable and leaving the app in a working state. (The original 5-phase split collapsed Phase 3 + 4 — sidecar removal and CLI removal — into a single "Node Purge" PR for review-context coherence.)

### Phase 1: Encoder facade scaffold (additive only)

- Add `encoder/*` modules + `EventSink` trait + `MockSink`.
- Add Cargo dependencies + `[profile.release]` tuning.
- Add unit tests + fixtures.
- **Untouched**: `commands.rs`, `sidecar.rs`, frontend, Node CLI. Sidecar continues to work.

**Land gate**: `cargo test` green; binary builds.

**Risk**: Low (pure addition).

### Phase 2: commands.rs cuts over to new encoder

- Rewrite `commands.rs::compress` using `spawn_blocking` + `Semaphore` + encoder facade.
- Remove `ensure_sidecar`, `SidecarState`, `on_sidecar_crashed`.
- Rewrite `batch.rs` with atomic `completed` counter; emit `batch-done`.
- Slim `FileDonePayload` (remove `tmp` field) + insta snapshots.
- Integration tests (using MockSink).
- **Frontend**: add `batch-done` listener for Undo unlock.

**Land gates** (all mandatory):
- A/B comparison test on 10–20 images passes (≤ ±5% size delta, no visual regression).
- Manual E2E checklist passes end-to-end.
- Sidecar code still present but no longer reachable.

**Risk**: Medium. This is the core cutover.

### Phase 3: The Node Purge (Phase 3 + 4 merged)

- Delete `desktop/src-tauri/src/sidecar.rs`.
- Delete `desktop/src-tauri/binaries/`, `sidecar-deps/`.
- Delete `desktop/scripts/fetch-node.sh`, `copy-sidecar-deps.mjs`.
- Replace `build-portable.mjs` with `build-dist.mjs` (rename + zip).
- Prune `tauri.conf.json` (`externalBin`, sidecar `resources`).
- Clean `lib.rs` (sidecar registration).
- Delete `index.js`, `src/sidecar.js`, `src/encoder.js`, `src/config.js`, `src/*.test.js`.
- Remove root `package.json` `bin` entry; remove `hando` link instructions.
- Update root `README.md` / `CLAUDE.md`: CLI section retired; Architecture section rewritten.

**Land gate**: Tauri release build produces a single-file binary that launches by double-click. Repo no longer references Node.js outside of frontend dev tooling.

**Risk**: Low (pure deletion after Phase 2 validates the new path).

### Phase 4: CI/CD matrix + docs

- Add `.github/workflows/release.yml` using `tauri-apps/tauri-action`.
- Matrix: `windows-latest` + `macos-14` (universal2).
- Artifact naming: `Hando-{platform}-{arch}-v{version}.{ext}`.
- Update README prerequisites section (NASM / MSVC / Xcode CLT).

**Land gate**: A pushed tag produces both Windows and macOS-universal artifacts.

**Risk**: Medium (CI setup is fiddly the first time).

### Interruption recovery

| Stop after | App state | Recovery cost |
|---|---|---|
| Phase 1 | Sidecar still working; new encoder code idle | Zero |
| Phase 2 | Desktop using Rust encoder; sidecar code dead but present | Low (cleanup PR) |
| Phase 3 | Desktop fully self-contained; CLI gone; CI not yet wired | Zero (manual builds still work) |
| Phase 4 | Complete | — |

## Open Questions

None at spec sign-off. Anything that arises during Phase 1–4 implementation will be raised as plan-level issues.

## Appendix: Glossary

- **Sidecar**: an external process Tauri spawns alongside the main binary (here, the Node.js encoder process being removed).
- **Trellis quantization**: an mozjpeg optimization that re-encodes DCT coefficients to minimize rate-distortion; produces smaller files at the same visual quality.
- **Universal binary** (macOS): a single executable containing both x86_64 and arm64 slices, selected at runtime.
- **Companion**: a secondary output format (WebP or AVIF) generated alongside the main encoded file when the corresponding setting is enabled.
