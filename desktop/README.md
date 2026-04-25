# Hando desktop

A Tauri 2.x desktop app that compresses images locally with native Rust encoders, drag-and-drop UX, and Recycle Bin-backed Undo.

## Features

- Drag-and-drop or click-to-add JPEG / PNG / WebP / AVIF files
- Recursive folder expansion (directories are walked and filtered to supported formats)
- Per-format quality sliders (JPEG / PNG / WebP / AVIF), persisted via `tauri-plugin-store`
- Optional WebP and AVIF companion output alongside each compressed original
- Originals are moved to the Recycle Bin — Undo restores them (and removes companions)
- Live progress bar and cumulative "saved bytes" summary in the status bar

## Architecture

Single-process Tauri 2 app. Encoding runs in-process; no sidecar.

```
WebView (vanilla TS + Vite)
    ↕ invoke / events (Tauri IPC)
Rust host (tokio, in-process encoders, trash)
    └─ encoder facade (encoder/mod.rs)
        ├─ jpeg.rs (mozjpeg)
        ├─ png.rs  (imagequant + oxipng)
        ├─ webp.rs (libwebp via webp crate)
        └─ avif.rs (ravif via rav1e)
```

- **Rust host** (`src-tauri/src/`): `commands.rs` exposes `compress`, `undo_last_batch`, `open_trash`, and `confirm_close`; `batch.rs` tracks per-batch disposals for Undo with an atomic completion counter that emits `batch-done` on the last file; `trash.rs` wraps the `trash` crate with Windows `\\?\` prefix handling.
- **Encoder** (`src-tauri/src/encoder/`): `mod.rs` is the facade with shared types; `decode.rs` produces RGBA buffers (mozjpeg for JPEG; `image::ImageReader` for PNG/WebP/AVIF) and applies EXIF orientation up-front so encoders can strip it; per-format files implement encode. `event_sink.rs` defines the `EventSink` trait with a Tauri-emitting impl for production and a `MockSink` for tests.
- **Frontend** (`src/`): reactive `Store` in `state.ts`, typed IPC wrappers in `ipc.ts`, recursive folder expansion in `fs.ts`, and UI modules under `ui/` (dropzone, file list, toolbar, settings, status bar). `main.ts` listens for `batch-done` to authoritatively unlock Undo.

See the root [`CLAUDE.md`](../CLAUDE.md) for the full data-flow walkthrough and Windows-specific notes.

## Development

```bash
npm install            # install frontend deps
npm run tauri dev      # start the app in dev mode
npm run tauri build    # release build
npm run dist           # release build + rename + zip into dist-final/
```

Rust-only rebuild + tests:

```bash
cd src-tauri && cargo build
cd src-tauri && cargo test
```

### First-time setup

Native Rust encoders need a C toolchain and NASM:

- **Windows**: Visual Studio 2022 Build Tools (Desktop development with C++) + NASM (`winget install nasm`). Run cargo from the *"x64 Native Tools Command Prompt for VS 2022"* shell — `mozjpeg-sys` and `webp-sys` need MSVC's `cl.exe` and NASM in PATH.
- **macOS**: Xcode Command Line Tools (`xcode-select --install`) + NASM (`brew install nasm`).

## Recommended IDE setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## License

Licensed under the GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later). See the root [`LICENSE`](../LICENSE) file for full text and the root [`README`](../README.md#license) for third-party component notices.
