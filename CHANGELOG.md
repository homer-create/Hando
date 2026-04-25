# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased] — v0.1.0

> Pending: tag-push smoke test, macOS CI validation, clean-machine install test.
> See [docs/release-checklist.md](docs/release-checklist.md).

### Fixed
- **macOS CI bundle** — `generate-fixtures` moved from `[[bin]]` to `[[example]]`; Tauri bundles all `[[bin]]` entries but skips examples, so the bundler no longer tries to copy the uncompiled dev utility and fails with "does not exist"
- **macOS build** — `trash::os_limited` (list + restore) is gated to Windows/Linux in `trash` 5.x; wrapped the undo restore path in `#[cfg(...)]` so macOS compiles cleanly (trashed files remain in Trash on macOS, as the crate exposes no programmatic restore API there)
- **CI release creation** — added `permissions: contents: write` to `release.yml`; without it `GITHUB_TOKEN` couldn't create the draft release

### Changed
- **AVIF encoding speed** — `ravif` speed raised from 6 → 8 and per-encode thread count changed from a fixed 1–2 to `num_cpus / 2` clamped to [2, 4]; typical encode time drops ~3-4x with negligible quality difference
- **Replaced Node sidecar with Rust-native encoder** — encoding pipeline now runs entirely in-process via Rust crates; eliminates the bundled Node binary and JSON-lines sidecar protocol
- Standalone CLI (`index.js`) removed; desktop app is now the only artifact
- Portable build script replaced with a `dist/` artifact organizer

### Added — Rust encoder (`desktop/src-tauri/src/encoder/`)
- JPEG encoder via `mozjpeg`
- PNG encoder via `imagequant` + `oxipng`
- WebP encoder via `libwebp`
- AVIF encoder via `ravif` with memory-aware threading
- RGBA decode with EXIF orientation normalization
- `EventSink` trait + `MockSink` for unit testing without Tauri
- `TauriEmitter` production sink
- Stage-based progress events (decode → encode → companion → done)
- Atomic batch completion counter with `tick()`
- Encoder fixture suite with a synthetic image generator

### Added — CI / release
- Cross-platform `release.yml` (GitHub Actions + `tauri-action`): builds Windows `.exe` and macOS `.dmg` on tag push, creates a draft GitHub Release
- Tagged-release process documented in `docs/`

### Fixed
- Progress animation, batch reset, and 2% skip threshold
- Ambiguous `cargo run` resolved via `default-run = hando`
- `tauri.conf.json` pruned of stale `externalBin` / sidecar resource entries

---

## [0.0.x] — Desktop App with Node Sidecar *(2026-04-24)*

Initial Tauri desktop app. Encoding handled by a bundled Node sidecar process (`src/sidecar.js`) communicating over JSON-lines stdin/stdout.

### Added — Desktop app
- Tauri 2 project scaffolded under `desktop/`
- Three-tier architecture: WebView ↔ Rust host ↔ Node sidecar
- Rust `compress` command dispatches encode jobs to sidecar
- Rust sidecar manager with JSON-lines protocol and tokio channels
- Atomic Trash + rename flow per compressed file (Windows Recycle Bin compatible)
- Undo last batch: deletes compressed files + companions, restores originals from Trash
- Recursive folder expansion with format filtering via `tauri-plugin-fs`
- Drag-drop (Tauri 2 API) + click-to-add file picker
- Settings panel: JPEG/PNG/WebP/AVIF quality sliders, Emit WebP/AVIF toggles; persisted via `tauri-plugin-store`
- File list with status icons, size columns, savings %
- Status bar with progress bar, cumulative saved bytes, Show Trash link
- Open-trash command and close-confirm dialog
- Sidecar crash recovery + `sidecar-crashed` event
- Cross-device rename fallback for Windows (C: temp → D: source)
- `\\?\` prefix stripping for Windows Recycle Bin compatibility
- AVIF encoding + Undo deletes companions + `avifQuality` setting
- WebP companion output placed alongside original

### Added — Branding & UI
- Hando brand icon (replaces Tauri placeholder)
- Goldman font for toolbar title
- Three-state theme resolution (system / light / dark) with `data-theme` management
- Theme preference persisted in Settings UI

### Added — Build
- Portable build pipeline: bundles `sharp` deps alongside app; `Hando-portable/` layout

### Added — CLI (precursor, later removed)
- Node CLI (`index.js`): recursive image discovery, bounded concurrency pool, mtime-based skip, WebP companion output, JPEG/PNG/WebP encoding via `sharp`
- Shared `src/config.js` and `src/encoder.js` modules (still used by sidecar at this stage)

---

[Unreleased]: https://github.com/homershie/Hando/compare/HEAD...HEAD
