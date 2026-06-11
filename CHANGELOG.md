# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

### Changed
- **2MP+ 大圖的 auto 模式旋鈕守門** — AVIF 搜尋速度起手抬到 max(使用者值, 9)：實測時間砍半（realphoto avif@85 3063ms→1579ms）、體積只多 ~6%；PNG 候選的 oxipng 壓到 min(使用者值, 2)：level 4 在 2MP 照片級 PNG 單發 3.7s，乘上搜尋發數會把單張時間撐爆。manual 模式不受影響（單發、使用者明示旋鈕）
- **超高 bpp JPEG（≥8 bpp）的 auto 模式直走無損轉碼** — 顆粒照片（如 24MP、bpp≈10 的 large_photo）有損候選全滅：q75→q85 分數只 5.27→16.10（ssimulacra2 重罰顆粒被抹掉），每發 encode+judge 還要 3–8s；現在 bpp ≥ 8 跳過整輪有損搜尋直接 DCT 轉碼，單張省下數十秒白燒，相機直出 JPEG（2–5 bpp）不受影響
- **S 值人眼定案** — homer 以 `docs/calibration/` 階梯樣本並排複核，可見差異約從 s ≈ 70–75 開始，三檔 preset（視覺無損 90 / 平衡 80 / 激進 70）維持原值定案；`PRESET_TARGETS` 不變，`docs/calibration.md` 與 `docs/rubric.md` 狀態更新。實拍照片複核仍列待辦（合成 fixtures 偏好壓，重點是激進檔 70 是否要抬）

### Fixed
- **ICC 來源的 AVIF 重壓省幅 <10% 改判 skip** — ravif 只支援 nclx，AVIF 輸出必丟 ICC profile；with_icc.avif 實測贏家過 90 門檻卻只省 2.8%，為這點省幅默默丟色彩描述檔等於資料損失（攝影師等級的偏色風險）；現在 main 輸出是 AVIF 且來源帶 ICC 時，省幅門檻（`keep_bar`）從 2% 抬到 10%，不到就 SkippedNoGain 保留原檔
- **ICC profile passthrough（重壓路徑）** — decode 原本直接丟棄 ICC（`decode.rs` `let icc = None`），廣色域來源（如 iPhone 的 Display P3）重壓後會偏色；新增 `encoder/icc.rs`，decode 端抽出（JPEG APP2 / PNG iCCP / WebP ICCP / AVIF colr-prof box），encode 端重嵌（JPEG APP2 / PNG iCCP / WebP 升級為 VP8X+ICCP container），含 ICC fixtures 與各格式 roundtrip 測試。已知限制：AVIF **輸出**端 ravif/avif-serialize 只支援 nclx，無法嵌 ICC，AVIF 輸出（含 companion）仍會丟 profile（rubric §0.5 已記）
- **JPEG ICC 寫入避開 mozjpeg crate 的 0-based bug** — mozjpeg 0.10.13 的 `write_icc_profile` 把 APP2 分段編號寫成 0-based（ICC 規範是 1-based），合規讀取器（如 libjpeg-turbo 的 `jpeg_read_icc_profile`）會拒讀；改用自家 `icc::jpeg_app2_segments`（1-based）+ `write_marker`，讀取端則同時容忍 0-based 檔案
- **JPEG progressive 開關原本是 no-op** — mozjpeg 預設就是 progressive，舊程式的 `set_progressive_mode()` 呼叫並沒有改變任何行為；現在 `jpegProgressive=false` 走 `set_fastest_defaults()`（baseline profile），檔案大 ~3 倍但編碼快 2–9 倍，開關才有真實意義
- **SkippedNoGain 殘留暫存檔** — encode 在「省 <2% 即跳過」時沒刪掉已寫出的 tmp 檔，長期使用會在暫存目錄累積垃圾；現在 skip 前先清掉
- **AVIF input could not be decoded** — the `image` crate's `avif` feature is encode-only, so dropping an `.avif` file failed at runtime with "format not supported"; AVIF decode now goes through `avif-decode` (bundled libaom, no system dependency), with 8/16-bit and gray/alpha variants normalized to RGBA8

