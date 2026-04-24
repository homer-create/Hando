# ImageOpt CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a single-file Node CLI (`imageopt`) that recursively scans an input folder, compresses images with Sharp, additionally emits WebP alongside the compressed original, and uses mtime-based incremental builds.

**Architecture:** One entry file (`index.js`) with a shebang, config constants at the top, and three logical blocks: (1) CLI arg parsing & validation, (2) recursive file discovery, (3) per-file processing with concurrency + mtime skip. Declared as a `bin` in `package.json` so `npm link` exposes the `imageopt` command.

**Tech Stack:** Node.js (LTS), `sharp` for image processing, Node built-ins (`fs/promises`, `path`, `process`). No CLI parsing library.

**Testing note:** Per the spec, this project uses lightweight manual acceptance testing against a fixtures folder rather than per-function unit tests. Tasks include explicit manual verification runs; a smoke-test fixture is created in Task 2.

---

## File Structure

| File                                    | Responsibility                                              |
| --------------------------------------- | ----------------------------------------------------------- |
| `package.json`                          | Dependencies + `bin` declaration                            |
| `index.js`                              | Entire CLI: args, discovery, processing, reporting          |
| `.gitignore`                            | Exclude `node_modules`, `dist`, OS cruft                    |
| `README.md`                             | Install + usage                                             |
| `fixtures/sample/` (gitignored subtree) | Manual test images (user adds their own)                    |

---

## Task 1: Initialize project scaffolding

**Files:**
- Create: `.gitignore`
- Create: `package.json`
- Run: `git init`, `npm install sharp`

- [ ] **Step 1: Initialize git**

```bash
cd /Users/csii/Desktop/git/ImageOpt
git init
```

Expected: `Initialized empty Git repository in .../ImageOpt/.git/`

- [ ] **Step 2: Create `.gitignore`**

```gitignore
node_modules/
dist/
fixtures/
.DS_Store
*.log
```

- [ ] **Step 3: Create `package.json`**

```json
{
  "name": "imageopt",
  "version": "0.1.0",
  "description": "Recursively compress images and emit WebP using Sharp.",
  "type": "module",
  "bin": {
    "imageopt": "./index.js"
  },
  "scripts": {
    "start": "node index.js"
  },
  "engines": {
    "node": ">=18"
  },
  "license": "MIT"
}
```

Rationale: `"type": "module"` lets us use ESM `import` syntax; Node 18+ has stable `fs/promises` and top-level await.

- [ ] **Step 4: Install sharp**

Run: `npm install sharp`
Expected: `added N packages` with no errors. `node_modules/` and `package-lock.json` created.

- [ ] **Step 5: Commit the skeleton**

```bash
git add .gitignore package.json package-lock.json
git commit -m "chore: initialize project with sharp"
```

Note: `node_modules/` is gitignored; `package-lock.json` IS committed.

---

## Task 2: Create the CLI entry file with arg parsing

**Files:**
- Create: `index.js`

- [ ] **Step 1: Create `index.js` with shebang, CONFIG, and arg parser**

```js
#!/usr/bin/env node
import { stat } from 'node:fs/promises';
import { resolve } from 'node:path';

const CONFIG = {
  JPEG_QUALITY: 75,
  PNG_QUALITY: 75,
  WEBP_QUALITY: 75,
  EXTENSIONS: ['.jpg', '.jpeg', '.png', '.webp'],
  CONCURRENCY: 4,
};

function printUsageAndExit() {
  console.error('Usage: imageopt <input-dir> -o <output-dir>');
  process.exit(1);
}

function parseArgs(argv) {
  const args = argv.slice(2);
  let input = null;
  let output = null;
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === '-o' || a === '--output') {
      output = args[++i];
    } else if (!a.startsWith('-') && input === null) {
      input = a;
    } else {
      console.error(`Unknown argument: ${a}`);
      printUsageAndExit();
    }
  }
  if (!input || !output) printUsageAndExit();
  return { input: resolve(input), output: resolve(output) };
}

async function validateInputDir(dir) {
  try {
    const s = await stat(dir);
    if (!s.isDirectory()) {
      console.error(`Input is not a directory: ${dir}`);
      process.exit(1);
    }
  } catch {
    console.error(`Input directory does not exist: ${dir}`);
    process.exit(1);
  }
}

async function main() {
  const { input, output } = parseArgs(process.argv);
  await validateInputDir(input);
  console.log(`Input:  ${input}`);
  console.log(`Output: ${output}`);
  // Discovery + processing added in later tasks.
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
```

