# Hando 影像優化 Goal（/goal task spec）

> 這份檔是**任務書**，餵給 Claude Code 的 `/goal`。它只回答一件事：**loop 該去哪裡。**
> 「怎麼確認真的到了」交給 [`rubric.md`](rubric.md)（裁判）；「每個候選的數字」交給 `bench eval`（量測層）。
>
> **配套關係**
> - 任務（這份）→ 目的地 + 搜尋空間 + **停止條件**
> - [`rubric.md`](rubric.md) → 硬門檻 + 目標函數（verifier 照它評分）
> - `bench eval` / `bench corpus` → 一行指令吐一行 JSON，把宣稱變成數字

---

## 0. 一句話目標

> 對 `corpus/`（或 `src-tauri/tests/fixtures/`）裡的每一張圖，在編碼器的
> **格式 × 品質 × effort 旋鈕**搜尋空間裡，找出**通過 [`rubric.md`](rubric.md) 全部硬門檻、
> 且檔案最小**的那組候選；逐張記錄贏家與壓縮率，跨張累積「哪種圖、哪組參數贏」的筆記。

不是「把每張圖都壓成有損」，也不是「都壓成無損」——把所有候選丟進同一套門檻（rubric §5），門檻 + 大小目標會自動分流（照片走有損、文字截圖走無損）。

---

## 1. 量測層：loop 唯一該呼叫的指令

先建好 release binary（一次）：

```bash
cd src-tauri && cargo build --release --example bench
BIN=src-tauri/target/release/examples/bench
```

### 1.1 列語料庫 + 分流（搜尋前先跑一次）

```bash
$BIN corpus <dir>        # 預設 tests/fixtures
```

每張圖吐一行 JSON：`path / format / width / height / bytes / bpp / blockiness / class_hint / reason`。
`class_hint`（A / B）是**建議值**——最終 A/B 判定照 rubric §1，但這行先告訴你該走哪條規則分支。

### 1.2 評一個候選（搜尋主迴圈）

```bash
$BIN eval <input> <out_ext:jpg|png|webp|avif> <quality> [knob]
#   knob: jpg→progressive(0|1, 預設1)  png→oxipng(0..=6, 預設4)
#         webp→method(0..=6, 預設4)    avif→speed(1..=10, 預設8)
```

成功吐一行 JSON、exit 0：

```json
{"input":"...","out":"webp","quality":75,"knob":4,"src_bytes":65116,
 "out_bytes":9326,"ratio":0.8568,"width":1920,"height":1080,"bpp":0.2512,
 "ssimulacra2":77.74,"lossless":false,"encode_ms":78,"ok":true}
```

decode/encode 失敗、或不可能的請求（如透明來源輸出 JPEG）→ `"ok":false` + exit 1。

> **量測層邊界（誠實標註）**：`bench eval` 只做**重新編碼**候選。JPEG 的**無損 DCT 轉碼**
> （rubric §4 B2 主路徑）走的是 app 內 `encoder/jpeg.rs::optimize_lossless()`，**沒有**從
> bench 暴露。所以 B2 的 JPEG，loop **不要**用 eval 跑有損重壓去比大小——照 §3 規則直接判
> 「只走無損轉碼，交給 app auto 模式」。要在 bench 端也能量轉碼，需另開 `eval-transcode`（TODO）。

---

## 2. 搜尋空間

對每張圖，候選 = `格式 × quality × knob` 的組合。**不要全網格暴搜**——照下面分支裁掉不可能的分支，再用停止條件收斂。

| 格式 | quality 候選 | knob 候選 | 備註 |
|---|---|---|---|
| jpeg | 50 / 65 / 75 / 85 / 90 | progressive 1（預設）；缺位元時試 0 | 透明來源跳過 |
| webp | 65 / 75 / 85 / 90 / 100 | method 4（預設）；要更小再試 6 | q100 = 無損 |
| avif | 40 / 50 / 60 / 75 / 85 | speed 8（預設）；要更小再試 6/4（變慢） | 輸出端丟 ICC（rubric §0.5） |
| png | （imagequant q）60 / 75 / 85 / 100 | oxipng 4（預設）；要更小再試 6 | 無損候選主力 |

**先試預設 knob，只有當某品質檔已通過門檻、想再擠壓縮率時才動 knob。** knob 的邊際收益小、時間成本高（avif speed 4 比 8 慢數倍），別在 knob 上燒預算。

---

## 3. 分支規則（照 `bench corpus` 的 class_hint + bpp）

設 `S` = preset 目標分（UI 三檔：視覺無損 90 / 平衡 80 / 激進 70；本次跑哪檔由呼叫者指定）。

