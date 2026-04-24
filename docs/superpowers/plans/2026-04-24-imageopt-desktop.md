# ImageOpt Desktop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the `imageopt` CLI into a Tauri desktop app with ImageOptim-like UX — drag-drop, in-place compress, originals to Trash, Undo, per-format quality sliders, optional WebP/AVIF companions.

**Architecture:** Three-tier process model. Tauri WebView (vanilla TS) calls Rust commands via `invoke`; Rust spawns a bundled Node sidecar that runs existing Sharp logic; sidecar and Rust communicate over stdin/stdout using JSON lines. Originals go to system Trash via the Rust `trash` crate. Per-file events stream back through Tauri's event bus.

**Tech Stack:** Node 18+ (builtin `node:test`), Sharp 0.34, Tauri 2.x, Rust (tokio, serde_json, trash, uuid), TypeScript + Vite for the webview.

---

## Task Index

**Part A: CLI refactor (shared modules)**
- Task 1: Extract `CONFIG` to `src/config.js`
- Task 2: Extract `encode()` and `ENCODERS` map to `src/encoder.js`
- Task 3: Refactor `index.js` to use the extracted modules

**Part B: Node sidecar**
- Task 4: Sidecar JSON-lines loop — happy path encode
- Task 5: Sidecar error paths — malformed JSON, encode errors, no-gain skip
- Task 6: Sidecar concurrency + companion (WebP) outputs

**Part C: Tauri shell + settings**
- Task 7: Scaffold Tauri 2.x project at `desktop/`
- Task 8: Base UI shell — dropzone, file list, toolbar skeleton
- Task 9: Settings panel UI — sliders + toggles with in-memory state
- Task 10: Persist settings via `tauri-plugin-store`

**Part D: Compress flow end-to-end**
- Task 11: Rust `sidecar.rs` — spawn Node, JSON-lines reader/writer
- Task 12: Rust `compress` command — dispatch to sidecar, emit events
- Task 13: Frontend dropzone → `invoke('compress')` wire-up
- Task 14: Frontend file-list — rows react to `file-done` / `file-error` / `file-skipped`
- Task 15: Rust Trash + atomic rename per file

**Part E: Undo + edge cases + polish**
- Task 16: Rust `batch.rs` + `undo_last_batch` command
- Task 17: Frontend Undo button + disabled state
- Task 18: Folder expansion + unsupported-file filtering
- Task 19: Status bar — cumulative saved + Show Trash
- Task 20: Close-requested confirm dialog + sidecar crash recovery

**Part F: Packaging**
- Task 21: Node portable binary bundling + sidecar registration in `tauri.conf.json`
- Task 22: Release build + manual smoke test

---

## Prerequisites

Before starting, the engineer should have:

- Node 18+ (`node --version` returns v18 or newer)
- Rust toolchain (`rustc --version`, `cargo --version`). Install from https://rustup.rs if missing.
- Tauri 2.x prerequisites per platform:
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`)
  - **Windows:** Microsoft C++ Build Tools + WebView2 (auto-installed on Win 11)
  - **Linux:** `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
- Existing `imageopt` CLI repo cloned and `npm install` already run (confirmed: `node_modules/sharp` exists).
- A few test images in `daxi/` (user already has these) and `fixtures/sample/` (already exists — red.jpg, blue.png, green.webp, nested/yellow.JPEG).

---

## Part A: CLI refactor (shared modules)

The existing `index.js` is a single file containing config, encoder logic, concurrency pool, and CLI entry. We split the reusable pieces into `src/` so the sidecar can import them without dragging the CLI's argv parsing along.

### Task 1: Extract `CONFIG` to `src/config.js`

**Files:**
- Create: `src/config.js`
- Create: `src/config.test.js`
- Modify: `index.js:6-12` (replace CONFIG block with import)

- [ ] **Step 1: Write the failing test**

Create `src/config.test.js`:
```js
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CONFIG } from './config.js';

test('CONFIG exposes quality defaults and extension list', () => {
  assert.equal(CONFIG.JPEG_QUALITY, 75);
  assert.equal(CONFIG.PNG_QUALITY, 75);
  assert.equal(CONFIG.WEBP_QUALITY, 75);
  assert.equal(CONFIG.CONCURRENCY, 4);
  assert.deepEqual(CONFIG.EXTENSIONS, ['.jpg', '.jpeg', '.png', '.webp']);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test src/config.test.js`
Expected: FAIL with "Cannot find module './config.js'".

- [ ] **Step 3: Create `src/config.js`**

```js
export const CONFIG = {
  JPEG_QUALITY: 75,
  PNG_QUALITY: 75,
  WEBP_QUALITY: 75,
  EXTENSIONS: ['.jpg', '.jpeg', '.png', '.webp'],
  CONCURRENCY: 4,
};
```

- [ ] **Step 4: Replace the CONFIG block in `index.js`**

Replace lines 6–12 (the whole `const CONFIG = { ... };` block) with:
```js
import { CONFIG } from './src/config.js';
```

Place the import alongside the other imports at the top of the file.

- [ ] **Step 5: Run test + smoke test CLI**

```bash
node --test src/config.test.js
imageopt ./fixtures/sample -o ./dist/sample
```
Expected: test passes; CLI prints "Processed: N   Skipped: 0   Failed: 0" and produces files in `./dist/sample/`.

- [ ] **Step 6: Commit**

```bash
git add src/config.js src/config.test.js index.js
git commit -m "refactor: extract CONFIG to src/config.js"
```

---

### Task 2: Extract `encode()` and `ENCODERS` map to `src/encoder.js`

**Files:**
- Create: `src/encoder.js`
- Create: `src/encoder.test.js`
- Create: `fixtures/test/red.jpg`, `fixtures/test/blue.png`, `fixtures/test/green.webp` (generated in the test, not committed)

- [ ] **Step 1: Write the failing test**

Create `src/encoder.test.js`:
```js
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, rm, stat } from 'node:fs/promises';
import { join } from 'node:path';
import sharp from 'sharp';
import { ENCODERS, encode } from './encoder.js';

const TMP = './fixtures/test-encoder';

before(async () => {
  await mkdir(TMP, { recursive: true });
  await sharp({
    create: { width: 200, height: 200, channels: 3, background: { r: 255, g: 0, b: 0 } },
  }).jpeg().toFile(join(TMP, 'red.jpg'));
  await sharp({
    create: { width: 200, height: 200, channels: 3, background: { r: 0, g: 0, b: 255 } },
  }).png().toFile(join(TMP, 'blue.png'));
  await sharp({
    create: { width: 200, height: 200, channels: 3, background: { r: 0, g: 255, b: 0 } },
  }).webp().toFile(join(TMP, 'green.webp'));
});

after(async () => {
  await rm(TMP, { recursive: true, force: true });
});

test('ENCODERS has entries for jpg/jpeg/png/webp', () => {
  assert.ok(ENCODERS['.jpg']);
  assert.ok(ENCODERS['.jpeg']);
  assert.ok(ENCODERS['.png']);
  assert.ok(ENCODERS['.webp']);
});

test('encode produces a valid JPEG', async () => {
  const dst = join(TMP, 'red.out.jpg');
  const { outBytes } = await encode({
    srcPath: join(TMP, 'red.jpg'),
    dstPath: dst,
    ext: '.jpg',
    opts: { jpegQuality: 75 },
  });
  assert.ok(outBytes > 0, 'outBytes should be positive');
  const meta = await sharp(dst).metadata();
  assert.equal(meta.format, 'jpeg');
});

test('encode produces a valid palette PNG', async () => {
  const dst = join(TMP, 'blue.out.png');
  await encode({
    srcPath: join(TMP, 'blue.png'),
    dstPath: dst,
    ext: '.png',
    opts: { pngQuality: 75 },
  });
  const meta = await sharp(dst).metadata();
  assert.equal(meta.format, 'png');
});

test('encode produces a valid WebP', async () => {
  const dst = join(TMP, 'green.out.webp');
  await encode({
    srcPath: join(TMP, 'green.webp'),
    dstPath: dst,
    ext: '.webp',
    opts: { webpQuality: 75 },
  });
  const meta = await sharp(dst).metadata();
  assert.equal(meta.format, 'webp');
});

test('encode throws on unsupported extension', async () => {
  await assert.rejects(
    encode({ srcPath: join(TMP, 'red.jpg'), dstPath: join(TMP, 'x.tiff'), ext: '.tiff', opts: {} }),
    /Unsupported extension: \.tiff/,
  );
});

test('encode lowercases extension before lookup', async () => {
  const dst = join(TMP, 'red.upper.jpg');
  const { outBytes } = await encode({
    srcPath: join(TMP, 'red.jpg'),
    dstPath: dst,
    ext: '.JPG',
    opts: { jpegQuality: 75 },
  });
  assert.ok(outBytes > 0);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test src/encoder.test.js`
Expected: FAIL with "Cannot find module './encoder.js'".

- [ ] **Step 3: Create `src/encoder.js`**