### Added
- **三 preset 發版前 smoke test** — `auto_mode_presets_smoke`（`#[ignore]`，發版前 `cargo test -- --ignored` 跑）：S=70/80/90 三檔對 A 類 / B1 / B2 / ICC 代表 fixtures 跑 auto 搜尋，結果只能是「過 effective gate 的 Encoded」或 skip；補上先前只驗過 S=80 的缺口
- **bench `eval` / `corpus` 子命令（給 /goal loop 的 verifier 接口）** — `eval <input> <out_ext> <quality> [knob]` 對單一候選吐一行 JSON（src/out bytes、ratio、ssimulacra2、lossless、encode_ms、bpp、dims），decode/encode 失敗或不可能請求（如透明來源輸出 JPEG）回 `"ok":false` 並 exit 1，符合 rubric「一行指令 → 一個數字/exit code」鐵律；`corpus <dir>` 逐張列出 §1 input-gate 訊號（format/bpp/jpeg-blockiness/class_hint A·B），讓 orchestrator 在搜尋前先分流。原本 bench 只有固定網格批次（sweep/grid），無法逐候選驅動
- **偽裝有損偵測（rubric §1 第二道防線）** — `judge.rs::jpeg_blockiness`：對解碼像素量 JPEG 8×8 格線指紋（只統計小振幅亮度梯度的 8 相位分布，內容邊不干擾、耐裁切，零依賴純 Rust）；自動模式下 PNG／無損 WebP 超過門檻 1.25 即視為「JPEG 另存的偽裝無損」，畫質門檻抬到 B 類的 `max(S, 90)`，廠商提供的二手圖不再被當乾淨原圖重壓。fixtures 實測：乾淨來源 ≤ 1.14、JPEG 史（q60–q92）≥ 1.39；已知極限：q≈95+ 相機級 JPEG 另存測不到（但此類來源近乎乾淨，照 A 類處理損失極小）
- **裁判（judge）模組** — `encoder/judge.rs`：ssimulacra2 感知畫質評分（基準圖 vs 候選）、無損逐像素比對 `pixels_identical`、輸入閘用的 `bits_per_pixel`；rubric §3 的硬門檻從此可量測
- **bench harness** — `examples/bench.rs`：對 `tests/fixtures/` 整批跑 候選（格式 × 品質階梯），輸出 rubric 三數字 size / ssimulacra2 / encode-time 的 markdown 表；`calibrate` 子命令產出 §8.2 人眼校準用的品質階梯樣本（檔名含分數）
- **S 值校準文件** — `docs/calibration.md`：記錄量測階梯、暫定 preset 值（視覺無損 90 / 平衡 80 / 激進 70）與人眼定案步驟；樣本目錄 `docs/calibration/` 進 gitignore
- **編碼器旋鈕外露** — `EncodeOpts` 新增 `avifSpeed`（1–10）、`pngOxipngLevel`（0–6）、`webpMethod`（0–6）、`jpegProgressive`，全部帶 serde default 向後相容；WebP 改走 `encode_advanced`（lossless 模式設 `exact=1`，透明像素下的 RGB 不再被改寫，rubric §2 像素相同門檻才守得住）
- **參數網格與預設值調整** — `bench grid` 子命令掃 旋鈕組合；依數據把 oxipng 預設 2 → 4（再省 8–28%、時間仍 <2.5s/張），AVIF speed 8 與 WebP method 4 經數據確認保留；詳見 `docs/bench-results.md`
- **自動品質模式（quality-targeted encoding）** — `encoder/auto.rs`：對每張圖在步進 4 的品質格上二分搜尋「過 ssimulacra2 目標 S 的最小品質」（~5 輪收斂），無損與有損候選競爭取最小（rubric §5）；B 類輸入依 bpp 分級：高 bpp（相機 JPEG）用 preset S 重壓、低 bpp 的 JPEG 只走無損轉碼、無轉碼可走的格式門檻抬到 max(S, 90) 綁住世代損失（rubric §4）。`EncodeOpts` 新增 `mode`（serde default = manual，舊前端不受影響）與 `targetQuality`
- **設定 UI：模式與畫質目標** — 預設改為自動模式，三檔 preset（視覺無損 90 / 平衡 80 / 激進 70，暫定值見 `docs/calibration.md`）；手動模式保留四條 quality 滑桿；新增進階區（AVIF 速度、PNG/WebP 壓縮力度、漸進式 JPEG）；7 個語系的 i18n 全數補齊
- **JPEG 無損轉碼** — `encoder/jpeg.rs::optimize_lossless()`：jpegtran -optimize -progressive 等價的 DCT 係數轉碼（mozjpeg-sys FFI），像素 bit-identical、零畫質風險，保留 ICC（APP2）；EXIF 旋轉來源自動跳過（轉碼會剝 orientation tag）。已接進 encode 流程：JPEG 有損重壓省不到 2% 時改試無損轉碼當第二機會，原本被 skip 的高壓 JPEG 現在常能免費再省幾 %
- **docs/rubric.md** — AI 優化 loop 用的驗收 rubric；複查後改寫為「施工藍圖」格式：新增 §0.5 現況盤點（標明 ssimulacra2 / bench harness / JPEG 無損轉碼皆為 TODO）、比較基準改為「基準圖」（套用 orientation、剝除 metadata 後的 raw，修正 EXIF 旋轉圖會被冤枉淘汰的邏輯 bug）、移除未支援的 JXL、§8 施工順序含自動品質模式（quality-targeted encoding）的產品方向

