# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### CLI

```bash
npm install                                  # install dependencies
node index.js <input-dir> -o <output-dir>    # run directly
npm link                                     # expose `imageopt` globally
imageopt <input-dir> -o <output-dir>         # run via linked CLI
```

### Node tests

```bash
node --test src/*.test.js        # run all unit tests
node --test src/config.test.js   # run a single test file
```

### Desktop app (Tauri, under `desktop/`)

```bash
cd desktop && npm install              # install frontend deps
cd desktop && npm run tauri dev        # start the desktop app in dev mode
cd desktop/src-tauri && cargo build    # compile Rust only
```

**First-time setup (Windows):** Download the Node binary required by the Tauri bundler:

```powershell
# From repo root — downloads node-x86_64-pc-windows-msvc.exe into desktop/src-tauri/binaries/
powershell -c "Invoke-WebRequest https://nodejs.org/dist/v20.11.1/node-v20.11.1-win-x64.zip -OutFile $env:TEMP\node.zip; Expand-Archive $env:TEMP\node.zip $env:TEMP\node-dl -Force; Copy-Item $env:TEMP\node-dl\node-v20.11.1-win-x64\node.exe desktop\src-tauri\binaries\node-x86_64-pc-windows-msvc.exe"
```

## Architecture

### CLI (`index.js` + `src/`)

Single ES-module entry point (Node ≥ 18, `sharp`).

Shared modules under `src/`:
- **`src/config.js`** — exports `CONFIG` (quality defaults, extensions, concurrency)
- **`src/encoder.js`** — exports `ENCODERS` map and `encode({ srcPath, dstPath, ext, opts })` via Sharp

Pipeline in `index.js`:
1. **`parseArgs`** — parses `<input-dir> -o <output-dir>`; exits on bad input
2. **`discoverImages`** — recursive walk collecting `.jpg/.jpeg/.png/.webp`
3. **`runPool`** — bounded concurrency pool (default 4 workers)
4. **`processFile`** — per file: calls `encode()` for original format + `.webp` variant via `writeIfNeeded`
5. **`writeIfNeeded`** — skips if output mtime ≥ source mtime (incremental builds)

### Desktop app (Tauri 2.x, `desktop/`)

Three-tier process model:

```
WebView (vanilla TS + Vite)
    ↕ invoke / events (Tauri IPC)
Rust host (tokio, serde_json, trash, uuid)
    ↕ JSON-lines over stdin/stdout
Node sidecar (src/sidecar.js — imports src/encoder.js)
```

**Rust host (`desktop/src-tauri/src/`):**
- `sidecar.rs` — spawns Node sidecar, async JSON-lines reader/writer over tokio channels; emits `sidecar-crashed` on EOF
- `commands.rs` — `compress`: sends encode jobs to sidecar, applies Trash + copy-fallback rename per file, tracks companion paths; `undo_last_batch`: deletes compressed files + companions and restores originals from Trash; `open_trash`, `confirm_close`
- `batch.rs` — tracks disposals (including companion paths) per batch for Undo
- `trash.rs` — wraps `trash` crate with `\\?\` prefix stripping for Windows Recycle Bin compatibility; falls back to `.original` rename if Trash unavailable
- Plugins: `tauri-plugin-store` (settings), `tauri-plugin-fs` (folder expansion), `tauri-plugin-dialog` (file picker)

**Frontend (`desktop/src/`):**
- `state.ts` — reactive `Store` (Map keyed by path + subscriber set), `FileRow` type
- `ipc.ts` — typed wrappers for `invoke('compress')`, `invoke('undo_last_batch')`, `invoke('open_trash')`, and `listen()` for `file-done / file-error / file-skipped` events; `toOpts()` maps Settings → EncodeOpts
- `fs.ts` — `expandPaths()`: recursive folder expansion via `tauri-plugin-fs`, filters to supported extensions
- `ui/dropzone.ts` — uses `getCurrentWindow().onDragDropEvent()` (Tauri 2 API) for drag-drop; `@tauri-apps/plugin-dialog` `open()` for click-to-add
- `ui/file-list.ts` — subscribes to store, renders grid rows with status icons, size columns, savings %
- `ui/toolbar.ts` — Settings + Undo buttons; returns `ToolbarApi` with `setUndoEnabled`
- `ui/settings.ts` — overlay panel with JPEG/PNG/WebP/AVIF quality sliders and Emit WebP/AVIF toggles; persisted via `tauri-plugin-store`
- `ui/statusbar.ts` — shows progress bar during compression; cumulative saved bytes + Show Trash link when done

**Node sidecar (`src/sidecar.js`):**
Reads JSON-lines commands from stdin, encodes via `encode()`, writes JSON-lines events to stdout. Concurrency pool (`CONFIG.CONCURRENCY = 4`). Events: `done`, `error`, `skipped-no-gain`, `companion-error`, `parse-error`. Supports `emitWebp` and `emitAvif` companion outputs.

**Supported formats:** JPEG, PNG, WebP, AVIF (encode + companion output).

**Key data flow for a compress batch:**
1. User drops files / clicks to add → `expandPaths` filters to supported extensions
2. Frontend `invoke('compress', { batchId, files, opts, moveOriginalsToTrash })`
3. Rust sends one `encode` JSON-line per file to Node sidecar
4. Sidecar encodes main + optional WebP/AVIF companions to temp paths, emits `done`
5. Rust receives `done`: trashes original (with `\\?\` strip for Windows), copy-fallback renames temp → src, places companions alongside, records disposal + companion paths
6. Rust emits `file-done` Tauri event → frontend updates store row

**Undo:** `BatchState` tracks `Disposal` records (Trash path or `.original` backup + companion paths). `undo_last_batch` deletes compressed files and companions, then calls `trash::restore_all` to recover originals from Recycle Bin.

**Windows-specific notes:**
- `Path::canonicalize()` returns `\\?\`-prefixed paths; `trash.rs` strips this before storing so Recycle Bin lookup matches
- Cross-drive `fs::rename` fails (e.g. `C:\Temp` → `D:\files`); `commands.rs` falls back to `fs::copy` + `remove_file`
- Node binary must be pre-downloaded to `desktop/src-tauri/binaries/node-x86_64-pc-windows-msvc.exe` (gitignored)

## Docs

Design specs and implementation plans live in `docs/superpowers/`:
- `specs/` — design specs (CLI and desktop app)
- `plans/` — implementation plans; desktop plan is `2026-04-24-imageopt-desktop.md`
