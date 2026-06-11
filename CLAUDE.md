# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Desktop app (Tauri)

```bash
npm install                  # install frontend deps
npm run tauri dev            # start the desktop app in dev mode
npm run tauri build          # release build
npm run dist                 # release build + rename artifacts + zip
cd src-tauri && cargo build  # compile Rust only
cd src-tauri && cargo test   # run Rust unit + integration tests
```

**Windows toolchain note:** Run cargo from the *"x64 Native Tools Command Prompt for VS 2022"* shell. `mozjpeg-sys` and `webp-sys` need MSVC's `cl.exe` and NASM in PATH.

## Architecture

Single-file Tauri 2 desktop app. No CLI, no sidecar.

```
WebView (TypeScript + Vite)
    ↕ invoke / events (Tauri IPC)
Rust host (tokio, in-process encoders, trash)
    └─ encoder facade (encoder/mod.rs)
        ├─ jpeg.rs   (mozjpeg; + lossless DCT transcode via mozjpeg-sys)
        ├─ png.rs    (imagequant + oxipng)
        ├─ webp.rs   (libwebp via webp crate, encode_advanced)
        ├─ avif.rs   (ravif via rav1e; decode via avif-decode/libaom)
        ├─ judge.rs  (ssimulacra2 perceptual score + pixel-identity + bpp + jpeg-blockiness fingerprint)
        └─ auto.rs   (auto quality mode: quality-targeted binary search)
```

**Optimization rubric & benchmarks:** `docs/rubric.md` is the source of truth for
quality gates (hard constraints) vs. the size objective. The bench harness is
`cargo run --release --example bench` (`sweep` / `grid` / `calibrate` modes);
results in `docs/bench-results.md`, S-value calibration in `docs/calibration.md`.

**Rust host (`src-tauri/src/`):**
- `commands.rs` — `compress`: spawns one `tokio::task::spawn_blocking` per file, gated by a `Semaphore` of `(num_cpus - 1).clamp(1, 8)`; calls `encoder::encode()`; emits `file-done` / `file-error` / `file-skipped` / `companion-error` / `trash-fallback` events. `undo_last_batch`: deletes compressed files + companions and restores originals from Trash.
- `encoder/mod.rs` — `encode()` facade. Dispatches by `ImageExt` and `EncodeOpts.mode` (`manual` = fixed quality numbers, `auto` = quality-targeted). Returns `EncodeOutcome::Encoded(EncodeResult)` or `EncodeOutcome::SkippedNoGain`. JPEG gets a lossless-transcode second chance before skipping.
- `encoder/auto.rs` — Auto mode: binary-search the smallest quality clearing the ssimulacra2 target (`targetQuality`, UI presets 90/80/70); lossless + lossy candidates compete, smallest passing wins. Lossy sources are gated by bpp (B1 ≥ 1.0 bpp → preset target; B2 → lossless transcode only or `max(S, 90)`). Lossless containers carrying a JPEG 8×8 grid fingerprint (`judge::jpeg_blockiness` ≥ 1.25, i.e. a JPEG re-saved as PNG) are demoted to the B-class bound.
- `encoder/judge.rs` — ssimulacra2 score vs the baseline image (decoded original, orientation applied, metadata stripped), `pixels_identical`, `bits_per_pixel`.
- `encoder/decode.rs` — Unified RGBA decode. JPEG via `mozjpeg::Decompress`; PNG/WebP via `image::ImageReader`; AVIF via `avif-decode` (the image crate's avif feature is encode-only). Applies EXIF orientation by rotating the pixel buffer, then strips EXIF on encode.
- `encoder/event_sink.rs` — `EventSink` trait with `TauriEmitter` (production) and `MockSink` (tests). Avoids the brittle `tauri::test::mock_app()` API entirely.
- `batch.rs` — `BatchState` with atomic `completed`/`expected` counters; `tick()` emits `batch-done` when the last file completes.
- `trash.rs` — Wraps `trash` crate; `\\?\` prefix stripping for Windows; `.original` rename fallback.

**Frontend (`src/`):**
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

## Changelog rule

**Every code change must be recorded in `CHANGELOG.md` before the task is considered done.**

- Add entries under the correct `[Unreleased]` section (`### Added`, `### Changed`, `### Fixed`, or `### Removed`).
- One line per logical change; lead with **what** changed and include the **why / impact** (e.g. "~3x faster", "fixes crash on …").
- Do not batch multiple unrelated changes into one entry.

## Docs

Design specs and implementation plans live in `docs/superpowers/`:
- `specs/` — design specs (desktop app + rust-native encoder)
- `plans/` — implementation plans; the rust-native encoder refactor is `2026-04-25-rust-native-encoder.md`
