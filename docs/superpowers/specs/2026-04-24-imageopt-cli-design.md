# ImageOpt CLI — 設計文件

**日期**: 2026-04-24
**狀態**: Draft

## 目的

提供一個簡單的 CLI 工具,遞迴掃描輸入資料夾內的點陣圖,使用 Sharp 進行壓縮,並額外輸出對應的 WebP 版本,方便在網頁中以 `<picture>` 搭配 fallback 使用。

## 範疇

**In scope**
- CLI 進入點 `imageopt`
- 遞迴輸入資料夾,保留子資料夾結構輸出
- 對 `.jpg` / `.jpeg` / `.png` / `.webp` 四種輸入格式,輸出「壓縮後原格式」+「對應 WebP」
- 以 mtime 比對進行增量建置(輸出檔比原檔新就跳過)
- 固定預設品質 75,常數集中在檔案頂端方便日後調整

**Out of scope**
- CLI flag 設定品質(品質調整以修改原始碼常數為主)
- GIF / SVG / AVIF / HEIC 等其他格式
- 圖片尺寸縮放 / resize
- 自動上傳 / 圖床整合
- Watch 模式

## 使用方式

```bash
imageopt <input-dir> -o <output-dir>
```

範例:

```bash
imageopt ./src/images -o ./dist/images
```

預期輸出對應(`./src/images/banner/hero.jpg` → 輸出兩個檔案):

```
./dist/images/banner/hero.jpg   # 壓縮後的原格式
./dist/images/banner/hero.webp  # 對應的 WebP
```

對於原本就是 `.webp` 的輸入,只輸出一份壓縮過的 `.webp`(不重複產生)。

## 核心行為

1. 解析 CLI 參數,取得 `<input-dir>` 與 `-o <output-dir>`。驗證輸入資料夾存在、輸出參數有提供。
2. 遞迴收集輸入資料夾下所有副檔名屬於支援清單的檔案。
3. 對每個檔案:
   a. 計算對應的輸出路徑(保留相對路徑),包含「原格式輸出」與「WebP 輸出」兩個目標。
   b. 對每個目標,比較原檔 mtime 與輸出檔 mtime;若輸出存在且 mtime ≥ 原檔 mtime,跳過該目標。
   c. 其餘目標以 Sharp 讀入來源、套用對應編碼器壓縮,寫入輸出(必要時建立中介資料夾)。
4. 以固定的併發數執行多個檔案的處理(見 CONFIG.CONCURRENCY)。
5. 單一檔案處理失敗時印出警告,但不中斷整體流程;紀錄失敗計數。
6. 全部完成後印出總結:已處理、已跳過、失敗、總耗時、節省總位元組數。

## 設定常數

集中在 `index.js` 頂端:

```js
const CONFIG = {
  JPEG_QUALITY: 75,
  PNG_QUALITY: 75,   // 透過 sharp 的 png 壓縮設定近似
  WEBP_QUALITY: 75,
  EXTENSIONS: ['.jpg', '.jpeg', '.png', '.webp'],
  CONCURRENCY: 4,
};
```

日後要調整壓縮強度只需改此區塊。

## 專案結構

```
ImageOpt/
├── package.json       # 宣告 bin、依賴 sharp
├── index.js           # CLI 進入點(含 #!/usr/bin/env node)
├── .gitignore         # node_modules 等
└── README.md          # 使用說明
```

單一 `index.js` 包含所有邏輯;範疇不大,不先拆檔。

## 相依套件

- `sharp` — 影像處理與編碼
- Node.js 內建:`fs` / `fs/promises`、`path`、`process`

不使用 commander / yargs;CLI 參數自行解析即可(位置參數 `<input-dir>`、旗標 `-o <output-dir>`)。

## CLI 安裝 / 分發

- `package.json` 宣告:
  ```json
  {
    "bin": { "imageopt": "./index.js" }
  }
  ```
- 開發時:`npm link` 一次即可全域使用 `imageopt`。
- 未來分享:他人 clone 後執行 `npm install && npm link`,或作者 `npm publish` 後以 `npx imageopt` 使用。

## 錯誤處理

| 情境                             | 行為                                   |
| -------------------------------- | -------------------------------------- |
| 未提供 `<input-dir>` 或 `-o`     | 印出 usage,`exit 1`                   |
| `<input-dir>` 不存在或非資料夾   | 印錯誤訊息,`exit 1`                   |
| 輸出資料夾不存在                 | 建立(遞迴 `mkdir -p`)                |
| 單一檔案讀取/解碼/編碼失敗       | 印警告(檔名 + 原因),失敗計數 +1,繼續 |
| sharp 本身載入失敗               | 以未捕捉錯誤終止(安裝問題,交給使用者) |

## 輸出與 Log

- 單檔處理:不逐檔印訊息,避免 log 過長(可於未來加 `--verbose`)。
- 結束時印出總結,例如:
  ```
  Processed: 42   Skipped: 8   Failed: 0
  Time: 3.21s     Saved: 1.3 MB
  ```

## 測試策略

由於專案範疇小且以 I/O 與二進位處理為主,採用輕量手動驗收:

1. 準備 `fixtures/` 內含不同格式與大小的測試圖。
2. 執行一次,檢查輸出目錄結構與檔案是否齊全。
3. 再執行一次,確認有跳過(透過總結中的 Skipped 計數)。
4. 修改一張輸入圖的 mtime,再執行一次,確認該圖被重壓。

若未來邏輯複雜化,再考慮引入單元測試(例如對 mtime 比對邏輯做純函式抽離並以 Node test runner 驗證)。

## 未來可能擴充(非本次範疇)

- `--verbose` / `--quiet` flag
- `--concurrency <n>` flag
- AVIF 輸出
- 尺寸 resize(`--max-width`)
- Watch 模式
