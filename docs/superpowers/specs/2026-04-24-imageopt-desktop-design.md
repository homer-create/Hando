# ImageOpt Desktop — Design Spec

**Date:** 2026-04-24
**Status:** Approved (pending written-spec review)

## Goal

Turn the existing `imageopt` CLI into a Tauri-based desktop app with an ImageOptim-like UX: drag images in → compress in place → originals go to Trash → Undo available. Per-format quality sliders, optional WebP/AVIF companion outputs, extensible encoder registry.

## Scope

### In scope (v1)

- Tauri desktop app (macOS + Windows; Linux as build target but not primary test platform)
- Drag-and-drop files/folders into the main window
- Recursive folder expansion (collect supported files)
- In-place compression (overwrite source after successful encode)
- Originals moved to system Trash with one-level Undo
- Per-format quality sliders: JPEG / PNG / WebP (slots reserved for AVIF)
- Toggles for "also emit WebP alongside" and "also emit AVIF alongside"
- Settings persisted across launches
- Live per-file progress: status icon, original size, new size, savings %
- Bottom status bar: cumulative saved bytes + link to Trash
- Reuse existing `index.js` compression logic via Node sidecar (no re-encoding change)

### Out of scope (v1)

- In-place overwrite toggle off (stay ImageOptim-style only; CLI already has "separate output" mode)
- Per-file quality overrides (global only)
- Preview / before-after thumbnails
- Menu bar app mode
- Auto-update / signed releases (can be added post-v1)
- AVIF (architecture supports it — actual toggle is post-v1)
- Multi-batch undo (only last batch is undoable)
- Command-line launch arguments for the desktop app

## Architecture

Three-tier process model:

```
Tauri WebView (frontend: TS + vanilla DOM or light framework)
       │ Tauri invoke commands
       ▼
Tauri Rust core (src-tauri/)
  - Command handlers, sidecar lifecycle, Trash integration, undo log
       │ JSON lines over stdio
       ▼
Node sidecar (bundled)
  - Imports src/encoder.js, runs Sharp pipeline, emits per-file JSON
```

**Why Sharp sidecar (not Rust-native encoders):** user prioritised output quality parity with existing CLI. Sharp binds to mozjpeg / libimagequant / libwebp — the same C libraries the best Rust crates would bind to. Sidecar guarantees identical output at the cost of ~50 MB extra bundle size.

**Bundle size estimate:** macOS app ~60–80 MB (Node ~40 MB + sharp native ~20 MB + Tauri/webview ~5 MB).

**Communication protocol:** JSON lines over stdin/stdout. One command per line in, one event per line out. Simple, debuggable, cross-platform, no IPC library dependency.

### AVIF extensibility

Encoder module exposes an `ENCODERS` map keyed by extension (`.jpg`, `.png`, `.webp`, future `.avif`). Adding AVIF is one table entry (`.avif: (pipeline, opts) => pipeline.avif({ quality: opts.avifQuality })`) plus one settings slider + toggle. No structural changes required.

## File Structure

```
ImageOpt/
├── index.js                    # Existing CLI entry — keep, thin wrapper over encoder
├── src/
│   ├── encoder.js              # [new] Pure encode logic: input path + opts → output bytes
│   ├── sidecar.js              # [new] JSON-lines protocol loop; stdin in, stdout out
│   └── config.js               # [new] CONFIG constants extracted from index.js
├── desktop/
│   ├── src-tauri/
│   │   ├── src/
│   │   │   ├── main.rs         # Tauri app bootstrap
│   │   │   ├── commands.rs     # compress / undo_last_batch / open_trash / load_settings / save_settings
│   │   │   ├── sidecar.rs      # Spawn Node, read JSON lines, dispatch to event bus
│   │   │   ├── batch.rs        # Per-batch state: pending files, Trash paths for undo
│   │   │   └── trash.rs        # Wrapper over `trash` crate, with fallback to .original.<ext> copy
│   │   ├── Cargo.toml
│   │   ├── tauri.conf.json     # Registers sidecar binary + permissions
│   │   ├── icons/
│   │   └── binaries/           # Node runtime per target triple (populated by build script)
│   ├── src/
│   │   ├── index.html
│   │   ├── main.ts             # invoke() bindings, event subscriptions
│   │   ├── state.ts            # File list state, keyed by absolute path
│   │   ├── ui/
│   │   │   ├── dropzone.ts
│   │   │   ├── file-list.ts
│   │   │   ├── settings.ts
│   │   │   └── toolbar.ts
│   │   └── style.css
│   ├── package.json            # vite + typescript; no sharp here
│   └── vite.config.ts
└── package.json                # Existing CLI deps (sharp) — sidecar reads from this node_modules
```