```js
import sharp from 'sharp';

export const ENCODERS = {
  '.jpg':  (pipeline, opts) => pipeline.jpeg({ quality: opts.jpegQuality, mozjpeg: true }),
  '.jpeg': (pipeline, opts) => pipeline.jpeg({ quality: opts.jpegQuality, mozjpeg: true }),
  '.png':  (pipeline, opts) => pipeline.png({ quality: opts.pngQuality, palette: true, compressionLevel: 9 }),
  '.webp': (pipeline, opts) => pipeline.webp({ quality: opts.webpQuality }),
};

export async function encode({ srcPath, dstPath, ext, opts }) {
  const key = ext.toLowerCase();
  const encoder = ENCODERS[key];
  if (!encoder) throw new Error(`Unsupported extension: ${ext}`);
  const pipeline = encoder(sharp(srcPath), opts);
  const { size: outBytes } = await pipeline.toFile(dstPath);
  return { outBytes };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test src/encoder.test.js`
Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/encoder.js src/encoder.test.js
git commit -m "feat: add shared encoder module with ENCODERS registry"
```

---

### Task 3: Refactor `index.js` to use the extracted modules

**Files:**
- Modify: `index.js` (replace `encodeForExt` usage with `encode()` from the new module)

- [ ] **Step 1: Snapshot golden output before refactor**

```bash
rm -rf fixtures/golden
imageopt ./fixtures/sample -o ./fixtures/golden
```
Expected: produces a tree under `fixtures/golden/` matching the structure of `fixtures/sample/`.

- [ ] **Step 2: Replace `encodeForExt` + `writeIfNeeded` internals in `index.js`**

In `index.js`, replace the `encodeForExt` function (lines 91–103) with an import at the top:

```js
import { encode } from './src/encoder.js';
```

Replace `writeIfNeeded` (lines 105–114) with:

```js
async function writeIfNeeded(srcPath, outPath, ext, opts) {
  if (!(await needsRebuild(srcPath, outPath))) {
    return { wrote: false, srcBytes: 0, outBytes: 0 };
  }
  await mkdir(dirname(outPath), { recursive: true });
  const { outBytes } = await encode({ srcPath, dstPath: outPath, ext, opts });
  const { size: srcBytes } = await stat(srcPath);
  return { wrote: true, srcBytes, outBytes };
}
```

Replace `processFile` (lines 116–133) with:

```js
async function processFile(file, outputRoot) {
  const outSameFormat = join(outputRoot, file.relPath);
  const outWebp = join(
    outputRoot,
    file.relPath.replace(new RegExp(`${file.ext.replace('.', '\\.')}$`, 'i'), '.webp'),
  );
  const opts = {
    jpegQuality: CONFIG.JPEG_QUALITY,
    pngQuality: CONFIG.PNG_QUALITY,
    webpQuality: CONFIG.WEBP_QUALITY,
  };
  const results = [];
  if (file.ext !== '.webp') {
    results.push(await writeIfNeeded(file.absPath, outSameFormat, file.ext, opts));
  }
  results.push(await writeIfNeeded(file.absPath, outWebp, '.webp', opts));
  return results;
}
```

Delete the `encodeForExt` function from `index.js`.

- [ ] **Step 3: Run CLI against fixtures**

```bash
rm -rf dist/sample
imageopt ./fixtures/sample -o ./dist/sample
```
Expected: "Processed: 5   Skipped: 0   Failed: 0" (or equivalent count for your fixture tree).

- [ ] **Step 4: Compare against golden**

```bash
for f in $(find fixtures/golden -type f); do
  rel=${f#fixtures/golden/}
  newfile="dist/sample/$rel"
  goldsize=$(stat -f %z "$f" 2>/dev/null || stat -c %s "$f")
  newsize=$(stat -f %z "$newfile" 2>/dev/null || stat -c %s "$newfile")
  diff=$(( goldsize > newsize ? goldsize - newsize : newsize - goldsize ))
  tol=$(( goldsize / 50 ))   # 2% tolerance
  if [ "$diff" -gt "$tol" ]; then
    echo "MISMATCH: $rel ($goldsize vs $newsize, diff $diff > tol $tol)"
  fi
done
```
Expected: no MISMATCH lines printed (output is byte-identical or within 2%).

- [ ] **Step 5: Run all JS tests**

Run: `node --test src/*.test.js`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add index.js
git commit -m "refactor: route index.js through src/encoder.js"
```

---

## Part B: Node sidecar

The sidecar is a long-running Node process. It reads JSON commands from stdin (one per line), processes them, and writes JSON events to stdout (one per line). This is what the Tauri Rust code will spawn and talk to.

### Task 4: Sidecar JSON-lines loop — happy path encode

**Files:**
- Create: `src/sidecar.js`
- Create: `src/sidecar.test.js`

- [ ] **Step 1: Write the failing test**

Create `src/sidecar.test.js`:
```js
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, rm, stat, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import sharp from 'sharp';

const TMP = './fixtures/test-sidecar';
const SIDECAR = resolve('./src/sidecar.js');

before(async () => {
  await mkdir(TMP, { recursive: true });
  await sharp({
    create: { width: 400, height: 400, channels: 3, background: { r: 255, g: 100, b: 50 } },
  }).jpeg({ quality: 100 }).toFile(join(TMP, 'big.jpg'));
});

after(async () => {
  await rm(TMP, { recursive: true, force: true });
});

function runSidecar(commands, { timeoutMs = 5000 } = {}) {
  return new Promise((resolveP, rejectP) => {
    const proc = spawn('node', [SIDECAR], { stdio: ['pipe', 'pipe', 'pipe'] });
    let out = '';
    let err = '';
    proc.stdout.on('data', (d) => (out += d.toString()));
    proc.stderr.on('data', (d) => (err += d.toString()));
    const timer = setTimeout(() => {
      proc.kill('SIGKILL');
      rejectP(new Error(`sidecar timed out after ${timeoutMs}ms. stderr: ${err}`));
    }, timeoutMs);
    proc.on('close', (code) => {
      clearTimeout(timer);
      resolveP({ code, out, err });
    });
    for (const cmd of commands) proc.stdin.write(JSON.stringify(cmd) + '\n');
    proc.stdin.end();
  });
}

test('sidecar handles a single encode command and emits done', async () => {
  const id = 'test-1';
  const src = resolve(join(TMP, 'big.jpg'));
  const { out, code } = await runSidecar([
    { cmd: 'encode', id, src, ext: '.jpg', opts: { jpegQuality: 75 } },
  ]);
  assert.equal(code, 0);
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  const done = lines.find((l) => l.type === 'done' && l.id === id);
  assert.ok(done, `expected a done event, got: ${out}`);
  assert.ok(done.tmp && done.tmp.endsWith('.jpg'));
  assert.ok(done.srcBytes > 0);
  assert.ok(done.outBytes > 0);
  assert.ok(done.outBytes < done.srcBytes, 'compressed file should be smaller');
  assert.deepEqual(done.companions, []);
  await stat(done.tmp);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test src/sidecar.test.js`
Expected: FAIL with "Cannot find module" or timeout (no file yet).

- [ ] **Step 3: Create minimal sidecar**

Create `src/sidecar.js`:
```js
#!/usr/bin/env node
import { createInterface } from 'node:readline';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { stat } from 'node:fs/promises';
import { encode } from './encoder.js';

const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });

function emit(event) {
  process.stdout.write(JSON.stringify(event) + '\n');
}

async function handleEncode({ id, src, ext, opts }) {
  const tmp = join(tmpdir(), `imageopt-${randomUUID()}${ext}`);
  const { outBytes } = await encode({ srcPath: src, dstPath: tmp, ext, opts });
  const { size: srcBytes } = await stat(src);
  emit({ type: 'done', id, tmp, srcBytes, outBytes, companions: [] });
}

for await (const line of rl) {
  if (!line.trim()) continue;
  const msg = JSON.parse(line);
  if (msg.cmd === 'encode') await handleEncode(msg);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test src/sidecar.test.js`
Expected: test passes.

- [ ] **Step 5: Commit**

```bash
git add src/sidecar.js src/sidecar.test.js
git commit -m "feat: sidecar JSON-lines loop with encode handler"
```

---

### Task 5: Sidecar error paths — malformed JSON, encode errors, no-gain skip

**Files:**
- Modify: `src/sidecar.js`
- Modify: `src/sidecar.test.js`

- [ ] **Step 1: Write failing tests**

Append to `src/sidecar.test.js`:
```js
test('malformed JSON on stdin does not crash sidecar', async () => {
  const src = resolve(join(TMP, 'big.jpg'));
  const proc = spawn('node', [SIDECAR], { stdio: ['pipe', 'pipe', 'pipe'] });
  let out = '';
  proc.stdout.on('data', (d) => (out += d.toString()));
  proc.stdin.write('this-is-not-json\n');
  proc.stdin.write(JSON.stringify({ cmd: 'encode', id: 'after-bad', src, ext: '.jpg', opts: { jpegQuality: 75 } }) + '\n');
  proc.stdin.end();
  await new Promise((r) => proc.on('close', r));
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  assert.ok(lines.some((l) => l.type === 'parse-error'), 'expected parse-error event');
  assert.ok(lines.some((l) => l.type === 'done' && l.id === 'after-bad'), 'sidecar should continue after bad line');
});

test('encode on missing file emits error event', async () => {
  const { out } = await runSidecar([
    { cmd: 'encode', id: 'missing', src: '/no/such/file.jpg', ext: '.jpg', opts: { jpegQuality: 75 } },
  ]);
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  const err = lines.find((l) => l.type === 'error' && l.id === 'missing');
  assert.ok(err, `expected error event, got: ${out}`);
  assert.ok(err.msg && err.msg.length > 0);
});

test('no-gain skip: already-compressed tiny file', async () => {
  // write a tiny JPEG at q=60 so re-encoding at q=75 won't save anything
  const tiny = join(TMP, 'tiny.jpg');
  await sharp({
    create: { width: 64, height: 64, channels: 3, background: { r: 10, g: 20, b: 30 } },
  }).jpeg({ quality: 40, mozjpeg: true }).toFile(tiny);
  const { out } = await runSidecar([
    { cmd: 'encode', id: 'tiny', src: resolve(tiny), ext: '.jpg', opts: { jpegQuality: 95 } },
  ]);
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  const skip = lines.find((l) => l.type === 'skipped-no-gain' && l.id === 'tiny');
  assert.ok(skip, `expected skipped-no-gain, got: ${out}`);
  assert.ok(skip.srcBytes > 0);
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `node --test src/sidecar.test.js`
Expected: 3 new failures (parse-error not emitted; error swallowed; no-gain not detected).

- [ ] **Step 3: Extend sidecar error handling**

Replace the main loop in `src/sidecar.js` with:
```js
import { unlink } from 'node:fs/promises';

async function handleEncode({ id, src, ext, opts }) {
  let tmp;
  try {
    tmp = join(tmpdir(), `imageopt-${randomUUID()}${ext}`);
    const { outBytes } = await encode({ srcPath: src, dstPath: tmp, ext, opts });
    const { size: srcBytes } = await stat(src);
    if (outBytes >= srcBytes) {
      await unlink(tmp).catch(() => {});
      emit({ type: 'skipped-no-gain', id, srcBytes });
      return;
    }
    emit({ type: 'done', id, tmp, srcBytes, outBytes, companions: [] });
  } catch (err) {
    if (tmp) await unlink(tmp).catch(() => {});
    emit({ type: 'error', id, msg: err.message });
  }
}

for await (const line of rl) {
  if (!line.trim()) continue;
  let msg;
  try {
    msg = JSON.parse(line);
  } catch (err) {
    emit({ type: 'parse-error', msg: err.message, line });
    continue;
  }
  try {
    if (msg.cmd === 'encode') await handleEncode(msg);
    else emit({ type: 'error', id: msg.id ?? null, msg: `Unknown cmd: ${msg.cmd}` });
  } catch (err) {
    emit({ type: 'error', id: msg.id ?? null, msg: err.message });
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `node --test src/sidecar.test.js`
Expected: all 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/sidecar.js src/sidecar.test.js
git commit -m "feat: sidecar error paths and no-gain skip"
```

---

### Task 6: Sidecar concurrency + companion (WebP) outputs

**Files:**
- Modify: `src/sidecar.js`
- Modify: `src/sidecar.test.js`

- [ ] **Step 1: Write failing tests**

Append to `src/sidecar.test.js`:
```js
test('sidecar processes multiple commands in order and emits per-id done', async () => {
  const src = resolve(join(TMP, 'big.jpg'));
  const { out } = await runSidecar([
    { cmd: 'encode', id: 'a', src, ext: '.jpg', opts: { jpegQuality: 75 } },
    { cmd: 'encode', id: 'b', src, ext: '.jpg', opts: { jpegQuality: 60 } },
    { cmd: 'encode', id: 'c', src, ext: '.jpg', opts: { jpegQuality: 40 } },
  ]);
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  const ids = lines.filter((l) => l.type === 'done').map((l) => l.id).sort();
  assert.deepEqual(ids, ['a', 'b', 'c']);
});

test('emitWebp produces a companion entry in done event', async () => {
  const src = resolve(join(TMP, 'big.jpg'));
  const { out } = await runSidecar([
    { cmd: 'encode', id: 'x', src, ext: '.jpg', opts: { jpegQuality: 75, webpQuality: 75, emitWebp: true } },
  ]);
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  const done = lines.find((l) => l.type === 'done' && l.id === 'x');
  assert.ok(done);
  assert.equal(done.companions.length, 1);
  assert.equal(done.companions[0].ext, '.webp');
  assert.ok(done.companions[0].tmp && done.companions[0].tmp.endsWith('.webp'));
  assert.ok(done.companions[0].outBytes > 0);
});

test('companion encode failure emits companion-error but main still done', async () => {
  const src = resolve(join(TMP, 'big.jpg'));
  // Force companion failure by requesting an unsupported companion ext via a bad opts shape
  // simulate: set webpQuality to an invalid value that sharp rejects
  const { out } = await runSidecar([
    { cmd: 'encode', id: 'y', src, ext: '.jpg', opts: { jpegQuality: 75, webpQuality: 9999, emitWebp: true } },
  ]);
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  const done = lines.find((l) => l.type === 'done' && l.id === 'y');
  const warn = lines.find((l) => l.type === 'companion-error' && l.id === 'y');
  assert.ok(done, 'main encode should still succeed');
  assert.ok(warn, 'companion failure should be surfaced');
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `node --test src/sidecar.test.js`
Expected: 3 failures (serial-only, no companions, no companion-error).

- [ ] **Step 3: Add concurrency pool + companion emission**

Update `src/sidecar.js`. Add after the imports:
```js
import { CONFIG } from './config.js';

const CONCURRENCY = CONFIG.CONCURRENCY;
let inFlight = 0;
const queue = [];

function schedule(fn) {
  return new Promise((resolve) => {
    const job = async () => {
      inFlight++;
      try { await fn(); } finally {
        inFlight--;
        const next = queue.shift();
        if (next) next();
      }
      resolve();
    };
    if (inFlight < CONCURRENCY) job();
    else queue.push(job);
  });
}
```

Replace `handleEncode` with:
```js
async function handleEncode({ id, src, ext, opts }) {
  let mainTmp;
  try {
    mainTmp = join(tmpdir(), `imageopt-${randomUUID()}${ext}`);
    const { outBytes } = await encode({ srcPath: src, dstPath: mainTmp, ext, opts });
    const { size: srcBytes } = await stat(src);
    if (outBytes >= srcBytes) {
      await unlink(mainTmp).catch(() => {});
      emit({ type: 'skipped-no-gain', id, srcBytes });
      return;
    }
    const companions = [];
    if (opts.emitWebp && ext !== '.webp') {
      const compTmp = join(tmpdir(), `imageopt-${randomUUID()}.webp`);
      try {
        const { outBytes: compBytes } = await encode({
          srcPath: src, dstPath: compTmp, ext: '.webp', opts,
        });
        companions.push({ ext: '.webp', tmp: compTmp, outBytes: compBytes });
      } catch (err) {
        emit({ type: 'companion-error', id, ext: '.webp', msg: err.message });
      }
    }
    emit({ type: 'done', id, tmp: mainTmp, srcBytes, outBytes, companions });
  } catch (err) {
    if (mainTmp) await unlink(mainTmp).catch(() => {});
    emit({ type: 'error', id, msg: err.message });
  }
}
```

Change the main loop from `await handleEncode(msg)` to `schedule(() => handleEncode(msg))` (don't await — let the pool manage it):

```js
for await (const line of rl) {
  if (!line.trim()) continue;
  let msg;
  try {
    msg = JSON.parse(line);
  } catch (err) {
    emit({ type: 'parse-error', msg: err.message, line });
    continue;
  }
  if (msg.cmd === 'encode') schedule(() => handleEncode(msg));
  else emit({ type: 'error', id: msg.id ?? null, msg: `Unknown cmd: ${msg.cmd}` });
}

// Wait for any remaining in-flight jobs before exiting.
while (inFlight > 0 || queue.length > 0) {
  await new Promise((r) => setTimeout(r, 20));
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `node --test src/sidecar.test.js`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/sidecar.js src/sidecar.test.js
git commit -m "feat: sidecar concurrency pool and WebP companion outputs"
```

---

## Part C: Tauri shell + settings

Create the Tauri project, a minimal UI shell, and a settings panel. No compression yet — just structure and persistence.

### Task 7: Scaffold Tauri 2.x project at `desktop/`

**Files:**
- Create: everything under `desktop/` (generated by the scaffold)
- Modify: `.gitignore` (exclude `desktop/node_modules`, `desktop/src-tauri/target`, `desktop/dist`)

- [ ] **Step 1: Run the Tauri scaffold**

```bash
cd /Users/csii/Desktop/git/ImageOpt
npm create tauri-app@latest -- desktop --template vanilla-ts --manager npm --identifier com.imageopt.app
```
Expected: creates a `desktop/` directory with a vanilla-TypeScript Tauri 2 project.

- [ ] **Step 2: Install frontend deps**

```bash
cd desktop
npm install
```
Expected: no errors; `desktop/node_modules` populated.

- [ ] **Step 3: Update `.gitignore`**

Append to `/Users/csii/Desktop/git/ImageOpt/.gitignore`:
```
desktop/node_modules/
desktop/src-tauri/target/
desktop/dist/
```

- [ ] **Step 4: Verify dev runs**

From `desktop/`:
```bash
npm run tauri dev
```
Expected: a Tauri window opens with the default "Welcome to Tauri" content. Close the window (Cmd-Q or Ctrl-Q).

- [ ] **Step 5: Set app name + window title**

Edit `desktop/src-tauri/tauri.conf.json`:
```json
{
  "productName": "ImageOpt",
  "app": {
    "windows": [
      {
        "title": "ImageOpt",
        "width": 720,
        "height": 520,
        "minWidth": 560,
        "minHeight": 400,
        "resizable": true
      }
    ]
  }
}
```
(Merge these into the existing file; don't replace other fields.)

- [ ] **Step 6: Commit**

```bash
cd /Users/csii/Desktop/git/ImageOpt
git add .gitignore desktop/
git commit -m "chore: scaffold Tauri 2 project in desktop/"
```

---

### Task 8: Base UI shell — dropzone, file list, toolbar skeleton

**Files:**
- Replace: `desktop/src/index.html`
- Replace: `desktop/src/main.ts`
- Replace: `desktop/src/style.css`
- Create: `desktop/src/ui/dropzone.ts`
- Create: `desktop/src/ui/toolbar.ts`
- Create: `desktop/src/ui/file-list.ts`
- Create: `desktop/src/state.ts`

- [ ] **Step 1: Write the state module**

Create `desktop/src/state.ts`:
```ts
export type FileStatus = 'pending' | 'working' | 'done' | 'error' | 'skipped-no-gain';

export interface FileRow {
  id: string;
  path: string;
  name: string;
  status: FileStatus;
  srcBytes?: number;
  outBytes?: number;
  errorMsg?: string;
}

type Listener = (rows: FileRow[]) => void;

class Store {
  private rows = new Map<string, FileRow>();
  private listeners = new Set<Listener>();

  snapshot(): FileRow[] { return Array.from(this.rows.values()); }
  subscribe(l: Listener): () => void { this.listeners.add(l); return () => this.listeners.delete(l); }
  upsert(row: FileRow) { this.rows.set(row.path, row); this.emit(); }
  update(path: string, patch: Partial<FileRow>) {
    const r = this.rows.get(path); if (!r) return;
    this.rows.set(path, { ...r, ...patch });
    this.emit();
  }
  findById(id: string): FileRow | undefined {
    for (const r of this.rows.values()) if (r.id === id) return r;
    return undefined;
  }
  clear() { this.rows.clear(); this.emit(); }
  private emit() { const snap = this.snapshot(); for (const l of this.listeners) l(snap); }
}

export const store = new Store();
```

- [ ] **Step 2: Write the dropzone UI**

Create `desktop/src/ui/dropzone.ts`:
```ts
import { store } from '../state';

export function mountDropzone(root: HTMLElement, onFiles: (paths: string[]) => void) {
  root.innerHTML = `<div class="dropzone" id="dropzone">Drag images here, or <span class="link">click to add</span></div>`;
  const dz = root.querySelector('#dropzone') as HTMLElement;

  dz.addEventListener('dragover', (e) => { e.preventDefault(); dz.classList.add('hover'); });
  dz.addEventListener('dragleave', () => dz.classList.remove('hover'));
  dz.addEventListener('drop', (e) => {
    e.preventDefault();
    dz.classList.remove('hover');
    const paths: string[] = [];
    for (const f of Array.from(e.dataTransfer?.files ?? [])) {
      // @ts-expect-error Tauri webview exposes .path on dropped files
      if (f.path) paths.push(f.path);
    }
    if (paths.length) onFiles(paths);
  });
}
```

- [ ] **Step 3: Write the file-list UI**

Create `desktop/src/ui/file-list.ts`:
```ts
import { store, FileRow } from '../state';

function fmtBytes(n?: number): string {
  if (n === undefined) return '';
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

function statusIcon(row: FileRow): string {
  switch (row.status) {
    case 'done': return '✓';
    case 'working': return '⏵';
    case 'error': return '⚠';
    case 'skipped-no-gain': return '=';
    default: return '○';
  }
}

function savings(row: FileRow): string {
  if (row.status !== 'done' || !row.srcBytes || !row.outBytes) return '';
  const pct = Math.round((1 - row.outBytes / row.srcBytes) * 100);
  return `−${pct}%`;
}

export function mountFileList(root: HTMLElement) {
  root.innerHTML = `<div class="file-list" id="file-list"></div>`;
  const list = root.querySelector('#file-list') as HTMLElement;

  function render(rows: FileRow[]) {
    if (rows.length === 0) {
      list.innerHTML = `<div class="empty">No files yet. Drag images onto the window.</div>`;
      return;
    }
    list.innerHTML = rows.map((r) => `
      <div class="row status-${r.status}" title="${r.path}">
        <span class="icon">${statusIcon(r)}</span>
        <span class="name">${r.name}</span>
        <span class="size old">${fmtBytes(r.srcBytes)}</span>
        <span class="size new">${fmtBytes(r.outBytes)}</span>
        <span class="savings">${savings(r)}</span>
      </div>
    `).join('');
  }

  store.subscribe(render);
  render(store.snapshot());
}
```

- [ ] **Step 4: Write the toolbar UI**

Create `desktop/src/ui/toolbar.ts`:
```ts
export function mountToolbar(root: HTMLElement, handlers: { onSettings: () => void; onUndo: () => void }) {
  root.innerHTML = `
    <div class="toolbar">
      <div class="title">ImageOpt</div>
      <div class="actions">
        <button id="btn-settings" class="btn">⚙ Settings</button>
        <button id="btn-undo" class="btn" disabled>↺ Undo</button>
      </div>
    </div>`;
  (root.querySelector('#btn-settings') as HTMLButtonElement).onclick = handlers.onSettings;
  (root.querySelector('#btn-undo') as HTMLButtonElement).onclick = handlers.onUndo;
}
```

- [ ] **Step 5: Replace `index.html`**

Replace `desktop/src/index.html` with:
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>ImageOpt</title>
    <link rel="stylesheet" href="style.css" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  </head>
  <body>
    <div id="app">
      <div id="toolbar"></div>
      <div id="dropzone"></div>
      <div id="list"></div>
      <div id="statusbar"></div>
    </div>
    <script type="module" src="main.ts"></script>
  </body>
</html>
```

- [ ] **Step 6: Replace `main.ts`**

Replace `desktop/src/main.ts` with:
```ts
import { mountToolbar } from './ui/toolbar';
import { mountDropzone } from './ui/dropzone';
import { mountFileList } from './ui/file-list';
import { store } from './state';
import { basename } from './util/path';

mountToolbar(document.getElementById('toolbar')!, {
  onSettings: () => console.log('settings clicked'),
  onUndo: () => console.log('undo clicked'),
});

mountDropzone(document.getElementById('dropzone')!, (paths) => {
  for (const p of paths) {
    store.upsert({
      id: crypto.randomUUID(),
      path: p,
      name: basename(p),
      status: 'pending',
    });
  }
});

mountFileList(document.getElementById('list')!);
```

Create `desktop/src/util/path.ts`:
```ts
export function basename(p: string): string {
  const idx = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return idx >= 0 ? p.slice(idx + 1) : p;
}
```

- [ ] **Step 7: Replace `style.css`**

Replace `desktop/src/style.css` with:
```css
:root {
  --bg: #ffffff;
  --bg-elevated: #f6f6f6;
  --text: #222;
  --text-secondary: #888;
  --border: #e5e5e5;
  --accent: #3b82f6;
  --good: #22c55e;
  --warn: #f59e0b;
  --err: #ef4444;
}
* { box-sizing: border-box; }
body { margin: 0; font-family: -apple-system, system-ui, sans-serif; color: var(--text); background: var(--bg); }
#app { display: flex; flex-direction: column; height: 100vh; }
.toolbar { display: flex; justify-content: space-between; align-items: center; padding: 8px 14px; border-bottom: 1px solid var(--border); background: var(--bg-elevated); }
.toolbar .title { font-weight: 600; }
.toolbar .btn { margin-left: 6px; padding: 4px 10px; background: transparent; border: 1px solid var(--border); border-radius: 4px; cursor: pointer; font-size: 13px; }
.toolbar .btn:disabled { opacity: 0.4; cursor: not-allowed; }
#dropzone { padding: 10px 14px; }
.dropzone { border: 1.5px dashed var(--border); border-radius: 6px; padding: 16px; text-align: center; color: var(--text-secondary); font-size: 13px; }
.dropzone.hover { border-color: var(--accent); color: var(--accent); }
.dropzone .link { color: var(--accent); text-decoration: underline; cursor: pointer; }
#list { flex: 1; overflow-y: auto; padding: 4px 14px 12px; }
.file-list .row { display: grid; grid-template-columns: 20px 1fr 80px 80px 60px; gap: 10px; padding: 8px 10px; border-radius: 4px; font-size: 13px; align-items: center; }
.file-list .row + .row { margin-top: 4px; }
.file-list .row.status-working { background: rgba(59, 130, 246, 0.08); }
.file-list .row.status-done .savings { color: var(--good); font-weight: 600; }
.file-list .row.status-error .icon { color: var(--err); }
.file-list .size.old { color: var(--text-secondary); text-decoration: line-through; }
.file-list .size.new { color: var(--good); }
.file-list .empty { text-align: center; color: var(--text-secondary); padding: 30px; font-size: 13px; }
#statusbar { padding: 8px 14px; border-top: 1px solid var(--border); background: var(--bg-elevated); font-size: 12px; color: var(--text-secondary); }
```

- [ ] **Step 8: Verify dev run**

From `desktop/`: `npm run tauri dev`
Expected: window opens with toolbar, empty dropzone, "No files yet" placeholder. Drag a file onto the window and it appears as a pending row.

- [ ] **Step 9: Commit**

```bash
cd /Users/csii/Desktop/git/ImageOpt
git add desktop/src/ desktop/src-tauri/tauri.conf.json
git commit -m "feat: base UI shell — dropzone, file list, toolbar"
```

---

### Task 9: Settings panel UI — sliders + toggles with in-memory state

**Files:**
- Create: `desktop/src/ui/settings.ts`
- Modify: `desktop/src/main.ts` (wire `onSettings` to open the panel)
- Modify: `desktop/src/style.css` (append settings styles)

- [ ] **Step 1: Create settings panel**

Create `desktop/src/ui/settings.ts`:
```ts
export interface Settings {
  jpegQuality: number;
  pngQuality: number;
  webpQuality: number;
  emitWebp: boolean;
  emitAvif: boolean;            // disabled in UI for v1
  moveOriginalsToTrash: boolean;
  concurrency: number;
}

export const DEFAULT_SETTINGS: Settings = {
  jpegQuality: 75,
  pngQuality: 75,
  webpQuality: 75,
  emitWebp: false,
  emitAvif: false,
  moveOriginalsToTrash: true,
  concurrency: 4,
};

let current: Settings = { ...DEFAULT_SETTINGS };

export function getSettings(): Settings { return { ...current }; }
export function setSettings(next: Settings) { current = { ...next }; }

export function openSettingsPanel(onChange: (s: Settings) => void) {
  const existing = document.getElementById('settings-overlay');
  if (existing) existing.remove();
  const overlay = document.createElement('div');
  overlay.id = 'settings-overlay';
  overlay.innerHTML = `
    <div class="settings-panel">
      <h2>Settings</h2>
      <label>JPEG quality <span id="v-jpeg">${current.jpegQuality}</span></label>
      <input type="range" min="1" max="100" id="jpeg" value="${current.jpegQuality}" />
      <label>PNG quality <span id="v-png">${current.pngQuality}</span></label>
      <input type="range" min="1" max="100" id="png" value="${current.pngQuality}" />
      <label>WebP quality <span id="v-webp">${current.webpQuality}</span></label>
      <input type="range" min="1" max="100" id="webp" value="${current.webpQuality}" />
      <label><input type="checkbox" id="emitWebp" ${current.emitWebp ? 'checked' : ''} /> Also emit WebP alongside</label>
      <label><input type="checkbox" id="emitAvif" disabled /> Also emit AVIF (coming soon)</label>
      <label><input type="checkbox" id="trash" ${current.moveOriginalsToTrash ? 'checked' : ''} /> Move originals to Trash</label>
      <div class="settings-actions"><button id="done" class="btn">Done</button></div>
    </div>`;
  document.body.appendChild(overlay);

  const bindRange = (id: string, key: keyof Settings) => {
    const el = overlay.querySelector(`#${id}`) as HTMLInputElement;
    const label = overlay.querySelector(`#v-${id}`) as HTMLElement;
    el.oninput = () => { label.textContent = el.value; (current as any)[key] = parseInt(el.value, 10); onChange({ ...current }); };
  };
  bindRange('jpeg', 'jpegQuality');
  bindRange('png', 'pngQuality');
  bindRange('webp', 'webpQuality');
  (overlay.querySelector('#emitWebp') as HTMLInputElement).onchange = (e) => { current.emitWebp = (e.target as HTMLInputElement).checked; onChange({ ...current }); };
  (overlay.querySelector('#trash') as HTMLInputElement).onchange = (e) => { current.moveOriginalsToTrash = (e.target as HTMLInputElement).checked; onChange({ ...current }); };
  (overlay.querySelector('#done') as HTMLButtonElement).onclick = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
}
```

- [ ] **Step 2: Wire the toolbar button**

In `desktop/src/main.ts`, change the `onSettings` handler:
```ts
import { openSettingsPanel, getSettings, setSettings } from './ui/settings';
// ...
mountToolbar(document.getElementById('toolbar')!, {
  onSettings: () => openSettingsPanel((s) => setSettings(s)),
  onUndo: () => console.log('undo clicked'),
});
```

- [ ] **Step 3: Style the settings overlay**

Append to `desktop/src/style.css`:
```css
#settings-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.3); display: flex; align-items: center; justify-content: center; z-index: 100; }
.settings-panel { background: var(--bg); border-radius: 8px; padding: 20px 24px; width: 360px; box-shadow: 0 10px 40px rgba(0,0,0,0.15); }
.settings-panel h2 { margin: 0 0 14px; font-size: 16px; }
.settings-panel label { display: block; margin-top: 12px; font-size: 13px; color: var(--text); }
.settings-panel input[type="range"] { width: 100%; margin-top: 4px; }
.settings-panel input[type="checkbox"] { margin-right: 6px; }
.settings-actions { margin-top: 16px; text-align: right; }
```

- [ ] **Step 4: Verify dev run**

From `desktop/`: `npm run tauri dev`
Expected: clicking the Settings button opens a panel with three sliders and checkboxes; sliders update their labels live; Done button closes it; clicking the backdrop also closes it.

- [ ] **Step 5: Commit**

```bash
git add desktop/src/
git commit -m "feat: settings panel with quality sliders and toggles"
```

---

### Task 10: Persist settings via `tauri-plugin-store`

**Files:**
- Modify: `desktop/src-tauri/Cargo.toml`
- Modify: `desktop/src-tauri/src/lib.rs` (or `main.rs` — whatever the scaffold created)
- Modify: `desktop/src-tauri/capabilities/default.json`
- Modify: `desktop/package.json`
- Modify: `desktop/src/ui/settings.ts` (read/write via plugin)
- Modify: `desktop/src/main.ts` (load settings before mounting)

- [ ] **Step 1: Add Rust plugin**

From `desktop/src-tauri/`:
```bash
cargo add tauri-plugin-store@2
```

- [ ] **Step 2: Register the plugin in Rust**

In `desktop/src-tauri/src/lib.rs`, inside `run()` / the Tauri builder chain, add:
```rust
.plugin(tauri_plugin_store::Builder::new().build())
```

- [ ] **Step 3: Grant the capability**

Edit `desktop/src-tauri/capabilities/default.json` and add to the `permissions` array:
```json
"store:default"
```

- [ ] **Step 4: Add the JS wrapper**

From `desktop/`:
```bash
npm install @tauri-apps/plugin-store
```

- [ ] **Step 5: Load/save via the store**

Replace the top of `desktop/src/ui/settings.ts` (the `let current` line and below through `setSettings`) with:
```ts
import { Store } from '@tauri-apps/plugin-store';

const SETTINGS_FILE = 'settings.json';
let storePromise: Promise<Store> | null = null;

function getStore(): Promise<Store> {
  if (!storePromise) storePromise = Store.load(SETTINGS_FILE);
  return storePromise;
}

let current: Settings = { ...DEFAULT_SETTINGS };

export async function loadSettings(): Promise<Settings> {
  try {
    const s = await getStore();
    const persisted = await s.get<Settings>('settings');
    if (persisted) current = { ...DEFAULT_SETTINGS, ...persisted };
  } catch (err) {
    console.warn('settings load failed, using defaults:', err);
  }
  return { ...current };
}

export async function saveSettings(next: Settings): Promise<void> {
  current = { ...next };
  try {
    const s = await getStore();
    await s.set('settings', current);
    await s.save();
  } catch (err) {
    console.error('settings save failed:', err);
  }
}

export function getSettings(): Settings { return { ...current }; }
```

- [ ] **Step 6: Update the `openSettingsPanel` onChange handler**

In `desktop/src/ui/settings.ts`, replace every `onChange({ ...current })` with a save:
```ts
// change the signature comment; callers pass a handler that may be async
const bindRange = (id: string, key: keyof Settings) => {
  const el = overlay.querySelector(`#${id}`) as HTMLInputElement;
  const label = overlay.querySelector(`#v-${id}`) as HTMLElement;
  el.oninput = () => {
    label.textContent = el.value;
    (current as any)[key] = parseInt(el.value, 10);
    saveSettings(current);
  };
};
```

Make the same change for the `emitWebp` and `trash` checkbox handlers — replace `onChange({ ...current })` with `saveSettings(current)`.

Remove the `onChange` parameter from `openSettingsPanel`; just export it as `openSettingsPanel()`.

- [ ] **Step 7: Bootstrap settings on app start**

In `desktop/src/main.ts`, replace the imports-and-mount block with:
```ts
import { mountToolbar } from './ui/toolbar';
import { mountDropzone } from './ui/dropzone';
import { mountFileList } from './ui/file-list';
import { store } from './state';
import { basename } from './util/path';
import { openSettingsPanel, loadSettings } from './ui/settings';

async function main() {
  await loadSettings();

  mountToolbar(document.getElementById('toolbar')!, {
    onSettings: () => openSettingsPanel(),
    onUndo: () => console.log('undo clicked'),
  });

  mountDropzone(document.getElementById('dropzone')!, (paths) => {
    for (const p of paths) {
      store.upsert({
        id: crypto.randomUUID(),
        path: p,
        name: basename(p),
        status: 'pending',
      });
    }
  });

  mountFileList(document.getElementById('list')!);
}
main();
```

- [ ] **Step 8: Verify persistence**

`npm run tauri dev`. Open Settings, change JPEG quality to 50, close the panel. Close the app (Cmd-Q). Reopen with `npm run tauri dev`. Open Settings again — JPEG should still be 50.

- [ ] **Step 9: Commit**

```bash
git add desktop/
git commit -m "feat: persist settings via tauri-plugin-store"
```

---

## Part D: Compress flow end-to-end

Now wire up the actual compression pipeline. At the end of Part D you can drop a file in, watch it compress, see the original in Trash, and the source file overwritten with the compressed version.

### Task 11: Rust `sidecar.rs` — spawn Node, JSON-lines reader/writer

**Files:**
- Create: `desktop/src-tauri/src/sidecar.rs`
- Modify: `desktop/src-tauri/src/lib.rs` (declare module, register state)
- Modify: `desktop/src-tauri/Cargo.toml` (add deps)

- [ ] **Step 1: Add Cargo deps**

From `desktop/src-tauri/`:
```bash
cargo add tokio --features full
cargo add serde --features derive
cargo add serde_json
cargo add uuid --features v4
cargo add anyhow
```

- [ ] **Step 2: Write the sidecar manager**

Create `desktop/src-tauri/src/sidecar.rs`:
```rust
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeCommand {
    pub cmd: &'static str, // always "encode"
    pub id: String,
    pub src: String,
    pub ext: String,
    pub opts: EncodeOpts,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeOpts {
    pub jpeg_quality: u32,
    pub png_quality: u32,
    pub webp_quality: u32,
    pub emit_webp: bool,
    pub emit_avif: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SidecarEvent {
    Done {
        id: String,
        tmp: String,
        #[serde(rename = "srcBytes")] src_bytes: u64,
        #[serde(rename = "outBytes")] out_bytes: u64,
        companions: Vec<Companion>,
    },
    Error { id: Option<String>, msg: String },
    #[serde(rename = "skipped-no-gain")]
    SkippedNoGain { id: String, #[serde(rename = "srcBytes")] src_bytes: u64 },
    #[serde(rename = "companion-error")]
    CompanionError { id: String, ext: String, msg: String },
    #[serde(rename = "parse-error")]
    ParseError { msg: String, line: String },
}

#[derive(Debug, Deserialize, Clone)]
pub struct Companion {
    pub ext: String,
    pub tmp: String,
    #[serde(rename = "outBytes")] pub out_bytes: u64,
}

pub struct Sidecar {
    child: Child,
    pub stdin: Arc<Mutex<ChildStdin>>,
    pub events: mpsc::UnboundedReceiver<SidecarEvent>,
}

impl Sidecar {
    /// Spawn a Node sidecar. `node_path` is the `node` binary; `script_path` is `src/sidecar.js`.
    pub async fn spawn(node_path: PathBuf, script_path: PathBuf) -> Result<Self> {
        let mut child = Command::new(&node_path)
            .arg(&script_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn {:?}", node_path))?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;

        let (tx, rx) = mpsc::unbounded_channel::<SidecarEvent>();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<SidecarEvent>(&line) {
                    Ok(evt) => { let _ = tx.send(evt); }
                    Err(err) => {
                        eprintln!("sidecar stdout parse error: {err} line: {line}");
                    }
                }
            }
        });

        Ok(Self {
            child,
            stdin: Arc::new(Mutex::new(stdin)),
            events: rx,
        })
    }

    pub async fn send(&self, cmd: &EncodeCommand) -> Result<()> {
        let mut line = serde_json::to_string(cmd)?;
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    pub async fn shutdown(mut self) {
        drop(self.stdin);
        let _ = self.child.kill().await;
    }
}
```

- [ ] **Step 3: Add a smoke test against the real sidecar**

Append to `desktop/src-tauri/src/sidecar.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        // desktop/src-tauri/ → repo root is two levels up
        let here = env!("CARGO_MANIFEST_DIR");
        PathBuf::from(here).join("../..").canonicalize().unwrap()
    }

    #[tokio::test]
    async fn spawns_sidecar_and_echoes_parse_error() {
        let root = repo_root();
        let script = root.join("src/sidecar.js");
        let mut sc = Sidecar::spawn(PathBuf::from("node"), script).await.unwrap();
        // Write a malformed line directly via stdin
        {
            let mut stdin = sc.stdin.lock().await;
            stdin.write_all(b"not-json\n").await.unwrap();
            stdin.flush().await.unwrap();
        }
        let evt = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            sc.events.recv(),
        ).await.unwrap().unwrap();
        assert!(matches!(evt, SidecarEvent::ParseError { .. }));
        sc.shutdown().await;
    }
}
```

- [ ] **Step 4: Declare the module**

In `desktop/src-tauri/src/lib.rs`, near the top add:
```rust
mod sidecar;
```

- [ ] **Step 5: Run the Rust test**

```bash
cd desktop/src-tauri
cargo test sidecar::tests::
```
Expected: the test passes (finds `node` on PATH, spawns sidecar, gets a parse-error event).

- [ ] **Step 6: Commit**

```bash
cd /Users/csii/Desktop/git/ImageOpt
git add desktop/src-tauri/
git commit -m "feat: Rust sidecar manager with JSON-lines protocol"
```

---

### Task 12: Rust `compress` command — dispatch to sidecar, emit events

**Files:**
- Create: `desktop/src-tauri/src/commands.rs`
- Modify: `desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Create command module**

Create `desktop/src-tauri/src/commands.rs`:
```rust
use crate::sidecar::{EncodeCommand, EncodeOpts, Sidecar, SidecarEvent};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

#[derive(Default, Clone)]
pub struct SidecarState(pub Arc<Mutex<Option<Sidecar>>>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressArgs {
    pub batch_id: String,
    pub files: Vec<CompressFile>,
    pub opts: EncodeOpts,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressFile {
    pub id: String,
    pub path: String,
    pub ext: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileDonePayload {
    pub id: String,
    pub tmp: String,
    pub src_bytes: u64,
    pub out_bytes: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileErrorPayload {
    pub id: String,
    pub msg: String,
}

fn sidecar_script_path(app: &AppHandle) -> PathBuf {
    // During dev, use the project sidecar.js. Task 21 will switch this to
    // a bundled sidecar resource path.
    let resource_dir = app.path().resource_dir().ok();
    if let Some(dir) = resource_dir {
        let candidate = dir.join("sidecar/sidecar.js");
        if candidate.exists() { return candidate; }
    }
    // Dev fallback: walk up from the Cargo manifest
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../src/sidecar.js").canonicalize().unwrap_or(here)
}

async fn ensure_sidecar(app: &AppHandle, state: &SidecarState) -> Result<(), String> {
    let mut guard = state.0.lock().await;
    if guard.is_some() { return Ok(()); }
    let script = sidecar_script_path(app);
    let sc = Sidecar::spawn(PathBuf::from("node"), script)
        .await
        .map_err(|e| e.to_string())?;
    let app_clone = app.clone();
    *guard = Some(sc);
    // Drain events in a background task. We spawn one per Sidecar instance.
    let st = app.state::<SidecarState>();
    let state_arc: Arc<Mutex<Option<Sidecar>>> = Arc::new(Mutex::new(None));
    let _ = (st, state_arc, app_clone);
    Ok(())
}

#[tauri::command]
pub async fn compress(
    app: AppHandle,
    state: State<'_, SidecarState>,
    args: CompressArgs,
) -> Result<(), String> {
    ensure_sidecar(&app, &state).await?;
    // Send one encode command per file.
    {
        let guard = state.0.lock().await;
        let sc = guard.as_ref().ok_or("sidecar missing")?;
        for f in &args.files {
            let cmd = EncodeCommand {
                cmd: "encode",
                id: f.id.clone(),
                src: f.path.clone(),
                ext: f.ext.clone(),
                opts: args.opts.clone(),
            };
            sc.send(&cmd).await.map_err(|e| e.to_string())?;
        }
    }
    // Drain the receiver in a task; route each event to the frontend.
    let app_c = app.clone();
    let state_arc: Arc<Mutex<Option<Sidecar>>> = state.inner().0.clone().into();
    tokio::spawn(async move {
        // Take ownership of the receiver only while the batch is in flight.
        // We re-acquire the mutex per event so other callers can send.
        loop {
            let evt_opt = {
                let mut guard = state_arc.lock().await;
                match guard.as_mut() {
                    Some(sc) => sc.events.recv().await,
                    None => None,
                }
            };
            match evt_opt {
                Some(SidecarEvent::Done { id, tmp, src_bytes, out_bytes, .. }) => {
                    let _ = app_c.emit("file-done", FileDonePayload { id, tmp, src_bytes, out_bytes });
                }
                Some(SidecarEvent::Error { id, msg }) => {
                    if let Some(id) = id {
                        let _ = app_c.emit("file-error", FileErrorPayload { id, msg });
                    } else {
                        eprintln!("sidecar error (no id): {msg}");
                    }
                }
                Some(SidecarEvent::SkippedNoGain { id, src_bytes }) => {
                    let _ = app_c.emit("file-skipped", serde_json::json!({ "id": id, "srcBytes": src_bytes }));
                }
                Some(SidecarEvent::CompanionError { id, ext, msg }) => {
                    let _ = app_c.emit("companion-error", serde_json::json!({ "id": id, "ext": ext, "msg": msg }));
                }
                Some(SidecarEvent::ParseError { msg, line }) => {
                    eprintln!("sidecar parse error: {msg} line: {line}");
                }
                None => break,
            }
        }
    });
    Ok(())
}
```

> **Note:** The event drainer above is intentionally naive — it runs forever per `compress` call, which means repeated calls would spawn duplicate drainers. Task 15 will promote it to a single on-startup drainer; keep the duplicate for now so each task is self-contained.

- [ ] **Step 2: Register the state and command**

In `desktop/src-tauri/src/lib.rs`, replace the contents with:
```rust
mod commands;
mod sidecar;

use commands::SidecarState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SidecarState::default())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![commands::compress])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cd desktop/src-tauri
cargo build
```
Expected: compiles without errors. Unused-variable warnings are acceptable for now; the `state_arc: Arc<...>` dance will be cleaned up in Task 15.

- [ ] **Step 4: Commit**

```bash
cd /Users/csii/Desktop/git/ImageOpt
git add desktop/src-tauri/
git commit -m "feat: Rust compress command dispatches to sidecar"
```

---

### Task 13: Frontend dropzone → `invoke('compress')` wire-up

**Files:**
- Modify: `desktop/src/main.ts`
- Modify: `desktop/src/state.ts` (add batch id)
- Modify: `desktop/src/ui/dropzone.ts` (unchanged; documentation only)
- Modify: `desktop/src-tauri/capabilities/default.json`

- [ ] **Step 1: Write an invoke wrapper**

Create `desktop/src/ipc.ts`:
```ts
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
    emitWebp: boolean;
    emitAvif: boolean;
  };
}

export async function compress(args: CompressArgs): Promise<void> {
  await invoke('compress', { args });
}

export interface FileDonePayload { id: string; tmp: string; srcBytes: number; outBytes: number; }
export interface FileErrorPayload { id: string; msg: string; }
export interface FileSkippedPayload { id: string; srcBytes: number; }

export function onFileDone(cb: (p: FileDonePayload) => void) { return listen<FileDonePayload>('file-done', (e) => cb(e.payload)); }
export function onFileError(cb: (p: FileErrorPayload) => void) { return listen<FileErrorPayload>('file-error', (e) => cb(e.payload)); }
export function onFileSkipped(cb: (p: FileSkippedPayload) => void) { return listen<FileSkippedPayload>('file-skipped', (e) => cb(e.payload)); }

export function toOpts(s: Settings) {
  return {
    jpegQuality: s.jpegQuality,
    pngQuality: s.pngQuality,
    webpQuality: s.webpQuality,
    emitWebp: s.emitWebp,
    emitAvif: s.emitAvif,
  };
}
```

- [ ] **Step 2: Wire the dropzone to invoke compress**

Replace the `mountDropzone` handler in `desktop/src/main.ts` with:
```ts
import { compress, toOpts } from './ipc';
import { getSettings } from './ui/settings';

function extOf(p: string): string {
  const idx = p.lastIndexOf('.');
  return idx >= 0 ? p.slice(idx).toLowerCase() : '';
}

mountDropzone(document.getElementById('dropzone')!, async (paths) => {
  const files = paths.map((p) => ({
    id: crypto.randomUUID(),
    path: p,
    ext: extOf(p),
    name: basename(p),
  }));
  for (const f of files) {
    store.upsert({ id: f.id, path: f.path, name: f.name, status: 'working' });
  }
  try {
    await compress({
      batchId: crypto.randomUUID(),
      files: files.map(({ id, path, ext }) => ({ id, path, ext })),
      opts: toOpts(getSettings()),
    });
  } catch (err) {
    console.error('compress failed:', err);
    for (const f of files) {
      store.update(f.path, { status: 'error', errorMsg: String(err) });
    }
  }
});
```

- [ ] **Step 3: Grant capabilities**

Edit `desktop/src-tauri/capabilities/default.json` and add to `permissions`:
```
"core:event:default"
```
(Replace dots/colons if your Tauri 2.x version uses slightly different naming; consult `cargo tauri permission ls` if it errors.)

- [ ] **Step 4: Verify**

`npm run tauri dev`. Drop a JPEG onto the window. Expected console output: `file-done` event (check DevTools in the Tauri window — right-click → Inspect). The file row should stay in "working" state because we haven't wired event listeners yet — that's Task 14.

- [ ] **Step 5: Commit**

```bash
git add desktop/
git commit -m "feat: dropzone invokes compress command"
```

---

### Task 14: Frontend file-list reacts to events

**Files:**
- Modify: `desktop/src/main.ts`

- [ ] **Step 1: Subscribe to events in `main.ts`**

In `desktop/src/main.ts`, add near the other imports:
```ts
import { onFileDone, onFileError, onFileSkipped } from './ipc';
```

Inside `main()`, after `await loadSettings();` but before the mounts, add:
```ts
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
```

Then add a helper on the store in `desktop/src/state.ts`:
```ts
  snapshotById(id: string): FileRow | undefined {
    for (const r of this.rows.values()) if (r.id === id) return r;
    return undefined;
  }
```
(This is the same as `findById` — just renaming for consistency. Delete `findById` to avoid duplication.)

- [ ] **Step 2: Verify**

`npm run tauri dev`. Drop a JPEG. Row should go `working` → `done` with before/after sizes and a green `−NN%` savings.

> **Note:** The source file is now being **renamed from a temp path** but we haven't wired the Trash-and-rename in Rust yet. The sidecar wrote to `/tmp/imageopt-<uuid>.jpg` — that file exists but the source is untouched. Task 15 adds the atomic replace.

- [ ] **Step 3: Commit**

```bash
git add desktop/src/
git commit -m "feat: file list reacts to sidecar events"
```

---

### Task 15: Rust Trash + atomic rename per file

**Files:**
- Create: `desktop/src-tauri/src/trash.rs`
- Create: `desktop/src-tauri/src/batch.rs`
- Modify: `desktop/src-tauri/src/commands.rs`
- Modify: `desktop/src-tauri/src/lib.rs`
- Modify: `desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Add the `trash` crate**

From `desktop/src-tauri/`:
```bash
cargo add trash@5
```

- [ ] **Step 2: Create the trash wrapper**

Create `desktop/src-tauri/src/trash.rs`:
```rust
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Outcome of disposing of the original file. Used by undo to know how to recover.
#[derive(Debug, Clone)]
pub enum DisposalKind {
    /// Moved to system Trash.
    Trashed,
    /// Renamed in place with a ".original" suffix because Trash was unavailable.
    RenamedFallback { backup_path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct Disposal {
    pub original_path: PathBuf,
    pub kind: DisposalKind,
}

/// Move to OS Trash; on failure, rename to `<name>.original.<ext>` alongside the source.
pub fn dispose_original(path: &Path) -> Result<Disposal> {
    let abs = path.canonicalize()?;
    match trash::delete(&abs) {
        Ok(()) => Ok(Disposal { original_path: abs, kind: DisposalKind::Trashed }),
        Err(primary) => {
            let backup = fallback_backup_path(&abs);
            std::fs::rename(&abs, &backup).map_err(|secondary| {
                anyhow::anyhow!("trash failed ({primary}); rename fallback also failed: {secondary}")
            })?;
            Ok(Disposal {
                original_path: abs,
                kind: DisposalKind::RenamedFallback { backup_path: backup },
            })
        }
    }
}

fn fallback_backup_path(src: &Path) -> PathBuf {
    let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = src.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
    let parent = src.parent().unwrap_or_else(|| Path::new("."));
    let mut candidate = parent.join(format!("{stem}.original{ext}"));
    let mut n = 2;
    while candidate.exists() {
        candidate = parent.join(format!("{stem}.original-{n}{ext}"));
        n += 1;
    }
    candidate
}

/// Restore disposals. For Trashed items, matches by original path; for RenamedFallback,
/// renames the backup back into place. Returns (restored_count, attempted_count).
pub fn restore_all(disposals: &[Disposal]) -> Result<usize> {
    let mut restored = 0;
    let trashed_targets: Vec<&PathBuf> = disposals
        .iter()
        .filter_map(|d| matches!(d.kind, DisposalKind::Trashed).then_some(&d.original_path))
        .collect();
    if !trashed_targets.is_empty() {
        let items = trash::os_limited::list()?;
        for want in &trashed_targets {
            if let Some(item) = items.iter().find(|i| PathBuf::from(&i.original_parent).join(&i.name) == **want) {
                if trash::os_limited::restore_all([item.clone()]).is_ok() {
                    restored += 1;
                }
            }
        }
    }
    for d in disposals {
        if let DisposalKind::RenamedFallback { backup_path } = &d.kind {
            if std::fs::rename(backup_path, &d.original_path).is_ok() {
                restored += 1;
            }
        }
    }
    Ok(restored)
}
```

- [ ] **Step 3: Create the batch state**

Create `desktop/src-tauri/src/batch.rs`:
```rust
use crate::trash::Disposal;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Default, Clone)]
pub struct Batch {
    pub id: String,
    pub disposals: Vec<Disposal>,
}

#[derive(Default)]
pub struct BatchState {
    pub current: Mutex<HashMap<String, Batch>>, // batch_id -> batch
    pub last_complete: Mutex<Option<Batch>>,
}

impl BatchState {
    pub fn start(&self, id: String) {
        let mut cur = self.current.lock().unwrap();
        cur.insert(id.clone(), Batch { id, disposals: vec![] });
    }
    pub fn record_disposal(&self, batch_id: &str, disposal: Disposal) {
        let mut cur = self.current.lock().unwrap();
        if let Some(b) = cur.get_mut(batch_id) {
            b.disposals.push(disposal);
        }
    }
    pub fn complete(&self, batch_id: &str) {
        let mut cur = self.current.lock().unwrap();
        if let Some(b) = cur.remove(batch_id) {
            *self.last_complete.lock().unwrap() = Some(b);
        }
    }
    pub fn take_last(&self) -> Option<Batch> {
        self.last_complete.lock().unwrap().take()
    }
}
```

- [ ] **Step 4: Wire Trash + rename into `compress`**

Replace the event drainer block in `desktop/src-tauri/src/commands.rs` with this per-file handler:
```rust
use std::path::Path;
use tokio::fs;

async fn apply_done(
    app: &AppHandle,
    batch_id: &str,
    id: &str,
    src_path: &str,
    tmp: &str,
    src_bytes: u64,
    out_bytes: u64,
    move_to_trash: bool,
    batches: &BatchState,
) -> Result<(), String> {
    if move_to_trash {
        let disposal = crate::trash::dispose_original(Path::new(src_path)).map_err(|e| e.to_string())?;
        let kind_note = match &disposal.kind {
            crate::trash::DisposalKind::Trashed => None,
            crate::trash::DisposalKind::RenamedFallback { backup_path } =>
                Some(format!("Trash unavailable; original backed up to {}", backup_path.display())),
        };
        batches.record_disposal(batch_id, disposal);
        if let Some(note) = kind_note {
            let _ = app.emit("trash-fallback", serde_json::json!({ "id": id, "note": note }));
        }
    } else {
        fs::remove_file(src_path).await.map_err(|e| e.to_string())?;
    }
    fs::rename(tmp, src_path).await.map_err(|e| e.to_string())?;
    let _ = app.emit("file-done", FileDonePayload {
        id: id.to_string(),
        tmp: tmp.to_string(),
        src_bytes,
        out_bytes,
    });
    Ok(())
}
```

Extend `CompressArgs` to include `moveOriginalsToTrash`:
```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressArgs {
    pub batch_id: String,
    pub files: Vec<CompressFile>,
    pub opts: EncodeOpts,
    pub move_originals_to_trash: bool,
}
```

Inside `compress`, after `ensure_sidecar`, call `batches.start(args.batch_id.clone())` where `batches` is a `State<BatchState>`. Build an `id → path` map from `args.files` so the drainer can look up the source path when it receives a `done` event. Adjust the event-drainer spawn to call `apply_done` on `SidecarEvent::Done` and `batches.complete(&batch_id)` when `pending_count` reaches zero.

Here is the full replacement body of `compress`:
```rust
#[tauri::command]
pub async fn compress(
    app: AppHandle,
    sc_state: State<'_, SidecarState>,
    batches: State<'_, BatchState>,
    args: CompressArgs,
) -> Result<(), String> {
    ensure_sidecar(&app, &sc_state).await?;
    batches.start(args.batch_id.clone());

    // id -> src path lookup
    let mut src_by_id: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for f in &args.files { src_by_id.insert(f.id.clone(), f.path.clone()); }
    let mut pending = args.files.len();

    {
        let guard = sc_state.0.lock().await;
        let sc = guard.as_ref().ok_or("sidecar missing")?;
        for f in &args.files {
            sc.send(&EncodeCommand {
                cmd: "encode",
                id: f.id.clone(),
                src: f.path.clone(),
                ext: f.ext.clone(),
                opts: args.opts.clone(),
            }).await.map_err(|e| e.to_string())?;
        }
    }

    let batch_id = args.batch_id.clone();
    let move_to_trash = args.move_originals_to_trash;
    let app_c = app.clone();
    let sc_arc = sc_state.inner().0.clone();
    let batches_handle = app.state::<BatchState>();
    let _ = batches_handle; // silences unused; state is global
    tokio::spawn(async move {
        let batches = app_c.state::<BatchState>();
        while pending > 0 {
            let evt = {
                let mut guard = sc_arc.lock().await;
                match guard.as_mut() {
                    Some(sc) => sc.events.recv().await,
                    None => None,
                }
            };
            match evt {
                Some(SidecarEvent::Done { id, tmp, src_bytes, out_bytes, .. }) => {
                    if let Some(src_path) = src_by_id.get(&id) {
                        if let Err(msg) = apply_done(&app_c, &batch_id, &id, src_path, &tmp, src_bytes, out_bytes, move_to_trash, &batches).await {
                            let _ = app_c.emit("file-error", FileErrorPayload { id: id.clone(), msg });
                        }
                    }
                    pending -= 1;
                }
                Some(SidecarEvent::Error { id: Some(id), msg }) => {
                    let _ = app_c.emit("file-error", FileErrorPayload { id, msg });
                    pending -= 1;
                }
                Some(SidecarEvent::SkippedNoGain { id, src_bytes }) => {
                    let _ = app_c.emit("file-skipped", serde_json::json!({ "id": id, "srcBytes": src_bytes }));
                    pending -= 1;
                }
                Some(SidecarEvent::CompanionError { id, ext, msg }) => {
                    let _ = app_c.emit("companion-error", serde_json::json!({ "id": id, "ext": ext, "msg": msg }));
                }
                Some(SidecarEvent::ParseError { msg, line }) => {
                    eprintln!("sidecar parse error: {msg} line: {line}");
                }
                Some(SidecarEvent::Error { id: None, msg }) => {
                    eprintln!("sidecar error (no id): {msg}");
                }
                None => break,
            }
        }
        batches.complete(&batch_id);
    });
    Ok(())
}
```

- [ ] **Step 5: Update `lib.rs` to register `BatchState`**

```rust
mod batch;
mod commands;
mod sidecar;
mod trash;

use batch::BatchState;
use commands::SidecarState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SidecarState::default())
        .manage(BatchState::default())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![commands::compress])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Update frontend to pass `moveOriginalsToTrash`**

In `desktop/src/ipc.ts`, extend `CompressArgs`:
```ts
export interface CompressArgs {
  batchId: string;
  files: CompressFile[];
  opts: { jpegQuality: number; pngQuality: number; webpQuality: number; emitWebp: boolean; emitAvif: boolean; };
  moveOriginalsToTrash: boolean;
}
```
In `desktop/src/main.ts`, update the `compress` call to include `moveOriginalsToTrash: getSettings().moveOriginalsToTrash`.

- [ ] **Step 7: Manual verification**

```bash
cp daxi/some-image.jpg /tmp/testimg.jpg
```
Then `npm run tauri dev`, drop `/tmp/testimg.jpg` onto the window. Verify:
- Row goes to "done" with smaller new size + green savings.
- `/tmp/testimg.jpg` now contains the compressed bytes (check `ls -la /tmp/testimg.jpg`).
- The original went to Trash (macOS: open the Trash and look for it).

- [ ] **Step 8: Commit**

```bash
git add desktop/
git commit -m "feat: atomic Trash + rename in compress flow"
```

---

## Part E: Undo + edge cases + polish

### Task 16: Rust `batch.rs` + `undo_last_batch` command

**Files:**
- Modify: `desktop/src-tauri/src/commands.rs`
- Modify: `desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add the command**

Append to `desktop/src-tauri/src/commands.rs`:
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoReport { pub restored: usize, pub attempted: usize }

#[tauri::command]
pub async fn undo_last_batch(batches: State<'_, BatchState>) -> Result<UndoReport, String> {
    let Some(batch) = batches.take_last() else {
        return Ok(UndoReport { restored: 0, attempted: 0 });
    };
    let attempted = batch.disposals.len();
    // Delete the compressed files that replaced originals (ignore errors: may already be gone).
    for d in &batch.disposals {
        let _ = tokio::fs::remove_file(&d.original_path).await;
    }
    let restored = crate::trash::restore_all(&batch.disposals).map_err(|e| e.to_string())?;
    Ok(UndoReport { restored, attempted })
}
```

Register it in `lib.rs`:
```rust
.invoke_handler(tauri::generate_handler![commands::compress, commands::undo_last_batch])
```

- [ ] **Step 2: Compile**

```bash
cd desktop/src-tauri
cargo build
```
Expected: success.

- [ ] **Step 3: Commit**

```bash
cd /Users/csii/Desktop/git/ImageOpt
git add desktop/src-tauri/
git commit -m "feat: undo_last_batch restores from Trash"
```

---

### Task 17: Frontend Undo button + disabled state

**Files:**
- Modify: `desktop/src/ipc.ts`
- Modify: `desktop/src/ui/toolbar.ts`
- Modify: `desktop/src/main.ts`

- [ ] **Step 1: Expose the command in the wrapper**

Append to `desktop/src/ipc.ts`:
```ts
export interface UndoReport { restored: number; attempted: number; }
export async function undoLastBatch(): Promise<UndoReport> {
  return invoke<UndoReport>('undo_last_batch');
}
```

- [ ] **Step 2: Make the toolbar undo-aware**

Change `mountToolbar` in `desktop/src/ui/toolbar.ts` to expose an enable/disable API:
```ts
export interface ToolbarApi { setUndoEnabled(enabled: boolean): void; }
export function mountToolbar(
  root: HTMLElement,
  handlers: { onSettings: () => void; onUndo: () => void },
): ToolbarApi {
  root.innerHTML = `
    <div class="toolbar">
      <div class="title">ImageOpt</div>
      <div class="actions">
        <button id="btn-settings" class="btn">⚙ Settings</button>
        <button id="btn-undo" class="btn" disabled>↺ Undo</button>
      </div>
    </div>`;
  const undoBtn = root.querySelector('#btn-undo') as HTMLButtonElement;
  (root.querySelector('#btn-settings') as HTMLButtonElement).onclick = handlers.onSettings;
  undoBtn.onclick = handlers.onUndo;
  return { setUndoEnabled: (v: boolean) => { undoBtn.disabled = !v; } };
}
```

- [ ] **Step 3: Enable Undo when a batch completes**

In `desktop/src/main.ts`, capture the toolbar API and track done counts:
```ts
import { undoLastBatch } from './ipc';

const toolbar = mountToolbar(document.getElementById('toolbar')!, {
  onSettings: () => openSettingsPanel(),
  onUndo: async () => {
    const r = await undoLastBatch();
    console.log(`undone: ${r.restored}/${r.attempted}`);
    toolbar.setUndoEnabled(false);
    // Re-render rows as "undone" by clearing them
    store.clear();
  },
});

// Track last-batch files: if all rows are terminal (done/error/skipped), enable undo.
store.subscribe((rows) => {
  const hasTerminal = rows.some((r) => r.status === 'done');
  const anyWorking = rows.some((r) => r.status === 'working' || r.status === 'pending');
  toolbar.setUndoEnabled(hasTerminal && !anyWorking);
});
```

- [ ] **Step 4: Manual verification**

`npm run tauri dev`. Drop a file. After it finishes, Undo should light up. Click Undo — source file returns (check via Finder / `ls`), compressed file removed, the row list clears.

- [ ] **Step 5: Commit**

```bash
git add desktop/
git commit -m "feat: undo button restores last batch"
```

---

### Task 18: Folder expansion + unsupported-file filtering

**Files:**
- Modify: `desktop/src-tauri/capabilities/default.json`
- Modify: `desktop/src-tauri/Cargo.toml`
- Modify: `desktop/src/ui/dropzone.ts`
- Modify: `desktop/src/main.ts`

- [ ] **Step 1: Add the fs plugin**

From `desktop/src-tauri/`:
```bash
cargo add tauri-plugin-fs@2
```

From `desktop/`:
```bash
npm install @tauri-apps/plugin-fs
```

Register in `desktop/src-tauri/src/lib.rs`:
```rust
.plugin(tauri_plugin_fs::init())
```

Grant in `desktop/src-tauri/capabilities/default.json` permissions:
```
"fs:allow-read-dir",
"fs:allow-stat"
```

- [ ] **Step 2: Write the expander**

Create `desktop/src/fs.ts`:
```ts
import { readDir, stat } from '@tauri-apps/plugin-fs';

const SUPPORTED = new Set(['.jpg', '.jpeg', '.png', '.webp']);

export async function expandPaths(paths: string[]): Promise<{ files: string[]; skipped: number }> {
  const files: string[] = [];
  let skipped = 0;
  for (const p of paths) {
    const s = await stat(p);
    if (s.isDirectory) {
      for (const entry of await readDir(p)) {
        const full = `${p}/${entry.name}`;
        const res = await expandPaths([full]);
        files.push(...res.files);
        skipped += res.skipped;
      }
    } else {
      const idx = p.lastIndexOf('.');
      const ext = idx >= 0 ? p.slice(idx).toLowerCase() : '';
      if (SUPPORTED.has(ext)) files.push(p);
      else skipped++;
    }
  }
  return { files, skipped };
}
```

- [ ] **Step 3: Wire it into the dropzone flow**

In `desktop/src/main.ts`, change the `mountDropzone` body:
```ts
import { expandPaths } from './fs';

mountDropzone(document.getElementById('dropzone')!, async (paths) => {
  const { files: expandedPaths, skipped } = await expandPaths(paths);
  if (skipped > 0) console.log(`Skipped ${skipped} unsupported`); // status bar in Task 19
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
    for (const f of files) store.update(f.path, { status: 'error', errorMsg: String(err) });
  }
});
```

- [ ] **Step 4: Manual verification**

`npm run tauri dev`. Drag a folder with a mix of images + .txt files. Verify only images are added and processed.

- [ ] **Step 5: Commit**

```bash
git add desktop/
git commit -m "feat: recursive folder expansion with format filtering"
```

---

### Task 19: Status bar — cumulative saved + Show Trash

**Files:**
- Create: `desktop/src/ui/statusbar.ts`
- Modify: `desktop/src-tauri/src/commands.rs`
- Modify: `desktop/src-tauri/src/lib.rs`
- Modify: `desktop/src/ipc.ts`
- Modify: `desktop/src/main.ts`

- [ ] **Step 1: Add `open_trash` command (Rust)**

Append to `desktop/src-tauri/src/commands.rs`:
```rust
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
```

Add `dirs-next` to Cargo.toml:
```bash
cd desktop/src-tauri
cargo add dirs-next
```

Register in `lib.rs`:
```rust
.invoke_handler(tauri::generate_handler![commands::compress, commands::undo_last_batch, commands::open_trash])
```

- [ ] **Step 2: Add JS wrapper**

Append to `desktop/src/ipc.ts`:
```ts
export async function openTrash(): Promise<void> { await invoke('open_trash'); }
```

- [ ] **Step 3: Create the status bar UI**

Create `desktop/src/ui/statusbar.ts`:
```ts
import { store, FileRow } from '../state';
import { openTrash } from '../ipc';

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

export function mountStatusBar(root: HTMLElement) {
  root.innerHTML = `<span id="sb-left"></span><span id="sb-right"></span>`;
  const left = root.querySelector('#sb-left') as HTMLElement;
  const right = root.querySelector('#sb-right') as HTMLElement;
  right.innerHTML = `Originals moved to Trash · <span class="link" id="show-trash">Show</span>`;
  (right.querySelector('#show-trash') as HTMLElement).onclick = () => { openTrash(); };

  function render(rows: FileRow[]) {
    let saved = 0;
    let done = 0;
    for (const r of rows) {
      if (r.status === 'done' && r.srcBytes && r.outBytes) {
        saved += r.srcBytes - r.outBytes;
        done++;
      }
    }
    left.innerHTML = done > 0 ? `Saved <b class="good">${fmtBytes(saved)}</b> across ${done} files` : '';
  }
  store.subscribe(render);
  render(store.snapshot());
}
```

Style the `#statusbar` and `.link` in `style.css`:
```css
#statusbar { display: flex; justify-content: space-between; align-items: center; }
#statusbar .link { color: var(--accent); text-decoration: underline; cursor: pointer; }
#statusbar .good { color: var(--good); }
```

- [ ] **Step 4: Mount it**

In `desktop/src/main.ts`:
```ts
import { mountStatusBar } from './ui/statusbar';
// ...
mountStatusBar(document.getElementById('statusbar')!);
```

- [ ] **Step 5: Manual verification**

`npm run tauri dev`. Drop a few files. Watch the left side of the status bar accumulate saved bytes. Click "Show" — the Trash / Recycle Bin should open.

- [ ] **Step 6: Commit**

```bash
git add desktop/
git commit -m "feat: status bar with cumulative saved and open-trash link"
```

---

### Task 20: Close-requested confirm dialog + sidecar crash recovery

**Files:**
- Modify: `desktop/src-tauri/src/lib.rs`
- Modify: `desktop/src/main.ts`
- Modify: `desktop/src/state.ts`

- [ ] **Step 1: Add a JS-side "any working" counter**

Append to `desktop/src/state.ts`:
```ts
export function anyWorking(): boolean {
  for (const r of store.snapshot()) {
    if (r.status === 'working' || r.status === 'pending') return true;
  }
  return false;
}
```

- [ ] **Step 2: Attach a `close-requested` handler in Rust**

In `desktop/src-tauri/src/lib.rs`, inside `pub fn run()`, between the last `.manage()` and `.run()`, add:
```rust
use tauri::{Emitter, Manager, WindowEvent};
// ...
    .on_window_event(|window, event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window.emit("close-requested", ());
        }
    })
```

Rust cannot round-trip a value from `eval`, so the flow is: Rust prevents the close and emits `close-requested`; JS decides whether to confirm and calls back into `confirm_close` (next step).

- [ ] **Step 3: Add `confirm_close` command**

Append to `commands.rs`:
```rust
use tauri::Window;

#[tauri::command]
pub async fn confirm_close(window: Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}
```

Register in `lib.rs` `generate_handler![...]`.

- [ ] **Step 4: Frontend listens and confirms**

In `desktop/src/main.ts`, add:
```ts
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { anyWorking } from './state';

listen('close-requested', async () => {
  if (anyWorking()) {
    const count = store.snapshot().filter((r) => r.status === 'working' || r.status === 'pending').length;
    if (!confirm(`${count} files still processing. Quit anyway?`)) return;
  }
  await invoke('confirm_close');
});
```

- [ ] **Step 5: Manual verification**

`npm run tauri dev`. Drop a large file. While it's processing, hit Cmd-W / click the close button. A confirm dialog should appear. If you click cancel, the app stays open.

- [ ] **Step 6: Commit**

```bash
git add desktop/
git commit -m "feat: confirm dialog on close while batch is in flight"
```

- [ ] **Step 7: Emit `sidecar-crashed` on EOF in Rust**

In `desktop/src-tauri/src/sidecar.rs`, change the reader task so that when `lines.next_line()` returns `Ok(None)` (EOF) we notify. The reader task is spawned without an `AppHandle`, so add a handle parameter to `Sidecar::spawn`:

```rust
use tauri::{AppHandle, Emitter};

impl Sidecar {
    pub async fn spawn(app: AppHandle, node_path: PathBuf, script_path: PathBuf) -> Result<Self> {
        // ... existing spawn code unchanged through let (tx, rx) = mpsc::unbounded_channel::<SidecarEvent>();
        let app_c = app.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        match serde_json::from_str::<SidecarEvent>(&line) {
                            Ok(evt) => { let _ = tx.send(evt); }
                            Err(err) => eprintln!("sidecar stdout parse error: {err} line: {line}"),
                        }
                    }
                    Ok(None) => {
                        let _ = app_c.emit("sidecar-crashed", ());
                        break;
                    }
                    Err(err) => {
                        eprintln!("sidecar stdout read error: {err}");
                        let _ = app_c.emit("sidecar-crashed", ());
                        break;
                    }
                }
            }
        });
        Ok(Self { child, stdin: Arc::new(Mutex::new(stdin)), events: rx })
    }
}
```

Update the single call site of `Sidecar::spawn` inside `commands::ensure_sidecar` to pass the `AppHandle`. At this point in the plan, `ensure_sidecar` still uses `PathBuf::from("node")`; Task 21 replaces that with `node_binary(app)`.
```rust
let sc = Sidecar::spawn(app.clone(), PathBuf::from("node"), script).await.map_err(|e| e.to_string())?;
```

Update the test in `sidecar.rs` — because it no longer has an `AppHandle`, wrap it or skip. Change the test to:
```rust
#[tokio::test]
#[ignore] // requires a Tauri AppHandle; cover via integration smoke instead
async fn spawns_sidecar_and_echoes_parse_error() { /* kept for reference; run manually */ }
```

- [ ] **Step 8: Clear sidecar state on crash + reset rows**

In `desktop/src-tauri/src/commands.rs`, add a helper and wire it into `lib.rs`:
```rust
pub async fn on_sidecar_crashed(sc_state: &SidecarState) {
    *sc_state.0.lock().await = None;
}
```

In `desktop/src-tauri/src/lib.rs`, add imports and a `.setup()` block:
```rust
use tauri::{Listener, Manager};

