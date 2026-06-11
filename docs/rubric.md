# Hando 影像優化 Rubric

> 給 loop / verifier 用的客觀驗收標準。
>
> **核心心法**：畫質與時間當「硬門檻」擋住，只對「檔案大小」做最佳化。
> **鐵律**：每一條都要能用「一行指令」變成一個數字或一個 exit code——量不出來的（例如「壓得漂亮」）不准進這份檔。
> **誠實性**：爛 rubric + 好模型 = 一個自信地跑錯方向的迴圈。寧可門檻嚴一點，也不要讓它鑽漏洞。
>
> **這份文件的角色**：施工藍圖（目標狀態），**不是現況描述**。§0.5 標明現況與缺口——loop 讀這份檔時，標 ⚠TODO 的能力是「要去蓋的」，不是「已經有的」。

---

## 0. 名詞

- **原始檔**：使用者最初丟進來、磁碟上的那顆檔案。
- **基準圖**：原始檔經「decode → 套用 EXIF orientation → 剝除 metadata」後的 raw 像素 buffer。
  ⚠ **所有像素比較與畫質量測一律以基準圖為參考，不是原始檔的 bytes。**
  原因：Hando 的 decode 管線（`encoder/decode.rs`）會把 EXIF 旋轉「烤進」像素並丟棄 EXIF/ICC。若直接拿磁碟上的原始檔去 `compare`，EXIF 旋轉圖的合格無損候選會被冤枉淘汰（像素轉正了、跟原始 bytes 解出來的不同向）。
- **候選**：loop 對同一張基準圖產生的某一組（格式 × 模式 × 參數）輸出。
- **通過 / 得分**：所有硬門檻都過才叫通過；只有通過者才計分，否則得分 = 0。

---

## 0.5 現況盤點（2026-06-11 §8 施工完成後更新）

| 能力 | 現況 | 狀態 |
|---|---|---|
| 支援格式 | JPEG / PNG / WebP / AVIF。**無 JXL**——本 rubric 不含 JXL，未來加格式再擴充。AVIF 解碼已修復（原本 image crate avif feature 只能編不能解） | ✅ 現有 |
| 感知畫質指標 | ✅ `encoder/judge.rs`：ssimulacra2（基準圖 vs 候選）+ `pixels_identical` + `bits_per_pixel` | ✅ 現有 |
| benchmark harness | ✅ `examples/bench.rs`：`sweep`（格式×品質階梯）/ `grid`（旋鈕網格）/ `calibrate`（人眼校準樣本）。結果見 `docs/bench-results.md` | ✅ 現有 |
| JPEG 無損轉碼 | ✅ `encoder/jpeg.rs::optimize_lossless()`（DCT 係數轉碼，像素 bit-identical，保 ICC；EXIF 旋轉來源跳過）。手動模式當 skip 前的第二機會；自動模式是 B 類主要候選 | ✅ 現有 |
| 可調參數 | ✅ quality ×4 + `avifSpeed`(1–10) / `pngOxipngLevel`(0–6，預設 4) / `webpMethod`(0–6) / `jpegProgressive`。網格數據支撐預設值 | ✅ 現有 |
| 跳過邏輯 | 省 < 2% 即 `SkippedNoGain`；自動模式下「沒有候選過畫質門檻」也映射到 skip | ✅ 現有 |
| 多候選機制 | ✅ 自動模式：無損 + 有損候選競爭、通過門檻者取最小（`encoder/auto.rs`）；companion 也走品質搜尋 | ✅ 現有 |
| metadata | EXIF orientation 套用後 EXIF 全剝；✅ ICC passthrough 已實作（`encoder/icc.rs`）：decode 抽出（JPEG APP2 / PNG iCCP / WebP ICCP / AVIF colr-prof）、encode 重嵌（JPEG APP2 / PNG iCCP / WebP VP8X+ICCP）。**唯一例外：AVIF 輸出** — ravif/avif-serialize 只支援 nclx colr box，無法嵌 ICC，AVIF 輸出（含 companion）仍會丟 profile | ⚠ 部分（剩 AVIF 輸出端） |
| `S` 校準 | ✅ 人眼定案 90/80/70（2026-06-11，homer；可見差異約 s 70–75，`docs/calibration.md`）。實拍照片複核列「後續」 | ✅ 現有 |
| 偽裝有損偵測 | ✅ `judge.rs::jpeg_blockiness`：JPEG 8×8 格線指紋（小振幅梯度的相位統計，耐裁切），無損容器超標即抬到 B 類門檻（§1 第二道防線）。完整 NR 指標（NIQE/BRISQUE，§4.3 輸出端檢查）仍後置 | ✅ 現有 |

