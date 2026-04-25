# Hando Desktop — Multi-language (i18n) Support

**Date:** 2026-04-25
**Scope:** Desktop app (`desktop/`) only. CLI is not affected.

## Goal

Add user-selectable multi-language support to the Hando desktop app, with a language picker in the existing Settings panel, system-locale auto-detection on first launch, and live in-place re-rendering when the language changes (no app reload).

## Supported locales

Seven locales:

- `zh-TW` 繁體中文
- `zh-CN` 简体中文
- `en` English
- `ja` 日本語
- `ko` 한국어
- `es` Español
- `pt` Português

Plus a meta-choice `auto` which resolves to one of the seven via `navigator.language`.

## Non-goals

- Translating CLI output (`index.js` / `src/sidecar.js`) — out of scope.
- Translating Rust-side console/log messages — out of scope (developer-facing only).
- Pluralization rules per language — see "Plurals" below.
- RTL languages — none of the chosen locales are RTL.

## Architecture

### Module layout

```
desktop/src/i18n/
  index.ts              # init, t, setLocale, getLocale, onLocaleChange, SUPPORTED_LOCALES, types
  locales/
    en.ts               # source-of-truth Messages object + Messages type export
    zh-TW.ts            # const m: Messages = { ... }
    zh-CN.ts
    ja.ts
    ko.ts
    es.ts
    pt.ts
```

### Module API (`desktop/src/i18n/index.ts`)

```ts
export type LocaleCode = 'en' | 'zh-TW' | 'zh-CN' | 'ja' | 'ko' | 'es' | 'pt';
export type LanguageSetting = 'auto' | LocaleCode;

export const SUPPORTED_LOCALES: readonly LocaleCode[];

// Initialize at boot. If 'auto', resolve from navigator.language.
export function init(setting: LanguageSetting): void;

// Translate a key with optional placeholder vars. {name} → vars.name.
export function t(key: MessageKey, vars?: Record<string, string | number>): string;

// Switch locale. Resolves 'auto' the same way as init(). Fires onLocaleChange listeners.
export function setLocale(setting: LanguageSetting): void;

// Currently active resolved locale (never 'auto').
export function getLocale(): LocaleCode;

// Subscribe to locale changes. Returns unsubscribe.
export function onLocaleChange(cb: (locale: LocaleCode) => void): () => void;
```

`MessageKey` and `Messages` are derived from `en.ts`:

```ts
// en.ts
export const messages = {
  app: { title: 'Hando' },
  toolbar: { settings: '⚙ Settings', undo: '↺ Undo' },
  // ...
} as const;
export type Messages = typeof messages;

// index.ts
type Leaves<T, P extends string = ''> = { [K in keyof T]:
  T[K] extends string ? `${P}${K & string}` : Leaves<T[K], `${P}${K & string}.`>
}[keyof T];
export type MessageKey = Leaves<Messages>;
```

This ensures every other locale file is type-checked against the English shape; missing keys fail `tsc`.

### Auto-detection

The resolver lowercases `navigator.language` first, then prefix-matches:

| Lowercased input matches | Resolved |
|---|---|
| `zh-tw`, `zh-hant*`, `zh-hk`, `zh-mo` | `zh-TW` |
| `zh-cn`, `zh-hans*`, `zh-sg`, `zh` (bare) | `zh-CN` |
| `ja*` | `ja` |
| `ko*` | `ko` |
| `es*` | `es` |
| `pt*` (incl. `pt-br`, `pt-pt`) | `pt` |
| anything else | `en` |

### Locale-aware number formatting

The `i18n` module also exports a small formatter:

```ts
export function fmtBytes(n: number): string;   // "1,2 MB" in es/pt; "1.2 MB" in en
```

Implementation: `Intl.NumberFormat(getLocale(), { maximumFractionDigits: 1 })` for the numeric part, then concatenate ` B` / ` KB` / ` MB` (units are universal; not translated). Replaces the duplicated `fmtBytes` currently in `desktop/src/ui/statusbar.ts` and `desktop/src/ui/file-list.ts` — both call sites switch to the i18n version, which also re-renders correctly when locale changes (because both components already subscribe to `onLocaleChange`).

### Trust boundary for `t()`

Translation files are bundled at build time as trusted source code, not runtime user input. Therefore:

- `t()` returns raw strings (no HTML escaping by default), allowing `{link}` in `dropzone.prompt` to interpolate an HTML span.
- Translators must not put untrusted markup in locale files (they're committed code, reviewed by the developer).
- Any future need to interpolate **user-supplied data** (file names, paths) into a translated template must be escaped at the call site — not via `t()`.

### Reactivity caveat

`t()` must be called at **message-display time**, not at handler-registration time, so the current locale wins. The existing `confirm()` and `alert()` call sites in `main.ts` already follow this pattern (the call happens inside the event listener). Future contributors should preserve it.

### Settings integration

Extend `Settings` (`desktop/src/ui/settings.ts`):

```ts
export interface Settings {
  // ... existing fields ...
  language: LanguageSetting;   // 'auto' | LocaleCode
}

export const DEFAULT_SETTINGS: Settings = {
  // ... existing defaults ...
  language: 'auto',
};
```

Persisted via the existing `tauri-plugin-store` `settings.json`. No new store.

The Settings panel gains one row at the top:

```html
<label>Language</label>
<select id="lang">
  <option value="auto">Auto detect (system)</option>
  <option value="zh-TW">繁體中文</option>
  <option value="zh-CN">简体中文</option>
  <option value="en">English</option>
  <option value="ja">日本語</option>
  <option value="ko">한국어</option>
  <option value="es">Español</option>
  <option value="pt">Português</option>
</select>
```

Native option labels are intentionally left in their own language so users can find their language even if the UI is currently in one they don't read.

On change: write `language` to settings (`saveSettings`) and call `i18n.setLocale(newValue)`.

### Reactivity (live re-render)

`i18n.onLocaleChange(cb)` fires after every `setLocale()` call.

Component-level wiring:

- **`mountToolbar`** — refactored to expose `{ setUndoEnabled, refresh }`. `refresh()` re-renders the inner HTML (rebinds buttons) preserving the current `disabled` state of Undo.
- **`mountDropzone`** — refactored to expose `{ refresh }`. `refresh()` re-renders the prompt HTML and re-binds the click handler. The Tauri `getCurrentWindow().onDragDropEvent` listener is registered exactly once at mount time and is **not** re-registered on refresh.
- **`mountFileList`** and **`mountStatusBar`** — render functions read `t()` at render time. They subscribe to `onLocaleChange` and call `render(store.snapshot())` when fired.
- **`openSettingsPanel`** — already builds fresh DOM on every open, so it always reflects the current locale. No subscription needed.
- **`main.ts`** — wires the toolbar and dropzone refresh:
  ```ts
  i18n.onLocaleChange(() => { toolbar.refresh(); dropzone.refresh(); });
  ```

### Boot sequence (in `main.ts`)

```ts
await loadSettings();
i18n.init(getSettings().language);   // resolves 'auto' via navigator.language
// ... existing event subscriptions ...
const toolbar = mountToolbar(...);
const dropzone = await mountDropzone(...);
mountFileList(...);
mountStatusBar(...);
i18n.onLocaleChange(() => { toolbar.refresh(); dropzone.refresh(); });
```

## Message key inventory

22 keys total. Brand name "Hando" stays untranslated as a proper noun.

| Key | English source |
|---|---|
| `toolbar.settings` | ⚙ Settings |
| `toolbar.undo` | ↺ Undo |
| `dropzone.prompt` | Drag images here, or `{link}` |
| `dropzone.clickToAdd` | click to add |
| `dropzone.imagesFilter` | Images |
| `fileList.empty` | No files yet. Drag images onto the window. |
| `statusbar.progress` | `{completed} / {total}` files (`{pct}`%) |
| `statusbar.saved` | Saved `{amount}` across `{count}` files |
| `statusbar.trashHint` | Originals moved to Trash |
| `statusbar.trashShow` | Show |
| `settings.title` | Settings |
| `settings.language` | Language |
| `settings.languageAuto` | Auto detect (system) |
| `settings.jpegQuality` | JPEG quality |
| `settings.pngQuality` | PNG quality |
| `settings.webpQuality` | WebP quality |
| `settings.avifQuality` | AVIF quality |
| `settings.emitWebp` | Also emit WebP alongside |
| `settings.emitAvif` | Also emit AVIF alongside |
| `settings.moveToTrash` | Move originals to Trash |
| `settings.done` | Done |
| `confirm.quitProcessing` | `{count}` files still processing. Quit anyway? |
| `alert.engineCrashed` | Image engine crashed. It will restart on the next drop. |

`dropzone.prompt` uses `{link}` so each translation can position the clickable span naturally within the sentence (Asian languages may want different word order).

## Plurals

No per-language plural rules. Phrasing is chosen to read naturally for any count:

- English: "Saved 1.2 MB across 5 files" — "files" reads OK even for count=1 in real usage (compression is always batch).
- Chinese / Japanese / Korean: counter-word style with no singular/plural distinction.
- Spanish / Portuguese: phrasing avoids needing inflection where possible (e.g., use "archivos" / "ficheiros" — the count being plural is the typical case).

If a future need for true plurals arises, this can be revisited.

## Translation source

Initial translations will be drafted in implementation by the developer (with care for Asian-language conventions where the existing UI already mixes "張"). They are intended to be reviewable / replaceable by a fluent speaker after landing.

Platform-convention notes for the `statusbar.trashHint` string (Windows is the primary build target per `CLAUDE.md`):

- `zh-TW` → 原始檔已移至資源回收筒
- `zh-CN` → 原始文件已移至回收站
- `ja` → 元ファイルをごみ箱に移動しました
- `ko` → 원본 파일을 휴지통으로 이동했습니다

`settings.languageAuto` for `zh-TW` will be "跟隨系統" (more concise than literal "自動偵測 (系統語系)" given the constrained Settings panel width).

## Out-of-scope but worth noting

- HTML `<title>Hando</title>` and `<html lang="en">` in `desktop/index.html` — these are fine as-is. The window title is the brand name; the `lang` attribute is not user-visible.
- Tauri menu bar / native menus — there are none currently.
- Notification text from Rust — none currently.

## Testing

The desktop frontend has no test infrastructure today. Adding TS test tooling is out of scope. Verification will be manual:

1. Boot app with system locales: zh-TW, zh-CN, en, ja, ko, es, pt — confirm correct default.
2. Boot app with an unmapped locale (e.g., `de-DE`) — confirm fallback to English.
3. Switch language in Settings and confirm toolbar, dropzone, file-list (with files present), and statusbar (with progress and saved-bytes states) all update without reload.
4. Confirm choice persists across app restart.

## Risks / open issues

- **Translation quality**: initial translations may need polishing by native speakers post-merge. Acceptable for a first pass.
- **Long strings overflowing layout**: German/Russian aren't in scope; the chosen 7 languages all produce reasonably compact strings for the existing layout. No layout changes anticipated.
- **`navigator.language` reliability inside Tauri WebView**: Tauri 2's WebView (WebView2 on Windows, WKWebView on macOS) exposes `navigator.language` from the OS. Confirmed to work; no fallback needed beyond the English default.