- **A 類（乾淨無損來源）**：同時生**無損候選**（png oxipng / webp q100）和**有損候選**（webp/avif/jpeg 各品質檔），全部丟同一套門檻，門檻 `ssimulacra2 ≥ S`。
- **B1（lossy 或低 bpp，但 bpp ≥ 1.0）**：相機直出高品質 JPEG，「vs 輸入」≈「vs 乾淨原圖」→ 直接用 preset `S` 當門檻重壓。
- **B2（bpp < 1.0，已被壓兇）**：
  - **JPEG**：**只**走無損轉碼（app `optimize_lossless`），**不要**用 eval 跑有損重壓比大小（量測層邊界，見 §1.2）。
  - **非 JPEG（avif / 有損 webp / EXIF 旋轉 JPEG）**：門檻抬到 `max(S, 90)`——「輸出與輸入視覺不可辨」，把世代損失綁死在一手內。

---

## 4. 硬門檻 + 目標函數（verifier 照 rubric 評每個候選）

每個 `eval` 候選，verifier 子代理（**獨立、全新 context**——官方文件：獨立 verifier 通常勝過 self-critique）檢查：

1. `ok == true`（decode + encode + re-decode 都成功）
2. 感知畫質：`ssimulacra2 ≥ S_effective`（S_effective 照 §3 分支）
3. 時間：`encode_ms ≤ T`（預算見 §6；超時即淘汰，避免 avif speed 1 這種無底洞）
4. 無損候選額外要求 `lossless == true`（rubric §2：逐像素相等）

**通過 = 4 條全過。只有通過者計分，否則得分 0。**
**目標函數**：通過者中 `out_bytes` 最小（= `ratio` 最大）者贏。

**跳過**：贏家的 `ratio < 0.02`（省不到 2%）→ 判 `SkippedNoGain`，保留原檔。

orchestrator 跨候選取最小贏家，寫進結果表。

---

## 5. ⚠ 停止條件（最容易爆的點——一定要塞死）

rubric 是 **maximize 壓縮率**，**沒有天然終點**。不給上限，loop 會一直探索到 harness 撐不住。**每一輪都要對照下面三條，任一觸發就收手：**

**每張圖**（三選一先到者為準）：
- **候選上限 N = 12**：試滿 12 組就停，回報目前最小贏家。
- **連續無改善 K = 3**：連續 3 個候選都沒比當前最小贏家更小 → 收斂，停。
- **單張時間上限 T_img = 60s**：含所有候選的累計編碼時間超過即停。

**整個語料庫**：
- **總時間上限 T_total**（無人值守整夜跑時設，例如 6h）；或所有圖跑完。

收手時**永遠回報目前最佳**，不要因為「還沒試完所有組合」就不回報。

---

## 6. 時間預算（硬門檻 #3 的 T）

| 預算 | 建議值 | 說明 |
|---|---|---|
| `T_lossy`（每候選） | ≤ 3s | 一般互動情境；avif speed <6 容易破表，破表即淘汰該候選 |
| `T_lossless`（每候選） | ≤ 3s | oxipng 6 + 大圖會接近 |
| `T_img`（每張，§5） | 60s | 自動品質搜尋會把編碼次數乘數倍 |
| `T_total`（整批） | 視情境 | 無人值守才設 |

---

## 7. Scaffolding（長跑迴圈照抄這幾條）

**7.1 進度對齊真實工具輸出**
回報前，把每個宣稱對照**這個 session 的 `bench eval` 實際輸出**。沒跑過 eval 就量不到的數字（「這張省了 40%」）不准寫進結果——沒驗證過就明說「未量測」。爛數字比沒數字更糟。

**7.2 記憶系統（跨張累積）**
把學到的寫進 `docs/goal-learnings.md`：**頂端一行摘要**，其下**一條教訓一段**（哪種圖類型 × 哪組參數贏、哪些分支白試了）。下一張圖、下一次跑，先讀這份檔再決定候選順序，省掉重複探索。

**7.3 無人值守提醒（整夜跑才加）**
> 你正在自主執行，使用者目前不在線上。可逆、且符合本任務原始請求的動作（跑 eval、寫結果表、寫 learnings）直接做，不要停下來問「要我繼續嗎」。只有遇到**不可逆**或**超出本任務範圍**的動作才停下等人。

---

## 8. 結果輸出

逐張一列，寫進 `docs/goal-results.md`（或 stdout 表）：

| image | class | winner（格式@q knob） | out_bytes | ratio | ssimulacra2 | 判定 |
|---|---|---|---|---|---|---|

判定 ∈ `{ 有損贏 / 無損贏 / 無損轉碼(B2-JPEG) / SkippedNoGain }`。

---

## ⚠ 撰寫 skill 的地雷（給未來把這份包成 skill 的人）

別在 prompt / skill 裡叫 loop「複述」或「解釋你的推理過程」「show your thinking」——
Fable 5 對這類指令會觸發 `reasoning_extraction` 拒答、然後 fallback 到 Opus。
要它回報的是**工具輸出的數字與結論**（§7.1），不是它的思路。舊 skill 要掃一遍把這種指令拿掉。

---

*心法回顧：這份 task 告訴 loop「去哪裡」，[`rubric.md`](rubric.md) 告訴 verifier「怎麼確認它真的到了那裡」，`bench eval` 把每個宣稱變成一個可查的數字。*