- [ ] **Step 2: Make it executable**

```bash
chmod +x index.js
```

- [ ] **Step 3: Manually verify arg parsing**

Run each and confirm:

```bash
node index.js
# Expected: "Usage: imageopt <input-dir> -o <output-dir>" and exit 1

node index.js ./nonexistent -o ./dist
# Expected: "Input directory does not exist: /abs/path/nonexistent" and exit 1

mkdir -p fixtures/sample
node index.js ./fixtures/sample -o ./dist
# Expected: prints absolute Input/Output paths and exits 0
```

- [ ] **Step 4: Commit**

```bash
git add index.js
git commit -m "feat: add CLI entry with arg parsing and input validation"
```

---

## Task 3: Recursively discover image files

**Files:**
- Modify: `index.js` (add `discoverImages` function; call from `main`)

- [ ] **Step 1: Add the discovery function**

Add after the `validateInputDir` function:

```js
import { readdir } from 'node:fs/promises';
import { join, extname, relative } from 'node:path';

async function discoverImages(rootDir) {
  const results = [];
  async function walk(dir) {
    const entries = await readdir(dir, { withFileTypes: true });
    for (const entry of entries) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        await walk(full);
      } else if (entry.isFile()) {
        const ext = extname(entry.name).toLowerCase();
        if (CONFIG.EXTENSIONS.includes(ext)) {
          results.push({
            absPath: full,
            relPath: relative(rootDir, full),
            ext,
          });
        }
      }
    }
  }
  await walk(rootDir);
  return results;
}
```

Move the two new imports (`readdir`, and the additional `path` imports) up next to the existing ones at the top of the file — do not leave `import` statements mid-file.

After editing, the imports block at the top should be:

```js
import { stat, readdir } from 'node:fs/promises';
import { resolve, join, extname, relative } from 'node:path';
```

- [ ] **Step 2: Wire discovery into `main`**

Replace the tail of `main` (the placeholder comment) with:

```js
  const files = await discoverImages(input);
  console.log(`Found ${files.length} images`);
  for (const f of files) console.log(`  ${f.relPath}`);
```

- [ ] **Step 3: Manually verify**

Drop a few `.jpg` / `.png` / `.webp` files into `fixtures/sample/` (including one in a subfolder like `fixtures/sample/nested/foo.jpg`). Then:

```bash
node index.js ./fixtures/sample -o ./dist
```

Expected: prints `Found N images` then one indented relative path per image, including the nested one. Non-image files are ignored.

- [ ] **Step 4: Commit**

```bash
git add index.js
git commit -m "feat: recursively discover supported image files"
```

---

## Task 4: Implement mtime-based skip decision

**Files:**
- Modify: `index.js` (add `needsRebuild` helper)

- [ ] **Step 1: Add the helper**

Insert above `main`:

```js
async function needsRebuild(srcPath, outPath) {
  let srcStat;
  try {
    srcStat = await stat(srcPath);
  } catch {
    return false; // source missing — upstream caller handles
  }
  let outStat;
  try {
    outStat = await stat(outPath);
  } catch {
    return true; // output missing — must build
  }
  return srcStat.mtimeMs > outStat.mtimeMs;
}
```

Rationale: `>` (not `≥`) means when an output's mtime equals the source's, we skip. That matches the spec's "skip if output is new enough" intent and avoids pointless re-encodes right after a build.

- [ ] **Step 2: No manual test yet** — this is exercised by Task 5's processing loop.

- [ ] **Step 3: Commit**

```bash
git add index.js
git commit -m "feat: add mtime-based rebuild decision helper"
```

---

## Task 5: Process a single file (compress + emit WebP)

**Files:**
- Modify: `index.js` (add `processFile`; replace the discovery-logging block in `main`)

- [ ] **Step 1: Add import and processor**

Add to the top imports:

```js
import { mkdir } from 'node:fs/promises';
import { dirname } from 'node:path';
import sharp from 'sharp';
```

Combine with existing imports so `fs/promises` and `path` are each imported once. Final top-of-file imports:

```js
import { stat, readdir, mkdir } from 'node:fs/promises';
import { resolve, join, extname, relative, dirname } from 'node:path';
import sharp from 'sharp';
```

Add this function above `main`:

```js
async function encodeForExt(pipeline, ext) {
  switch (ext) {
    case '.jpg':
    case '.jpeg':
      return pipeline.jpeg({ quality: CONFIG.JPEG_QUALITY, mozjpeg: true });
    case '.png':
      // Sharp uses quality only when palette:true (pngquant-like quantization).
      return pipeline.png({ quality: CONFIG.PNG_QUALITY, palette: true, compressionLevel: 9 });
    case '.webp':
      return pipeline.webp({ quality: CONFIG.WEBP_QUALITY });
    default:
      throw new Error(`Unsupported extension: ${ext}`);
  }
}

async function writeIfNeeded(srcPath, outPath, encodeFn) {
  if (!(await needsRebuild(srcPath, outPath))) {
    return { wrote: false, srcBytes: 0, outBytes: 0 };
  }
  await mkdir(dirname(outPath), { recursive: true });
  const pipeline = encodeFn(sharp(srcPath));
  const { size: outBytes } = await pipeline.toFile(outPath);
  const { size: srcBytes } = await stat(srcPath);
  return { wrote: true, srcBytes, outBytes };
}

async function processFile(file, outputRoot) {
  const outSameFormat = join(outputRoot, file.relPath);
  const outWebp = join(
    outputRoot,
    file.relPath.replace(new RegExp(`${file.ext.replace('.', '\\.')}$`, 'i'), '.webp'),
  );

  const results = [];
  // Same-format output (skip if source is already .webp — dedup with WebP output)
  if (file.ext !== '.webp') {
    results.push(
      await writeIfNeeded(file.absPath, outSameFormat, (p) => encodeForExt(p, file.ext)),
    );
  }
  // WebP output (always)
  results.push(
    await writeIfNeeded(file.absPath, outWebp, (p) => encodeForExt(p, '.webp')),
  );
  return results;
}
```

- [ ] **Step 2: Replace the discovery-logging block in `main`**

Find:

```js
  const files = await discoverImages(input);
  console.log(`Found ${files.length} images`);
  for (const f of files) console.log(`  ${f.relPath}`);
```

Replace with:

```js
  const files = await discoverImages(input);
  console.log(`Found ${files.length} images`);
  let processed = 0, skipped = 0, failed = 0, srcTotal = 0, outTotal = 0;
  for (const file of files) {
    try {
      const results = await processFile(file, output);
      for (const r of results) {
        if (r.wrote) { processed++; srcTotal += r.srcBytes; outTotal += r.outBytes; }
        else skipped++;
      }
    } catch (err) {
      failed++;
      console.warn(`WARN: failed ${file.relPath}: ${err.message}`);
    }
  }
  console.log(`Processed: ${processed}   Skipped: ${skipped}   Failed: ${failed}`);
  if (srcTotal > 0) {
    const savedKB = ((srcTotal - outTotal) / 1024).toFixed(1);
    console.log(`Saved: ${savedKB} KB`);
  }
```

- [ ] **Step 3: Manually verify first run**

With at least one `.jpg` and one `.png` in `fixtures/sample/` (including a nested subfolder):

```bash
rm -rf dist
node index.js ./fixtures/sample -o ./dist
```

Expected:
- `dist/` mirrors `fixtures/sample/` structure.
- Each non-webp source yields two outputs: same extension + `.webp`.
- Each `.webp` source yields one output: same path.
- Summary shows `Processed: N   Skipped: 0   Failed: 0` with a positive `Saved: X KB`.

- [ ] **Step 4: Manually verify second run skips**

```bash
node index.js ./fixtures/sample -o ./dist
```

Expected: `Processed: 0   Skipped: N   Failed: 0`. No `Saved:` line (srcTotal is 0).

- [ ] **Step 5: Manually verify mtime-triggered rebuild**