## Components

### 1. `src/encoder.js` (shared with CLI)

Single responsibility: given `{ srcPath, ext, opts }`, produce a file at `dstPath` using Sharp. No file system side effects beyond the single output. No Trash logic.

```js
export const ENCODERS = {
  '.jpg':  (pipeline, opts) => pipeline.jpeg({ quality: opts.jpegQuality, mozjpeg: true }),
  '.jpeg': (pipeline, opts) => pipeline.jpeg({ quality: opts.jpegQuality, mozjpeg: true }),
  '.png':  (pipeline, opts) => pipeline.png({ quality: opts.pngQuality, palette: true, compressionLevel: 9 }),
  '.webp': (pipeline, opts) => pipeline.webp({ quality: opts.webpQuality }),
  // '.avif': (pipeline, opts) => pipeline.avif({ quality: opts.avifQuality }),  // future
};

export async function encode({ srcPath, dstPath, ext, opts }) {
  const pipeline = ENCODERS[ext.toLowerCase()](sharp(srcPath), opts);
  const { size: outBytes } = await pipeline.toFile(dstPath);
  return { outBytes };
}
```

### 2. `src/sidecar.js` (Node sidecar entry)

Reads JSON lines from stdin, runs encode jobs with bounded concurrency (reuse `runPool` pattern from existing CLI), writes JSON lines to stdout.

**Protocol:**
```
→ {"cmd":"encode","id":"uuid","src":"/a/b/photo.jpg","ext":".jpg","opts":{jpegQuality:75,emitWebp:false,emitAvif:false}}
← {"type":"done","id":"uuid","tmp":"/tmp/imageopt-uuid.jpg","srcBytes":820000,"outBytes":312000,"companions":[]}
← {"type":"error","id":"uuid","msg":"Input file is malformed"}
← {"type":"skipped-no-gain","id":"uuid","srcBytes":820000}
```

When `emitWebp: true`, `done` event's `companions` array includes `{tmp, dstPath, outBytes}` for the `.webp` file (same basename, same folder).

Concurrency: default 4 jobs in-flight (same as CLI).

### 3. `desktop/src-tauri/src/commands.rs`

Tauri command handlers:

- `compress({ batch_id, files: Vec<String>, opts: EncodeOpts }) → ()`
  Kicks off async job: writes JSON lines to sidecar, emits per-file events.
- `undo_last_batch() → Result<UndoReport, String>`
  Walks the last batch's Trash entries, restores originals, removes compressed files. No-op if no batch.
- `open_trash() → ()`
  Platform-native Trash opener.
- `load_settings() → Settings`
- `save_settings(Settings) → ()`

### 4. `desktop/src-tauri/src/sidecar.rs`

- Manages Node process lifecycle (spawn lazily on first compress, kill on app quit)
- Reader thread parses stdout JSON lines, forwards as Tauri events (`file-done`, `file-error`, `file-skipped`)
- Writer channel serialises commands to stdin
- Detects sidecar crash (stdout EOF) and emits `sidecar-crashed` to frontend

### 5. `desktop/src-tauri/src/batch.rs`

- `Batch { id, files: HashMap<id, FileEntry>, trash_entries: Vec<TrashEntry> }`
- `FileEntry { src_path, status, src_bytes, out_bytes }`
- Last-completed batch retained as `last_batch: Option<Batch>` for Undo.
- On new batch start, previous `last_batch` is committed (no longer undoable).

