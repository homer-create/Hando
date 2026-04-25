# Theme System & App Icon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a three-state (Auto/Light/Dark) theme system to the Hando desktop app and document the icon replacement pipeline.

**Architecture:** CSS variables drive theming via a `[data-theme="dark"]` attribute on `<html>`; a new `theme.ts` module resolves preference → attribute and listens to system changes in Auto mode; preference is stored as a new `theme` field on the existing `Settings` object.

**Tech Stack:** Vanilla TypeScript, CSS custom properties, `window.matchMedia`, `tauri-plugin-store` (via existing `saveSettings`), Tauri 2.x WebView.

---

## File Map

| Action | Path | Purpose |
|---|---|---|
| Modify | `desktop/src/style.css` | New palette vars + dark override block + segmented-control CSS |
| Modify | `desktop/src/i18n/locales/en.ts` | Add `settings.theme*` keys to `Messages` interface |
| Modify | `desktop/src/i18n/locales/zh-TW.ts` | zh-TW translations for theme keys |
| Modify | `desktop/src/i18n/locales/zh-CN.ts` | zh-CN translations |
| Modify | `desktop/src/i18n/locales/ja.ts` | ja translations |
| Modify | `desktop/src/i18n/locales/ko.ts` | ko translations |
| Modify | `desktop/src/i18n/locales/es.ts` | es translations |
| Modify | `desktop/src/i18n/locales/pt.ts` | pt translations |
| Create | `desktop/src/ui/theme.ts` | `ThemePref` type, `initTheme()`, `setThemePref()` |
| Modify | `desktop/src/ui/settings.ts` | Add `theme: ThemePref` to `Settings`, add UI control |
| Modify | `desktop/src/main.ts` | Call `initTheme()` after `loadSettings()` |
| Delete | `desktop/src/styles.css` | Dead Vite boilerplate — never loaded |

---

## Task 1: Update CSS — new palette + dark mode + segmented-control styles

**Files:**
- Modify: `desktop/src/style.css`

- [ ] **Step 1: Replace `:root` block and add dark override + segmented-control styles**

Replace the entire contents of `desktop/src/style.css` with:

```css
:root {
  --bg:             #ffffff;
  --bg-elevated:    #f5f5f5;
  --text:           #0a0a0a;
  --text-secondary: #71717a;
  --border:         #e5e5e5;
  --accent:         #5e00ff;
  --accent-soft:    rgba(94, 0, 255, .08);
  --good:           #7ac404;
  --warn:           #f59e0b;
  --err:            #ff2e9b;
}
:root[data-theme="dark"] {
  --bg:             #09090b;
  --bg-elevated:    #18181b;
  --text:           #fafafa;
  --text-secondary: #a1a1aa;
  --border:         #27272a;
  --accent:         #6a38ff;
  --accent-soft:    rgba(106, 56, 255, .18);
  --good:           #ceff04;
  --warn:           #fbbf24;
  --err:            #ff2e9b;
}
* { box-sizing: border-box; }
body { margin: 0; font-family: -apple-system, system-ui, sans-serif; color: var(--text); background: var(--bg); }
#app { display: flex; flex-direction: column; height: 100vh; }
.toolbar { display: flex; justify-content: space-between; align-items: center; padding: 8px 14px; border-bottom: 1px solid var(--border); background: var(--bg-elevated); }
.toolbar .title { font-weight: 600; }
.toolbar .btn { margin-left: 6px; padding: 4px 10px; background: transparent; border: 1px solid var(--border); border-radius: 4px; cursor: pointer; font-size: 13px; color: var(--text); }
.toolbar .btn:disabled { opacity: 0.4; cursor: not-allowed; }
#dropzone { padding: 10px 14px; }
.dropzone { border: 1.5px dashed var(--border); border-radius: 6px; padding: 16px; text-align: center; color: var(--text-secondary); font-size: 13px; }
.dropzone.hover { border-color: var(--accent); color: var(--accent); }
.dropzone .link { color: var(--accent); text-decoration: underline; cursor: pointer; }
#list { flex: 1; overflow-y: auto; padding: 4px 14px 12px; }
.file-list .row { display: grid; grid-template-columns: 20px 1fr 80px 80px 60px; gap: 10px; padding: 8px 10px; border-radius: 4px; font-size: 13px; align-items: center; }
.file-list .row + .row { margin-top: 4px; }
.file-list .row.status-working { background: var(--accent-soft); }
.file-list .row.status-done .savings { color: var(--good); font-weight: 600; }
.file-list .row.status-error .icon { color: var(--err); }
.file-list .size.old { color: var(--text-secondary); text-decoration: line-through; }
.file-list .size.new { color: var(--good); }
.file-list .empty { text-align: center; color: var(--text-secondary); padding: 30px; font-size: 13px; }
#statusbar { padding: 8px 14px; border-top: 1px solid var(--border); background: var(--bg-elevated); font-size: 12px; color: var(--text-secondary); }
#statusbar { display: flex; justify-content: space-between; align-items: center; }
#statusbar .link { color: var(--accent); text-decoration: underline; cursor: pointer; }
#statusbar .good { color: var(--good); }
.progress-wrap { display: inline-block; width: 120px; height: 6px; background: var(--border); border-radius: 3px; vertical-align: middle; overflow: hidden; margin-right: 8px; }
.progress-bar { display: block; height: 100%; background: var(--accent); border-radius: 3px; transition: width 0.2s ease; }
.progress-label { vertical-align: middle; }
#settings-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 100; }
.settings-panel { background: var(--bg); border-radius: 8px; padding: 20px 24px; width: 360px; box-shadow: 0 10px 40px rgba(0,0,0,0.3); border: 1px solid var(--border); }
.settings-panel h2 { margin: 0 0 14px; font-size: 16px; color: var(--text); }
.settings-panel label { display: block; margin-top: 12px; font-size: 13px; color: var(--text); }
.settings-panel input[type="range"] { width: 100%; margin-top: 4px; accent-color: var(--accent); }
.settings-panel input[type="checkbox"] { margin-right: 6px; accent-color: var(--accent); }
.settings-panel select { width: 100%; margin-top: 4px; padding: 4px 6px; border: 1px solid var(--border); border-radius: 4px; background: var(--bg-elevated); color: var(--text); font-size: 13px; }
.settings-actions { margin-top: 16px; text-align: right; }
.settings-actions .btn { padding: 5px 14px; background: var(--accent); color: #fff; border: none; border-radius: 4px; cursor: pointer; font-size: 13px; }
.theme-seg { display: flex; border: 1px solid var(--border); border-radius: 5px; overflow: hidden; margin-top: 6px; }
.seg-btn { flex: 1; padding: 5px 8px; background: transparent; border: none; border-right: 1px solid var(--border); cursor: pointer; font-size: 12px; color: var(--text-secondary); }
.seg-btn:last-child { border-right: none; }
.seg-btn.active { background: var(--accent); color: #fff; font-weight: 600; }
```

