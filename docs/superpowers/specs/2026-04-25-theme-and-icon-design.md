---
title: Hando Desktop — Theme System & App Icon
date: 2026-04-25
status: approved
---

# Hando Desktop — Theme System & App Icon

發布前兩項 polish：深/淺色主題切換系統，以及替換 Tauri 預設佔位圖的 App Icon pipeline。

---

## 1. 主題切換系統 (Theme System)

### 行為規格

三態偏好，儲存於 `tauri-plugin-store`，和其他 Settings 共用 `settings.json`：

| 值 | 行為 |
|---|---|
| `'auto'` | 跟隨 OS `prefers-color-scheme`，即時響應系統切換 |
| `'light'` | 強制亮色，忽略系統偏好 |
| `'dark'` | 強制深色，忽略系統偏好 |

- 預設值：`'auto'`
- 切換入口：**僅 Settings 面板**（toolbar 不加快切按鈕）
- UI 元件：Settings 面板頂部加 segmented control（Auto / Light / Dark）

### CSS 策略

不用 `@media (prefers-color-scheme)`，改讓 JS 全程主導：

```css
/* :root 放亮色預設 */
:root { … }

/* data-theme="dark" 覆蓋 */
:root[data-theme="dark"] { … }
```

JS 解析偏好 → 寫 `document.documentElement.dataset.theme`。
Auto 模式額外監聽 `matchMedia('(prefers-color-scheme: dark)').addEventListener('change', …)` 即時跟隨。

優點：三態都走同一路徑，不需要 `@media` 條件混搭。

### 調色盤

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
```

色彩語意：
- `--accent`：互動元件、working 狀態、dropzone hover border
- `--accent-soft`：working row 背景
- `--good`：savings 數字、完成狀態 icon（亮色 #7ac404 / 深色 #ceff04）
- `--err`：error 狀態 icon 與文字
- `--warn`：留給未來警告訊息使用，目前僅作變數預留

### 實作模組

**新增 `desktop/src/ui/theme.ts`**

```ts
export type ThemePref = 'auto' | 'light' | 'dark';

export function initTheme(): void
// 從 store 讀 pref → resolve → 套用 data-theme，並設定 matchMedia 監聽器

export function setThemePref(pref: ThemePref): void
// 存 store + 重新 resolve + 套用
```

`initTheme()` 在 `main.ts` 的 `await loadSettings()` 之後、任何 `mount*()` 之前呼叫，避免首次渲染閃爍 (FOUC)。

**修改 `desktop/src/ui/settings.ts`**
- `Settings` interface 加 `theme: ThemePref`，預設 `'auto'`
- Settings 面板頂部加 segmented control：`Auto | Light | Dark`，`onchange` 呼叫 `setThemePref()`

**修改 `desktop/src/style.css`**
- 把 `:root` 更新為新調色盤
- 加入 `:root[data-theme="dark"] { … }` 區塊
- 把寫死的 `rgba(59, 130, 246, 0.08)` 換成 `var(--accent-soft)`
- 確認所有元件（toolbar、dropzone、file-list、settings-panel、statusbar、settings-overlay 遮罩）在兩個主題下都正常

**刪除 `desktop/src/styles.css`**
- Vite 範本殘留，`index.html` 未載入，實際無效，刪除避免混淆

### 測試清單

- [ ] Auto：OS 切換深/淺色時，app 即時跟隨（不需重啟）
- [ ] Light / Dark：手動切換後，重啟 app 設定持久
- [ ] 所有 UI 區域在兩種主題下：toolbar、dropzone、file-list rows（working / done / error / skipped）、settings panel、statusbar
- [ ] FOUC：冷啟動不閃白
- [ ] `styles.css` 已移除，`index.html` 無殘留參考

---

## 2. App Icon Pipeline

### 現況

`desktop/src-tauri/icons/` 全為 Tauri 預設佔位圖，需替換為 Hando 品牌識別。

### Logo 來源

`hando-logo-100.svg` — 斜體字母造型，兩個路徑（根路徑儲存，不進 `src-tauri/`）。

Icon 設計由開發者自行完成，本 spec 只規定：
- 配色使用品牌色（主色 `#5e00ff` 或雙色方案）
- 幾何扁平風格，圓角，2–3 色
- 邊緣保留 ~10% 安全距離（macOS 自動裁圓角）
- 於 16 / 32 / 128 / 512px 四個尺寸驗證可辨識性
- 深色底與淺色底均可辨識

### Icon 生成流程

```
1. 設計完成 → 匯出 1024×1024 透明底 PNG
   存到：desktop/src-tauri/icons/source.png

2. 執行（一次性，每次更新 icon 時重跑）：
   cd desktop && npx tauri icon src-tauri/icons/source.png

3. 自動覆寫所有所需格式：
   icon.ico  icon.icns  icon.png
   32x32.png  128x128.png  128x128@2x.png
   Square30x30Logo.png … Square310x310Logo.png
   StoreLogo.png
```

`source.png` 進 git（保留 master artwork，< 1MB，`tauri icon` 的唯一輸入）。

### 驗證清單

- [ ] `source.png` 已 commit 至 `desktop/src-tauri/icons/source.png`
- [ ] `npx tauri icon` 執行成功，無報錯
- [ ] Windows build：exe 右鍵 → 內容 → 看到新 icon（非 Tauri 預設）
- [ ] Windows taskbar / 視窗左上角顯示新 icon
- [ ] macOS（若有測試機）：Dock icon 顯示正確

---

## 實作順序建議

1. **主題 CSS 先行**：更新 `style.css`、刪 `styles.css`，在瀏覽器開 `index.html` 確認亮/深色顯示
2. **`theme.ts` + Settings UI**：接上三態切換邏輯
3. **整合測試**：跑 `npm run tauri dev`，手動驗證主題切換全流程
4. **Icon（設計完成後獨立執行）**：`source.png` 就位 → `npx tauri icon` → rebuild 驗證

主題系統與 icon 互相獨立，可分批 ship。
