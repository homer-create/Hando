# imageopt

Recursively compress images with [Sharp](https://sharp.pixelplumbing.com). Ships as both a CLI and a Tauri desktop app.

- **CLI** — batch-compress a directory tree, emit `.webp` companions for `<picture>` fallback, and skip unchanged files on re-runs.
- **Desktop app** — drag-and-drop compression with Recycle Bin-backed Undo, optional WebP/AVIF companion output, and per-format quality settings. See [`desktop/`](./desktop).

## CLI

### Install

```bash
git clone <this-repo>
cd ImageOpt
npm install
npm link          # exposes the `imageopt` command globally
```

### Usage

```bash
imageopt <input-dir> -o <output-dir>
```

Example:

```bash
imageopt ./src/images -o ./dist/images
```

For each `.jpg` / `.jpeg` / `.png` in `<input-dir>`, two files are written:

- The compressed original format (`photo.jpg` → `dist/photo.jpg`)
- A WebP version (`photo.jpg` → `dist/photo.webp`)

Input `.webp` files emit a single compressed `.webp`.

Subfolders are recursed and the structure is preserved.

### Incremental builds

On re-runs, each output is compared to its source by mtime. Outputs that are at least as new as the source are skipped. Touching a source file forces re-encoding on the next run.

### Configuration

Compression defaults live in [`src/config.js`](./src/config.js):

```js
export const CONFIG = {
  JPEG_QUALITY: 75,
  PNG_QUALITY: 75,
  WEBP_QUALITY: 75,
  AVIF_QUALITY: 50,
  EXTENSIONS: ['.jpg', '.jpeg', '.png', '.webp'],
  CONCURRENCY: 4,
};
```

Edit and re-run. (The CLI writes original-format + WebP today; AVIF is exposed through the desktop app.)

### Tests

```bash
node --test src/*.test.js
```

## Desktop app

A Tauri 2.x app that wraps the same encoder pipeline with a drag-and-drop UI, Recycle Bin-backed Undo, and WebP/AVIF companion emission. See [`desktop/README.md`](./desktop/README.md) for setup and architecture.

## Project layout

```
index.js                     CLI entry point
src/
  config.js                  shared quality defaults
  encoder.js                 Sharp pipeline per extension
  sidecar.js                 JSON-lines worker used by the desktop app
  *.test.js                  node:test unit tests
desktop/
  src/                       Vite + TS frontend
  src-tauri/                 Rust host (tokio, trash, IPC)
docs/superpowers/            specs and implementation plans
```
