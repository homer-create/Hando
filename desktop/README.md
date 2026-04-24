# imageopt desktop

A Tauri 2.x desktop app that wraps the `imageopt` encoder with a drag-and-drop UI, Recycle Bin-backed Undo, and WebP/AVIF companion emission.

## Features

- Drag-and-drop or click-to-add JPEG / PNG / WebP / AVIF files
- Recursive folder expansion (directories are walked and filtered to supported formats)
- Per-format quality sliders (JPEG / PNG / WebP / AVIF), persisted via `tauri-plugin-store`
- Optional WebP and AVIF companion output alongside each compressed original
- Originals are moved to the Recycle Bin — Undo restores them (and removes companions)
- Live progress bar and cumulative "saved bytes" summary in the status bar
- Sidecar crash detection (`sidecar-crashed` event) with surfaced error state

## Architecture

Three-tier process model:

```
WebView (vanilla TS + Vite)
    ↕ invoke / events (Tauri IPC)
Rust host (tokio, serde_json, trash, uuid)
    ↕ JSON-lines over stdin/stdout
Node sidecar (src/sidecar.js — imports src/encoder.js)
```

- **Rust host** (`src-tauri/src/`): `sidecar.rs` spawns the Node process and bridges JSON-lines ↔ tokio channels; `commands.rs` exposes `compress`, `undo_last_batch`, `open_trash`, and `confirm_close`; `batch.rs` tracks per-batch disposals for Undo; `trash.rs` wraps the `trash` crate with Windows `\\?\` prefix handling.
- **Frontend** (`src/`): reactive `Store` in `state.ts`, typed IPC wrappers in `ipc.ts`, recursive folder expansion in `fs.ts`, and UI modules under `ui/` (dropzone, file list, toolbar, settings, status bar).
- **Node sidecar** (`../src/sidecar.js`): reads JSON-lines encode jobs from stdin, runs the shared Sharp pipeline with a concurrency pool of 4, emits `done` / `error` / `skipped-no-gain` / `companion-error` events.

See the root [`CLAUDE.md`](../CLAUDE.md) for the full data-flow walkthrough and Windows-specific notes.

## Development

```bash
npm install            # install frontend deps
npm run tauri dev      # start the app in dev mode
```

Rust-only rebuild:

```bash
cd src-tauri && cargo build
```

### First-time setup (Windows)

The Tauri bundler expects a Node binary at `src-tauri/binaries/node-x86_64-pc-windows-msvc.exe`. It is gitignored — download it once:

```powershell
# From the repo root
powershell -c "Invoke-WebRequest https://nodejs.org/dist/v20.11.1/node-v20.11.1-win-x64.zip -OutFile $env:TEMP\node.zip; Expand-Archive $env:TEMP\node.zip $env:TEMP\node-dl -Force; Copy-Item $env:TEMP\node-dl\node-v20.11.1-win-x64\node.exe desktop\src-tauri\binaries\node-x86_64-pc-windows-msvc.exe"
```

## Recommended IDE setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