---

## [0.1.1] — 2026-04-27

### Added
- **CI ad-hoc macOS signing** — `release.yml` now runs `codesign --force --deep --sign -` on the universal `.app` after Tauri build; ensures Apple Silicon can launch the binary and gives the app a stable code identity (does not remove the Gatekeeper warning, which still requires Apple notarization)
- **README macOS first-launch instructions** — bilingual (EN/中文) `xattr -cr` workaround for the Gatekeeper warning on unsigned builds, plus a Sponsor section linking Ko-fi for users who want to help cover the Apple Developer Program fee toward future notarization

### Fixed
- **macOS portable .app.zip was non-launchable** — `scripts/build-dist.mjs` was zipping with `zip -r -j`; the `-j` ("junk paths") flag flattens directory structure and broke the `.app` bundle. Now zips from the parent dir with the relative bundle name and `-y` to preserve symlinks (Frameworks inside `.app` rely on them).

---

## [0.1.0] — 2026-04-26

### Added
- **README** — Features section, language note, static demo screenshot; copyright year corrected to 2026
- **CI portable artifacts** — `release.yml` now runs `build-dist.mjs` after Tauri build to upload renamed portable `.exe` / `.app.zip` alongside the default installer artifacts; `build-dist.mjs` gains `TARGET` env support for macOS universal path; Windows skips zip and uploads `.exe` directly; macOS zips `.app` bundle directly from source to avoid `copyFile` on a directory

### Fixed
- **macOS CI bundle** — `generate-fixtures` moved from `[[bin]]` to `[[example]]`; Tauri bundles all `[[bin]]` entries but skips examples, so the bundler no longer tries to copy the uncompiled dev utility and fails with "does not exist"
- **macOS build** — `trash::os_limited` (list + restore) is gated to Windows/Linux in `trash` 5.x; wrapped the undo restore path in `#[cfg(...)]` so macOS compiles cleanly (trashed files remain in Trash on macOS, as the crate exposes no programmatic restore API there)
- **CI release creation** — added `permissions: contents: write` to `release.yml`; without it `GITHUB_TOKEN` couldn't create the draft release