// ... inside builder chain, after the .manage() calls but before .run():
    .setup(|app| {
        let handle = app.handle().clone();
        handle.clone().listen("sidecar-crashed", move |_| {
            let h = handle.clone();
            tauri::async_runtime::spawn(async move {
                let st: tauri::State<commands::SidecarState> = h.state();
                commands::on_sidecar_crashed(st.inner()).await;
            });
        });
        Ok(())
    })
```

- [ ] **Step 9: Frontend — show toast, mark pending rows as errored**

In `desktop/src/main.ts`, after the other listeners:
```ts
import { listen } from '@tauri-apps/api/event';

listen('sidecar-crashed', () => {
  const snap = store.snapshot();
  for (const r of snap) {
    if (r.status === 'pending' || r.status === 'working') {
      store.update(r.path, { status: 'error', errorMsg: 'Engine crashed' });
    }
  }
  // Simple toast: alert is fine for v1; upgrade later.
  alert('Image engine crashed. It will restart on the next drop.');
});
```

- [ ] **Step 10: Manual verification**

`npm run tauri dev`. In a separate terminal, once a file is mid-compress, find the `node` child process (`pgrep -f 'src/sidecar.js'`) and `kill -9 <pid>`. Expected:
- Alert appears.
- The in-flight row turns ⚠ with "Engine crashed".
- Dropping a new file causes the sidecar to respawn and work resumes.

- [ ] **Step 11: Commit**

```bash
git add desktop/
git commit -m "feat: sidecar crash detection and recovery"
```

---

## Part F: Packaging

### Task 21: Node portable binary bundling + sidecar registration

**Files:**
- Create: `desktop/scripts/fetch-node.sh`
- Modify: `desktop/src-tauri/tauri.conf.json`
- Modify: `desktop/src-tauri/src/commands.rs` (`sidecar_script_path` body)
- Modify: `desktop/package.json` (prebuild script)
- Create: `desktop/src-tauri/binaries/.gitkeep`
- Modify: `.gitignore` (exclude binary dumps)

- [ ] **Step 1: Write the Node-fetch script**

Create `desktop/scripts/fetch-node.sh`:
```bash
#!/usr/bin/env bash
set -euo pipefail