---

## 1. 前置輸入閘（先判斷原始檔，決定走哪條規則）

量兩個值：
- `format` → 副檔名 + magic bytes（Hando 的 `ImageExt` 判定；WebP 需再判 lossy/lossless 變體）
- `bpp`（每像素位元）→ `bytes × 8 ÷ (寬 × 高)`

分類：
- **A 乾淨來源**：lossless 格式（PNG / 無損 WebP）**且** bpp 高 → 走 §2 / §3，`ssimulacra2 vs 基準圖` 有效。
- **B 已是有損**：lossy 格式（JPEG / 有損 WebP / AVIF）**或** bpp 低 → 走 §4（世代損失保護）。
- ✅ **第二道防線**（已實作，2026-06-11）：無損容器的像素若帶 JPEG 8×8 格線指紋（`judge.rs::jpeg_blockiness`，相位無關、耐裁切，門檻 1.25），即使容器無損也抬到 B 類門檻 `max(S, 90)`——擋掉「PNG 其實是從 JPEG 另存」這種藏雜訊的情況。已知極限：q≈95+ 相機級 JPEG 另存的 PNG 指紋太弱測不到，但這類來源近乎乾淨，照 A 類處理損失極小。完整 NR 指標（NIQE/BRISQUE）仍後置。

---

## 2. 無損候選 rubric（A 類）

**硬門檻（任一不過 → 淘汰，得分 0）**
1. 解碼成功 → 解碼器 exit code / Result = Ok
2. 像素完全相同 → 輸出 decode 後與**基準圖**逐 pixel 相等（diff count = 0）
   ⚠ 比較對象是基準圖，**不是原始檔**（理由見 §0 名詞）。跨格式時兩邊都 decode 成 raw 再比。
   ⚠ ICC passthrough 已實作（`encoder/icc.rs`，roundtrip 測試守著）；僅 AVIF 輸出因 ravif 只支援 nclx 而仍丟 ICC。rubric 以「像素值相等」為準，ICC 等位元組層面的 metadata 不入分。
3. 編碼時間 ≤ `T_lossless`

**目標函數**
4. 壓縮率 = `(原始大小 − 輸出大小) / 原始大小`，越大越好

---

## 3. 有損候選 rubric（A 類，含 無損 → 有損）

**硬門檻（任一不過 → 淘汰）**
1. 解碼成功
2. 感知畫質 → `ssimulacra2(基準圖, 輸出) ≥ S` ⚠TODO：指標尚未接入，蓋裁判是第一個施工項（見 §8）
   ⚠ 參考**一定是基準圖**，不是任何中間檔——否則會嚴重低估損失，門檻形同虛設。
3. 編碼時間 ≤ `T_lossy`

**目標函數**
4. 壓縮率越大越好

---

## 4. 已是有損輸入（B 類）的特別規則

不信任「ssimulacra2 vs 已髒參考」當綠燈（兩張「壞得很像」也會給高分）。
依 §1 的 bpp 訊號再分兩級（實作：`encoder/auto.rs`）：

- **B1（bpp ≥ 1.0，接近乾淨）**：相機直出的高品質 JPEG（bpp 2–5）幾乎沒有可見壓縮痕跡，「vs 輸入」≈「vs 乾淨原圖」→ 直接用 preset 的 `S` 當門檻重壓，這是工具的主用例。
- **B2（bpp < 1.0，已被壓兇）**：
  1. **無損轉碼優先** ✅（已實作）：JPEG 走 DCT 係數無損優化（`optimize_lossless`）——像素一個都沒動，縮多少賺多少，零畫質風險。B2 的 JPEG **只**走這條，不重壓。
  2. **沒有無損轉碼可走時**（AVIF / 有損 WebP / EXIF 旋轉的 JPEG）：門檻抬到 `max(S, 90)`——「輸出與輸入視覺上不可辨」，把世代損失綁死在一手之內。
  3. （未來）無參考指標 `NIQE(輸出) ≤ NIQE(輸入) + ε` 作為第二道防線 ⚠TODO（Rust 生態 NR 指標選項少，後置）

**目標函數**：在通過者中，壓縮率越大越好。

---

## 5. 跨候選的選法

**不預先決定每張圖該無損或有損。** loop 對同一張基準圖同時生出 §2 + §3（或 §4）的候選，全部丟進**同一套門檻**跑，通過者比大小，取最小。