### Changed
- **AVIF encoding speed** — `ravif` speed raised from 6 → 8 and per-encode thread count changed from a fixed 1–2 to `num_cpus / 2` clamped to [2, 4]; typical encode time drops ~3-4x with negligible quality difference
- **Replaced Node sidecar with Rust-native encoder** — encoding pipeline now runs entirely in-process via Rust crates; eliminates the bundled Node binary and JSON-lines sidecar protocol
- Standalone CLI (`index.js`) removed; desktop app is now the only artifact
- Portable build script replaced with a `dist/` artifact organizer

### Added — Rust encoder (`desktop/src-tauri/src/encoder/`)
- JPEG encoder via `mozjpeg`
- PNG encoder via `imagequant` + `oxipng`
- WebP encoder via `libwebp`
- AVIF encoder via `ravif` with memory-aware threading
- RGBA decode with EXIF orientation normalization
- `EventSink` trait + `MockSink` for unit testing without Tauri
- `TauriEmitter` production sink
- Stage-based progress events (decode → encode → companion → done)
- Atomic batch completion counter with `tick()`
- Encoder fixture suite with a synthetic image generator

### Added — CI / release
- Cross-platform `release.yml` (GitHub Actions + `tauri-action`): builds Windows `.exe` and macOS `.dmg` on tag push, creates a draft GitHub Release
- Tagged-release process documented in `docs/`

### Fixed
- Progress animation, batch reset, and 2% skip threshold
- Ambiguous `cargo run` resolved via `default-run = hando`
- `tauri.conf.json` pruned of stale `externalBin` / sidecar resource entries

---

## [0.0.x] — Desktop App with Node Sidecar *(2026-04-24)*

Initial Tauri desktop app. Encoding handled by a bundled Node sidecar process (`src/sidecar.js`) communicating over JSON-lines stdin/stdout.

### Added — Desktop app
- Tauri 2 project scaffolded under `desktop/`
- Three-tier architecture: WebView ↔ Rust host ↔ Node sidecar
- Rust `compress` command dispatches encode jobs to sidecar
- Rust sidecar manager with JSON-lines protocol and tokio channels
- Atomic Trash + rename flow per compressed file (Windows Recycle Bin compatible)
- Undo last batch: deletes compressed files + companions, restores originals from Trash
- Recursive folder expansion with format filtering via `tauri-plugin-fs`
- Drag-drop (Tauri 2 API) + click-to-add file picker
- Settings panel: JPEG/PNG/WebP/AVIF quality sliders, Emit WebP/AVIF toggles; persisted via `tauri-plugin-store`
- File list with status icons, size columns, savings %
- Status bar with progress bar, cumulative saved bytes, Show Trash link
- Open-trash command and close-confirm dialog
- Sidecar crash recovery + `sidecar-crashed` event
- Cross-device rename fallback for Windows (C: temp → D: source)
- `\\?\` prefix stripping for Windows Recycle Bin compatibility
- AVIF encoding + Undo deletes companions + `avifQuality` setting
- WebP companion output placed alongside original

### Added — Branding & UI
- Hando brand icon (replaces Tauri placeholder)
- Goldman font for toolbar title
- Three-state theme resolution (system / light / dark) with `data-theme` management
- Theme preference persisted in Settings UI

### Added — Build
- Portable build pipeline: bundles `sharp` deps alongside app; `Hando-portable/` layout

### Added — CLI (precursor, later removed)
- Node CLI (`index.js`): recursive image discovery, bounded concurrency pool, mtime-based skip, WebP companion output, JPEG/PNG/WebP encoding via `sharp`
- Shared `src/config.js` and `src/encoder.js` modules (still used by sidecar at this stage)

---

[0.1.1]: https://github.com/homershie/Hando/compare/v0.1.0...v0.1.1