VERSION="${NODE_VERSION:-v20.11.1}"
HOST_TRIPLE="$(rustc -vV | awk '/host:/ { print $2 }')"

BIN_DIR="$(cd "$(dirname "$0")/../src-tauri/binaries" && pwd)"
mkdir -p "$BIN_DIR"

download() {
  local triple="$1" platform="$2" arch="$3" ext="$4"
  local url="https://nodejs.org/dist/${VERSION}/node-${VERSION}-${platform}-${arch}.${ext}"
  local out="$BIN_DIR/node-${triple}${ext == "zip" ? ".exe" : ""}"
  echo "Fetching $url"
  case "$ext" in
    tar.gz)
      curl -fsSL "$url" | tar -xz -C "$BIN_DIR" --strip-components=2 "node-${VERSION}-${platform}-${arch}/bin/node"
      mv "$BIN_DIR/node" "$BIN_DIR/node-${triple}"
      chmod +x "$BIN_DIR/node-${triple}"
      ;;
    zip)
      tmp="$(mktemp -d)"
      curl -fsSL -o "$tmp/node.zip" "$url"
      unzip -q -d "$tmp" "$tmp/node.zip"
      cp "$tmp"/node-*/node.exe "$BIN_DIR/node-${triple}.exe"
      rm -rf "$tmp"
      ;;
  esac
}