門檻 + 大小目標會自動幫你選：
- 照片 → 有損又小又過門檻 → 有損贏
- 文字 / UI 截圖 → 有損在門檻會糊掉字邊緣、過不了 → 自動被擋，無損贏
- 極小圖示 → 無損有時反而更小 → 大小目標自動選無損

現況的 companion 機制（同一次 decode 編出 WebP/AVIF）已是這個方向的雛形，差的是「候選之間競爭、只留贏家」的裁決層。

---

## 6. 待校準的參數（先留 TODO，這些只有人能定）

| 參數 | 用途 | 校準法 |
|---|---|---|
| `S` | ssimulacra2 畫質下限 | ✅ 已定案 90/80/70（2026-06-11 人眼並排複核，可見線約 s 70–75；詳見 `docs/calibration.md`）。校準法存檔備查：挑代表圖（**務必含照片＋文字截圖**），各壓幾個等級，親眼跟原圖並排看，找「開始看得出差」那條線的分數 |
| `T_lossless` / `T_lossy` | 每張圖的時間預算（秒） | 看實際使用情境可接受的等待；注意自動品質模式（§8）會把編碼次數乘 3–5 倍 |
| `ε` | B 類重壓時可接受的 NIQE 退步量 | 寧小勿大，先抓接近 0 |
| bpp 分界 | 判 A / B 的低位元門檻 | 照片類約 < 0.5 起跳，視素材調 |

---

## 7. 工具（Rust crate 優先，直接進 harness，免跨語言）

| 用途 | 選型 | 接入狀態 |
|---|---|---|
| 感知畫質 | `ssimulacra2` crate（首選）；`dssim`（Kornel，備選） | ✅ `judge.rs` |
| 無損像素比對 | harness 內直接 decode 後逐 pixel 比（不必外掛 magick） | ✅ `judge.rs::pixels_identical` |
| 計時 | harness 內 `std::time::Instant`，多次取中位數；外部驗證可用 `hyperfine` | ✅ `examples/bench.rs` |
| JPEG 無損優化 | mozjpeg 的 DCT 係數路徑（jpegtran 等價）；mozjpeg crate 已在依賴裡 | ✅ `jpeg.rs::optimize_lossless` |
| 無參考指標 | 輸入端「偽裝有損」改用自製 JPEG 格線指紋（`judge.rs::jpeg_blockiness`，零依賴）✅；NIQE / BRISQUE（§4.3 輸出端檢查）Rust 生態選項少，後置 | ⚠ 部分 |

---

## 8. 施工順序（2026-06-11 全部完成 ✅）

1. ✅ **蓋裁判**：`encoder/judge.rs` + `examples/bench.rs`（`sweep` 模式）——每個候選吐 `size / ssimulacra2 / time` 三個數字。
2. ✅ **校準 `S`**：`bench calibrate` 產出階梯樣本，90/80/70 已於 2026-06-11 人眼定案（`docs/calibration.md`；可見差異約 s 70–75）。
3. ✅ **JPEG 無損轉碼路徑**：`optimize_lossless()`，像素 bit-identical 有測試守著。
4. ✅ **暴露旋鈕**：`avifSpeed` / `pngOxipngLevel` / `webpMethod` / `jpegProgressive`（serde default 向後相容）。
5. ✅ **parameter golf**：`bench grid` 掃過旋鈕網格，oxipng 預設 2→4 由數據定案（`docs/bench-results.md`）。
6. ✅ **自動品質模式**：`encoder/auto.rs` quality-targeted encoding（步進 4 的二分搜尋，~5 輪收斂）；B1/B2 輸入閘照 §4；UI 三檔 preset + 手動/進階區（`src/ui/settings.ts`）。

### 後續（非 §8 範圍）

- ~~ICC passthrough（重壓路徑）~~ ✅ 已實作（`encoder/icc.rs`）；殘留：AVIF 輸出端（ravif 只支援 nclx，無法嵌 ICC）
- 真實照片 fixtures + 重跑 `calibrate`，複核 `S`（合成樣本已人眼定案 90/80/70；實拍素材偏難壓，複核重點是激進檔 70 是否要抬）
- ~~輸入端第二道防線（偽裝有損偵測）~~ ✅ 已實作（`judge.rs::jpeg_blockiness`，2026-06-11）；殘留：§4.3 輸出端 `NIQE(輸出) ≤ NIQE(輸入) + ε` 仍後置，等實際遇到破圖案例再評估
- JXL 等新格式評估

---

*心法回顧：目標（task）告訴 loop「去哪裡」，這份 rubric 告訴 verifier「怎麼確認它真的到了那裡」。*