```bash
touch fixtures/sample/<one-image>.jpg
node index.js ./fixtures/sample -o ./dist
```

Expected: the two outputs for that one image appear in `Processed`, the rest in `Skipped`.

- [ ] **Step 6: Commit**

```bash
git add index.js
git commit -m "feat: compress originals and emit WebP with mtime skip"
```

---

## Task 6: Add bounded concurrency

**Files:**
- Modify: `index.js` (replace the sequential file loop with a worker pool)

- [ ] **Step 1: Add a small worker-pool helper**

Insert above `main`:

```js
async function runPool(items, limit, worker) {
  const results = new Array(items.length);
  let next = 0;
  const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (true) {
      const i = next++;
      if (i >= items.length) return;
      results[i] = await worker(items[i], i);
    }
  });
  await Promise.all(workers);
  return results;
}
```

- [ ] **Step 2: Use the pool in `main`**

Replace the `for (const file of files) { ... }` block with:

```js
  const perFile = await runPool(files, CONFIG.CONCURRENCY, async (file) => {
    try {
      const results = await processFile(file, output);
      return { ok: true, results };
    } catch (err) {
      console.warn(`WARN: failed ${file.relPath}: ${err.message}`);
      return { ok: false };
    }
  });
  for (const item of perFile) {
    if (!item.ok) { failed++; continue; }
    for (const r of item.results) {
      if (r.wrote) { processed++; srcTotal += r.srcBytes; outTotal += r.outBytes; }
      else skipped++;
    }
  }
```

- [ ] **Step 3: Manually verify behavior unchanged**

```bash
rm -rf dist
node index.js ./fixtures/sample -o ./dist
node index.js ./fixtures/sample -o ./dist
```

Expected: first run processes all; second run skips all. Result totals should match Task 5 exactly.

- [ ] **Step 4: Commit**

```bash
git add index.js
git commit -m "feat: run per-file processing with bounded concurrency"
```

---

## Task 7: Add elapsed-time summary

**Files:**
- Modify: `index.js` (wrap processing in a timer)

- [ ] **Step 1: Record start and print duration**

Right before `const perFile = await runPool(...)` add:

```js
  const startedAt = Date.now();
```

Right before the final `Processed:` log line add:

```js
  const seconds = ((Date.now() - startedAt) / 1000).toFixed(2);
```

And update the summary to:

```js
  console.log(`Processed: ${processed}   Skipped: ${skipped}   Failed: ${failed}`);
  console.log(`Time: ${seconds}s${srcTotal > 0 ? `     Saved: ${((srcTotal - outTotal) / 1024).toFixed(1)} KB` : ''}`);
```

Remove the older `if (srcTotal > 0)` block that was added in Task 5 — it's folded into the line above.

- [ ] **Step 2: Manually verify**

```bash
rm -rf dist
node index.js ./fixtures/sample -o ./dist
```

Expected: final summary matches the spec format:

```
Processed: 42   Skipped: 0   Failed: 0
Time: 1.23s     Saved: 987.6 KB
```

- [ ] **Step 3: Commit**

```bash
git add index.js
git commit -m "feat: report elapsed time and saved bytes"
```

---

## Task 8: Write README

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write the README**

```markdown
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
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add README with install and usage"
```

---

## Task 9: Final end-to-end smoke test

**Files:** none — verification only.

- [ ] **Step 1: Clean and run cold**

```bash
rm -rf dist
node index.js ./fixtures/sample -o ./dist
```

Expected: non-zero `Processed`, zero `Skipped`, zero `Failed`, positive `Saved`.

- [ ] **Step 2: Run warm**

```bash
node index.js ./fixtures/sample -o ./dist
```

Expected: `Processed: 0`, all skipped.

- [ ] **Step 3: Verify `npm link` path**

```bash
npm link
which imageopt
imageopt ./fixtures/sample -o ./dist
```

Expected: `which imageopt` prints a path inside the global npm prefix; the command runs and reports all-skipped.

- [ ] **Step 4: Verify unknown flag is rejected**

```bash
imageopt ./fixtures/sample -o ./dist --weird
```

Expected: error about unknown argument + usage + exit 1.

No commit — this task is verification.