- [ ] **Step 2: Open `desktop/index.html` in a browser (file://) and verify light-mode appearance looks correct** (toolbar, dropzone, statusbar styled)

- [ ] **Step 3: Commit**

```bash
git add desktop/src/style.css
git commit -m "style: new brand palette + dark mode CSS vars + segmented-control styles"
```

---

## Task 2: Add theme i18n keys to all locales

**Files:**
- Modify: `desktop/src/i18n/locales/en.ts`
- Modify: `desktop/src/i18n/locales/zh-TW.ts`
- Modify: `desktop/src/i18n/locales/zh-CN.ts`
- Modify: `desktop/src/i18n/locales/ja.ts`
- Modify: `desktop/src/i18n/locales/ko.ts`
- Modify: `desktop/src/i18n/locales/es.ts`
- Modify: `desktop/src/i18n/locales/pt.ts`

- [ ] **Step 1: Update `Messages` interface + en.ts**

In `desktop/src/i18n/locales/en.ts`, add four keys to the `settings` section of the interface AND the messages object:

Interface change — add to the `settings:` block:
```ts
settings: {
  // ... existing keys ...
  theme: string;
  themeAuto: string;
  themeLight: string;
  themeDark: string;
};
```

Messages object change — add to `settings:`:
```ts
settings: {
  // ... existing keys ...
  theme: 'Theme',
  themeAuto: 'Auto',
  themeLight: 'Light',
  themeDark: 'Dark',
},
```

- [ ] **Step 2: Update `zh-TW.ts`** — add to `settings:`:
```ts
theme: '外觀',
themeAuto: '自動',
themeLight: '亮色',
themeDark: '深色',
```

- [ ] **Step 3: Update `zh-CN.ts`** — add to `settings:`:
```ts
theme: '外观',
themeAuto: '自动',
themeLight: '浅色',
themeDark: '深色',
```

- [ ] **Step 4: Update `ja.ts`** — add to `settings:`:
```ts
theme: '外観',
themeAuto: '自動',
themeLight: 'ライト',
themeDark: 'ダーク',
```

- [ ] **Step 5: Update `ko.ts`** — add to `settings:`:
```ts
theme: '테마',
themeAuto: '자동',
themeLight: '라이트',
themeDark: '다크',
```

- [ ] **Step 6: Update `es.ts`** — add to `settings:`:
```ts
theme: 'Tema',
themeAuto: 'Auto',
themeLight: 'Claro',
themeDark: 'Oscuro',
```

- [ ] **Step 7: Update `pt.ts`** — add to `settings:`:
```ts
theme: 'Tema',
themeAuto: 'Auto',
themeLight: 'Claro',
themeDark: 'Escuro',
```

- [ ] **Step 8: Type-check — run from `desktop/`**

```bash
cd desktop && npx tsc --noEmit
```

Expected: no errors. If you see `Property 'theme' is missing in type`, a locale file was missed — fix it.

- [ ] **Step 9: Commit**

```bash
git add desktop/src/i18n/
git commit -m "i18n: add theme preference labels to all locales"
```

---

## Task 3: Create `theme.ts`

**Files:**
- Create: `desktop/src/ui/theme.ts`

- [ ] **Step 1: Create the file**

```ts
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { getSettings, saveSettings } from './settings';

export type ThemePref = 'auto' | 'light' | 'dark';

const mq = window.matchMedia('(prefers-color-scheme: dark)');
let mediaListener: ((e: MediaQueryListEvent) => void) | null = null;

function applyResolved(pref: ThemePref): void {
  if (mediaListener) {
    mq.removeEventListener('change', mediaListener);
    mediaListener = null;
  }
  const dark = pref === 'dark' || (pref === 'auto' && mq.matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  if (pref === 'auto') {
    mediaListener = (e) => {
      document.documentElement.dataset.theme = e.matches ? 'dark' : 'light';
    };
    mq.addEventListener('change', mediaListener);
  }
}

export function initTheme(): void {
  applyResolved(getSettings().theme ?? 'auto');
}

export function setThemePref(pref: ThemePref): void {
  applyResolved(pref);
  const s = getSettings();
  s.theme = pref;
  saveSettings(s);
}
```

- [ ] **Step 2: Type-check**

```bash
cd desktop && npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add desktop/src/ui/theme.ts
git commit -m "feat: add theme.ts — three-state theme resolution and data-theme management"
```

---

## Task 4: Wire theme into Settings UI + main.ts

**Files:**
- Modify: `desktop/src/ui/settings.ts`
- Modify: `desktop/src/main.ts`

- [ ] **Step 1: Add `theme` to `Settings` interface and `DEFAULT_SETTINGS`**

In `desktop/src/ui/settings.ts`:

Add import at top (after existing imports):
```ts
import type { ThemePref } from './theme';
```

Add `theme` to the `Settings` interface:
```ts
export interface Settings {
  // ... existing fields ...
  theme: ThemePref;
}
```

Add `theme` to `DEFAULT_SETTINGS`:
```ts
export const DEFAULT_SETTINGS: Settings = {
  // ... existing fields ...
  theme: 'auto',
};
```

- [ ] **Step 2: Add segmented control to settings panel HTML**

In `openSettingsPanel()`, inside the `overlay.innerHTML` template string, add the theme control as the **first item after `<h2>`** (before the language `<label>`):

```ts
overlay.innerHTML = `
  <div class="settings-panel">
    <h2>${t('settings.title')}</h2>
    <label>${t('settings.theme')}</label>
    <div class="theme-seg" id="theme-seg">
      <button class="seg-btn${current.theme === 'auto' ? ' active' : ''}" data-value="auto">${t('settings.themeAuto')}</button>
      <button class="seg-btn${current.theme === 'light' ? ' active' : ''}" data-value="light">${t('settings.themeLight')}</button>
      <button class="seg-btn${current.theme === 'dark' ? ' active' : ''}" data-value="dark">${t('settings.themeDark')}</button>
    </div>
    ... rest unchanged ...
  </div>`;
```

- [ ] **Step 3: Wire up the segmented control click handler**

Add the following after the other event bindings (before the closing brace of `openSettingsPanel`):

```ts
import { setThemePref, type ThemePref } from './theme';
```

Add at the end of `openSettingsPanel()` before the final `}`:
```ts
(overlay.querySelector('#theme-seg') as HTMLElement).addEventListener('click', (e) => {
  const btn = (e.target as HTMLElement).closest('[data-value]') as HTMLElement | null;
  if (!btn) return;
  const pref = btn.dataset.value as ThemePref;
  setThemePref(pref);
  overlay.querySelectorAll('.seg-btn').forEach((b) => b.classList.toggle('active', b === btn));
});
```

- [ ] **Step 4: Call `initTheme()` in `main.ts`**

In `desktop/src/main.ts`, add the import:
```ts
import { initTheme } from './ui/theme';
```

In the `main()` function, add `initTheme()` immediately after `i18n.init(...)`:
```ts
async function main() {
  await loadSettings();
  i18n.init(getSettings().language);
  initTheme();                          // ← add this line
  // ... rest unchanged ...
}
```

- [ ] **Step 5: Type-check**

```bash
cd desktop && npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 6: Run the app and verify manually**

```bash
cd desktop && npm run tauri dev
```

Verify:
- Settings panel shows "Theme" with Auto / Light / Dark buttons at the top
- Clicking "Dark" switches the app to dark mode immediately
- Clicking "Light" switches back
- Clicking "Auto" follows OS — change OS theme while app is open and verify it responds without restart
- Close and reopen app — selected theme is remembered

- [ ] **Step 7: Commit**

```bash
git add desktop/src/ui/settings.ts desktop/src/main.ts
git commit -m "feat: wire theme preference into Settings UI and boot sequence"
```

---

## Task 5: Delete `styles.css` + final type check

**Files:**
- Delete: `desktop/src/styles.css`

- [ ] **Step 1: Delete the file**

```bash
rm desktop/src/styles.css
```

- [ ] **Step 2: Confirm `index.html` has no reference to `styles.css`**

```bash
grep -n "styles.css" desktop/index.html
```

Expected: no output. If found, remove the `<link>` tag.

- [ ] **Step 3: Final type check**

```bash
cd desktop && npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 4: Smoke-test the app one more time**

```bash
cd desktop && npm run tauri dev
```

Confirm all UI looks correct in both Light and Dark. Check:
- toolbar buttons visible and styled
- dropzone border and text
- file-list rows: working (accent-soft bg), done (good-coloured savings), error (err-coloured icon)
- settings panel: correct background, visible labels, segmented control, sliders, checkboxes
- statusbar: correct background

- [ ] **Step 5: Commit**

```bash
git add -A desktop/src/styles.css
git commit -m "chore: remove dead styles.css (Vite boilerplate, never loaded)"
```

---

## Task 6: Icon Pipeline (manual — no code)

This task is executed once the icon design is ready. There is no code to write.

**Prerequisites:** 
- `hando-logo-100.svg` exists at repo root (already placed)
- Design the icon in your tool of choice using the brand brief in `docs/superpowers/specs/2026-04-25-theme-and-icon-design.md`

- [ ] **Step 1: Export `source.png`**

Export the finished design as a **1024 × 1024 px PNG with transparent background** and save to:
```
desktop/src-tauri/icons/source.png
```

- [ ] **Step 2: Generate all icon sizes**

```bash
cd desktop && npx tauri icon src-tauri/icons/source.png
```

Expected output: lists generated files. No errors.

- [ ] **Step 3: Verify generated files exist and are non-default**

```bash
ls desktop/src-tauri/icons/
```

Confirm `icon.ico`, `icon.icns`, `icon.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, and all `Square*Logo.png` files are present and have a recent modification time.

- [ ] **Step 4: Rebuild and verify**

```bash
cd desktop && npm run tauri build
```

On Windows: right-click the compiled `.exe` → Properties → look for the new icon (not the default Tauri "T" icon).

Check that the taskbar and window title bar also show the new icon when running.

- [ ] **Step 5: Commit**

```bash
git add desktop/src-tauri/icons/
git commit -m "feat: replace placeholder Tauri icons with Hando brand icon"
```

---

## Self-Review

**Spec coverage:**
- ✓ Three-state Auto/Light/Dark — Task 3 (`theme.ts`) + Task 4 (Settings UI)
- ✓ Settings-only toggle — Task 4 (segmented control in panel, no toolbar button)
- ✓ Persisted via tauri-plugin-store — Task 4 (`setThemePref` calls `saveSettings`)
- ✓ JS-driven `[data-theme]` strategy — Task 3 (`applyResolved`)
- ✓ matchMedia listener for Auto — Task 3 (`mediaListener` setup)
- ✓ Full palette (light + dark) — Task 1 (CSS vars)
- ✓ `--accent-soft` replaces hardcoded rgba — Task 1 (`.status-working`)
- ✓ `styles.css` deleted — Task 5
- ✓ FOUC prevention — Task 4 (`initTheme()` before any `mount*()`)
- ✓ Icon pipeline — Task 6
- ✓ i18n keys for theme UI — Task 2

**No placeholders:** All steps contain actual code or commands.

**Type consistency:**
- `ThemePref` defined in `theme.ts`, imported as `import type { ThemePref }` in `settings.ts` — consistent
- `getSettings().theme` used in `initTheme()` — `theme` added to `Settings` interface in Task 4 Step 1 — consistent
- `setThemePref` imported in `settings.ts` in Task 4 Step 3 — exported from `theme.ts` in Task 3 — consistent