### 6. `desktop/src/ui/*.ts`

- `dropzone.ts`: HTML5 drag-drop + file-picker click. Filters to supported extensions. Resolves folders via Tauri's fs API.
- `file-list.ts`: Renders rows keyed by absolute path. Subscribes to `file-done` / `file-error` / `file-skipped` events.
- `settings.ts`: Three range inputs (JPEG / PNG / WebP quality, 1–100) + three checkboxes (Emit WebP / Emit AVIF — grayed for v1 / Move originals to Trash).
- `toolbar.ts`: Settings button, Undo button (disabled when no undoable batch).

## Data Flow: Drop → Compress → Done

1. User drops N files/folders onto the dropzone.
2. Frontend expands folders, filters to supported extensions, generates `batch_id`, adds rows to state (status: `pending`).
3. Frontend calls `invoke('compress', { batch_id, files, opts })`.
4. Rust spawns sidecar if not running. Writes one JSON-line command per file to sidecar stdin.
5. Sidecar encodes to a temp path (`${tmpDir}/imageopt-<uuid><ext>`), reads source + output byte counts, emits `done` / `error` / `skipped-no-gain`.
6. Rust reader receives each `done`:
   a. Move original to Trash (record in batch trash entries).
   b. Rename temp file to original path.
   c. Emit `file-done` Tauri event with `{ id, src_bytes, out_bytes }`.
7. Frontend updates the row (status icon, new size column, savings %).
8. When all files resolve, batch is marked complete and becomes the new `last_batch`.

## Error Handling & Edge Cases

| Case | Behaviour |
|---|---|
| Unsupported format dropped | Frontend filters; shows "Skipped N unsupported files" in status bar |
| Folder dropped | Recursively expanded; only supported files added |
| File locked / unreadable | Sidecar emits `error`; UI row gets ⚠ icon + tooltip; source untouched |
| Disk full during temp write | Same as above; error message mentions disk space |
| Output larger than source | Drop temp file, leave source alone; UI row shows "No gain · 0%" |
| Trash fails (perms / network volume) | Fall back to `photo.original.jpg` copy; emit warning event |
| Duplicate filenames across folders | Absolute path is the key; UI shows full path on hover |
| Main encode succeeds but companion (WebP/AVIF) fails | Main file still replaces original; companion emits a warning event; UI row shows ✓ with a "WebP failed" subline |
| Sidecar crash | Rust detects EOF → pending files flagged ⚠; `sidecar-crashed` event → frontend offers "Restart engine" toast |
| New files dropped while batch running | Appended to same batch; sidecar consumes FIFO |
| Undo when Trash already emptied | Error toast "Originals no longer in Trash"; Undo button disabled |
| Quit while batch in flight | `close-requested` → confirm dialog "N files still processing, quit anyway?" |
| Corrupt settings JSON | Log warning, fall back to defaults, overwrite on next save |

### Invariants

1. **Source file is never modified until output is successfully written.** On any failure, source stays.
2. **Trash + rename is atomic per file.** If rename of the temp file fails, restore the original from Trash before emitting error for that file.
3. **Each file has exactly three terminal states:** `done`, `error`, `skipped-no-gain`.
4. **Undo is idempotent.** Second Undo is a no-op, not an error.

## Settings Schema

```ts
{
  jpegQuality: number;      // 1-100, default 75
  pngQuality: number;       // 1-100, default 75
  webpQuality: number;      // 1-100, default 75
  // avifQuality: number;   // future
  emitWebp: boolean;        // default false
  emitAvif: boolean;        // default false, disabled in UI for v1
  moveOriginalsToTrash: boolean;  // default true
  concurrency: number;      // default 4
}
```