case "$HOST_TRIPLE" in
  x86_64-apple-darwin)      download "$HOST_TRIPLE" darwin x64 tar.gz ;;
  aarch64-apple-darwin)     download "$HOST_TRIPLE" darwin arm64 tar.gz ;;
  x86_64-pc-windows-msvc)   download "$HOST_TRIPLE" win x64 zip ;;
  x86_64-unknown-linux-gnu) download "$HOST_TRIPLE" linux x64 tar.gz ;;
  *) echo "Unsupported host: $HOST_TRIPLE"; exit 1 ;;
esac

echo "Done. Host binary at $BIN_DIR/node-$HOST_TRIPLE"
```

Make it executable:
```bash
chmod +x desktop/scripts/fetch-node.sh
```

- [ ] **Step 2: Register sidecar in `tauri.conf.json`**

Edit `desktop/src-tauri/tauri.conf.json`, add to the `bundle` block:
```json
"externalBin": ["binaries/node"],
"resources": ["../../src/sidecar.js", "../../src/encoder.js", "../../src/config.js", "../../node_modules/sharp"]
```

> **Note:** Tauri expects `externalBin` to resolve per-target as `binaries/node-<triple>[.exe]`. The script produces exactly that shape.

- [ ] **Step 3: Update `sidecar_script_path`**

In `desktop/src-tauri/src/commands.rs`, replace `sidecar_script_path` with:
```rust
fn sidecar_script_path(app: &AppHandle) -> PathBuf {
    // In a bundled build, resources live under resource_dir().
    if let Ok(dir) = app.path().resource_dir() {
        let candidate = dir.join("src/sidecar.js");
        if candidate.exists() { return candidate; }
    }
    // Dev fallback
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../src/sidecar.js").canonicalize().unwrap_or(here)
}
```

And update `ensure_sidecar` to choose the bundled node binary in release, system `node` in dev:
```rust
fn node_binary(app: &AppHandle) -> PathBuf {
    if let Ok(dir) = app.path().resource_dir() {
        #[cfg(target_os = "windows")]
        let bundled = dir.join("node.exe");
        #[cfg(not(target_os = "windows"))]
        let bundled = dir.join("node");
        if bundled.exists() { return bundled; }
    }
    PathBuf::from("node")
}
```
Inside `ensure_sidecar`, change the spawn call (by this point in the plan, Task 20 has already added the `AppHandle` first argument):
```rust
let sc = Sidecar::spawn(app.clone(), node_binary(app), script).await.map_err(|e| e.to_string())?;
```

- [ ] **Step 4: Add prebuild hook**

In `desktop/package.json`, add to `scripts`:
```json
"pretauri": "./scripts/fetch-node.sh"
```
So `npm run tauri build` first pulls the right Node.

- [ ] **Step 5: Add `.gitignore` entry**

Append to `/Users/csii/Desktop/git/ImageOpt/.gitignore`:
```
desktop/src-tauri/binaries/node-*
```
Create a placeholder:
```bash
mkdir -p desktop/src-tauri/binaries
touch desktop/src-tauri/binaries/.gitkeep
```

- [ ] **Step 6: Commit**

```bash
git add desktop/ .gitignore
git commit -m "feat: bundle Node sidecar binary per target triple"
```

---

### Task 22: Release build + manual smoke test

**Files:**
- None (verification only)

- [ ] **Step 1: Run the release build**

From `desktop/`:
```bash
npm run tauri build
```
Expected: produces a `.dmg` in `desktop/src-tauri/target/release/bundle/dmg/` (macOS), `.msi` in `desktop/src-tauri/target/release/bundle/msi/` (Windows), or `.AppImage` / `.deb` under `target/release/bundle/` (Linux).

- [ ] **Step 2: Install and open**

On macOS: double-click the `.dmg`, drag ImageOpt to Applications, launch. You may need to right-click → Open to bypass Gatekeeper (unsigned build).

- [ ] **Step 3: Smoke test the full flow**

From the design spec's manual smoke test list:
1. Drop 5 assorted files from `daxi/`. Verify all shrink, originals in Trash.
2. Click Undo. Verify originals restored, compressed files gone.
3. Open Settings, drop JPEG to 50. Re-drop a file. Verify smaller output but decodable.
4. Enable "Emit WebP". Drop a JPEG. Verify both `.jpg` and `.webp` appear; `.jpg` overwrites, `.webp` is new.
5. Drop a folder with nested subfolders. Verify recursion.
6. Drop a file currently locked (open a `.jpg` in Preview, try). Verify ⚠ icon, no data loss.
7. Close app while batch running. Verify confirm dialog.

- [ ] **Step 4: File any regressions as follow-up tasks, not in this plan.**

- [ ] **Step 5: Tag the release**

```bash
git tag -a desktop-v0.1.0 -m "ImageOpt Desktop v0.1.0"
```

- [ ] **Step 6: Commit any tooling fixes made during the smoke test**

```bash
# Any adjustments from Step 3 failures get committed separately.
# If no changes, no commit needed.
```

---

## Appendix: test commands cheat sheet

```bash
# Run all JS tests
node --test src/*.test.js

# Run Rust tests
cd desktop/src-tauri && cargo test

# Run in dev
cd desktop && npm run tauri dev

# Build release
cd desktop && npm run tauri build

# Diff output against golden
./scripts/diff-against-golden.sh   # see Task 3, Step 4 for the inline version
```

## Appendix: regenerating golden fixtures

If you intentionally change encode parameters (e.g., switching mozjpeg off):

```bash
rm -rf fixtures/golden
imageopt ./fixtures/sample -o ./fixtures/golden
git add fixtures/golden
git commit -m "test: refresh golden fixtures after encoder change"
```
