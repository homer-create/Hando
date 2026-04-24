# imageopt

A small CLI that recursively compresses images with [Sharp](https://sharp.pixelplumbing.com) and emits a matching `.webp` alongside each file, ready for `<picture>` fallback.

## Install

```bash
git clone <this-repo>
cd ImageOpt
npm install
npm link          # exposes the `imageopt` command globally
```

## Usage

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

## Incremental builds

On re-runs, each output is compared to its source by mtime. Outputs that are at least as new as the source are skipped. Touching a source file forces re-encoding on the next run.

## Configuration

Compression settings are constants at the top of `index.js`:

```js
const CONFIG = {
  JPEG_QUALITY: 75,
  PNG_QUALITY: 75,
  WEBP_QUALITY: 75,
  EXTENSIONS: ['.jpg', '.jpeg', '.png', '.webp'],
  CONCURRENCY: 4,
};
```

Edit and re-run.