Persisted to `~/Library/Application Support/com.imageopt.app/settings.json` (macOS paths; Tauri's `app_config_dir` gives the right location per platform).

## Testing Strategy

### Tier 1: `src/encoder.js` unit tests

- Use `node:test` (Node 18+ builtin — no Jest/Vitest dep).
- Generate synthetic test fixtures (red JPEG, blue PNG, green WebP, nested mixed-case JPEG) — same shape as existing `fixtures/sample/`.
- Assertions per encoder call:
  - Output file exists and is decodable by Sharp.
  - Output size < input size (except the known "no gain" edge case, which is tested separately).
  - Output MIME type correct via Sharp metadata.
- Golden samples: run current CLI over `daxi/` once, save results to `fixtures/golden/`. Refactor-safety test: re-encode and assert ±2% byte tolerance.

### Tier 2: Sidecar protocol tests

- Spawn `src/sidecar.js` from a test, feed 3 JSON-line commands over stdin.
- Parse stdout lines; assert each command gets exactly one response (`done` / `error` / `skipped-no-gain`).
- Malformed JSON on stdin must not crash sidecar; it should emit an error line and keep running.
- SIGTERM → process exits cleanly within 1 second.

### Tier 3: Rust `cargo test`

- `commands.rs`: batch state transitions, undo-log integrity.
- `sidecar.rs`: spawn a fake echo binary, verify reader loop parses lines correctly.
- `trash.rs`: create temp file, call trash, assert path no longer exists.
- `batch.rs`: replay trash entries in reverse, assert files restored.

### Tier 4: Manual smoke test (per release)

1. Open app. Drop 5 assorted files from `daxi/`. Verify all shrink, originals in Trash.
2. Click Undo. Verify originals restored, compressed files gone.
3. Open Settings, drop JPEG to 50. Re-drop a file. Verify smaller output but decodable.
4. Enable "Emit WebP". Drop a JPEG. Verify both `.jpg` and `.webp` appear; `.jpg` overwrites, `.webp` is new.
5. Drop a folder with nested subfolders. Verify recursion.
6. Drop a file currently locked (open a `.jpg` in Preview, try). Verify ⚠ icon, no data loss.
7. Close app while batch running. Verify confirm dialog.
8. Run on macOS ARM + Intel + Windows 11 for a release.

### Not tested

- UI e2e (WebDriver/Playwright for Tauri) — CI setup cost not worth it at this scale. Relies on TypeScript unit tests for pure state logic + manual smoke.
- Image quality (SSIM). We trust Sharp; we're not re-testing it.

## Dependencies

### New runtime

**Frontend (`desktop/package.json`):**
- `vite` — dev server + bundler for the webview assets
- `typescript` — typed state / protocol definitions
- No UI framework for v1 (vanilla DOM) unless the state complexity demands it post-v1

**Rust (`src-tauri/Cargo.toml`):**
- `tauri` — core framework
- `tauri-plugin-shell` — sidecar process spawn
- `tauri-plugin-store` — settings persistence
- `trash` — cross-platform Trash
- `serde`, `serde_json` — protocol serialisation
- `uuid` — batch + file IDs
- `tokio` — async runtime (Tauri requires it)

**Shared Node:**
- `sharp` — already in CLI's `package.json`, no new dep

### Build-time

- Tauri CLI (`@tauri-apps/cli`) — `npm install -D` in `desktop/`
- Node-portable binary bundling script — packs `node` runtime per target triple into `src-tauri/binaries/` before `tauri build`. Platform matrix: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`.

## Distribution

- **Dev:** `cd desktop && npm run tauri dev` — live reload of webview, rust code hot on save.
- **Release:** `npm run tauri build` — produces `.dmg` (macOS), `.msi` (Windows), `.deb` / `.AppImage` (Linux).
- **Signing / notarisation:** out of scope for v1 — user will dismiss Gatekeeper warnings. Post-v1 consideration.

## Future Extensions

- AVIF support (one row in `ENCODERS` + one slider/checkbox)
- Menu bar mode: drag to menu bar icon, compresses silently
- Per-file quality override (right-click → "Recompress at 90")
- Auto-update via Tauri's built-in updater
- Quick Look plugin for size preview
- Drag output out of app (drag a compressed file to Finder / another app)
