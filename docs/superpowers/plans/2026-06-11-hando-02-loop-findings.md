# Hando 0.2 — Loop 發現落地 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 2026-06-11 goal loop 的發現（docs/goal-results.md + docs/goal-learnings.md）落進 app 的 auto 編碼路徑：P0 資料損失守門（ICC+AVIF）、P1 搜尋策略 guard（高 bpp 直走轉碼、2MP+ 旋鈕守門）、P2 三 preset 驗證。

**Architecture:** App 是「自己跑搜尋」型（`encoder/auto.rs` 二分搜尋），所以 §2 對應表轉譯成搜尋策略 + guard。主輸出不換格式（`commands.rs` 把 main 放回 src path），跨格式贏家規則只能落成同格式內的 knob guard。所有改動集中在 `encoder/mod.rs`（skip 守門）與 `encoder/auto.rs`（搜尋 guard）。

**Tech Stack:** Rust（mozjpeg / oxipng+imagequant / libwebp / ravif / ssimulacra2），測試 = cargo test + fixtures。

**前提確認（清單開頭的問題）：** app 自己跑搜尋（auto.rs 二分搜尋），非靜態對應。已實作且有測試的項目（<2% skip、B2-JPEG 只走轉碼、B2/偽裝 PNG 門檻抬 max(S,90)）不重做，只補缺的 guard。

---

### Task 1: P0 — ICC 來源的 AVIF 主輸出省幅 <10% 判 skip

AVIF 輸出端 ravif 只支援 nclx，必丟 ICC profile（icc.rs 模組註解、rubric §0.5）。bench 實測 with_icc.avif：avif@87 過 90 門檻但只省 2.8% — 為這點省幅丟色彩描述檔是資料損失。規則：main 輸出是 AVIF（只可能來源是 AVIF）且 `decoded.icc_profile` 存在時，省幅門檻 2% → 10%。

**Files:**
- Modify: `src-tauri/src/encoder/mod.rs:201-213`（skip 判斷）+ tests 區
- Modify: `CHANGELOG.md`

- [ ] **Step 1: 寫失敗測試**（`mod.rs` 的 `#[cfg(test)] mod tests` 內，`encode_tiny_already_optimized_skips` 之後）

```rust
#[test]
fn avif_source_with_icc_and_small_gain_skips() {
    // P0 §1.1: AVIF 重壓必丟 ICC（ravif 只寫 nclx）。with_icc.avif 的
    // B2 門檻是 max(S,90)，贏家只省 ~3%（bench 實測 2.8%）——為這點
    // 省幅丟色彩描述檔不值得，必須 skip 保留原檔。
    let o = EncodeOpts {
        mode: EncodeMode::Auto,
        target_quality: 80.0,
        ..EncodeOpts::default()
    };
    let outcome = encode(EncodeRequest {
        src_path: &fixture("with_icc.avif"),
        ext: ImageExt::Avif,
        opts: &o,
        progress_cb: None,
    })
    .unwrap();
    assert!(
        matches!(outcome, EncodeOutcome::SkippedNoGain { .. }),
        "ICC-tagged AVIF with <10% gain must skip, got {outcome:?}"
    );
}
```

- [ ] **Step 2: 跑測試確認失敗**

Run: `cd src-tauri && cargo test avif_source_with_icc_and_small_gain_skips`
Expected: FAIL（目前 2% bar 會讓 2.8% 的省幅過關 → Encoded）。
注意：若意外 PASS，先查 encode_auto 是否根本沒找到候選（None → skip 的假陽性）——在測試裡暫時 println outcome 確認是 Encoded 才繼續。

- [ ] **Step 3: 實作**（`mod.rs` 把固定 98 bar 改成動態）

把 `mod.rs:201-204` 的：

```rust
    // Skip if savings are less than 2% of the source — prevents endless re-compression
    // of already-optimized files where the encoder finds only marginal improvements.
    if main.bytes * 100 >= src_bytes * 98 {
```

改成：

```rust
    // Skip when the savings don't justify the cost. Baseline: <2% gain is
    // churn, not compression. AVIF main output additionally drops the ICC
    // profile (ravif is nclx-only, rubric §0.5), so an ICC-tagged source must
    // clear 10% before that loss is worth it (docs/goal-learnings.md,
    // with_icc.avif: 2.8% gain is not).
    let keep_bar: u64 =
        if main.ext == ImageExt::Avif && decoded.icc_profile.is_some() { 90 } else { 98 };
    if main.bytes * 100 >= src_bytes * keep_bar {
```

- [ ] **Step 4: 跑測試確認通過 + 不破壞既有測試**

Run: `cd src-tauri && cargo test --lib encoder::`
Expected: 全 PASS（含新測試）。

- [ ] **Step 5: CHANGELOG**（`[Unreleased]` → `### Fixed` 最上方加一行）

```markdown
- **ICC 來源的 AVIF 重壓省幅 <10% 改判 skip** — ravif 只支援 nclx，AVIF 輸出必丟 ICC profile；with_icc.avif 實測贏家過 90 門檻卻只省 2.8%，為這點省幅默默丟色彩描述檔等於資料損失（攝影師等級的偏色風險）；現在 main 輸出是 AVIF 且來源帶 ICC 時，省幅門檻 2% → 10%，不到就 SkippedNoGain 保留原檔
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/encoder/mod.rs CHANGELOG.md
git commit -m "fix(encoder): require 10% gain before re-encoding ICC-tagged AVIF

AVIF output drops the ICC profile (ravif is nclx-only). A 2.8% size win
is not worth silently losing color management, so the SkippedNoGain bar
rises from 2% to 10% for ICC-tagged AVIF sources.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: P1 — 超高 bpp JPEG 跳過有損搜尋，直走無損轉碼

large_photo（24MP、bpp≈10）有損候選全滅：q75→q85 分數只 5.27→16.10（ssimulacra2 重罰顆粒被抹掉），且每發 encode+judge 數秒。規則：JPEG 來源 bpp ≥ 8.0 → 有損搜尋免了，只走 DCT 轉碼（相機 JPEG 在 2–5 bpp，不受影響）。

**Files:**
- Modify: `src-tauri/src/encoder/auto.rs`（常數 + Jpeg 分支 + tests）
- Modify: `CHANGELOG.md`

- [ ] **Step 1: 改寫既有 noise 測試成 futile 案例 + 新增 B1 真實照片測試**

`auto.rs` tests 裡的 `auto_high_bpp_jpeg_searches_lossy_and_clears_gate` 整個替換成下面兩個測試（noise q95 的 bpp 很可能已落在 futile 區，語意要分家；B1 路徑改用 checked-in 的 landscape2.jpg，bpp≈2.59）：

```rust
#[test]
fn auto_b1_jpeg_searches_lossy_and_clears_gate() {
    // landscape2.jpg：真實相機 JPEG，bpp ≈ 2.6 → B1，preset 門檻直接適用，
    // 有損搜尋必須找到過門檻的候選。
    let path = fixture("landscape2.jpg");
    let baseline = decode(&path, ImageExt::Jpeg).unwrap();
    let src_bytes = std::fs::metadata(&path).unwrap().len();
    let bpp = judge::bits_per_pixel(src_bytes, baseline.width, baseline.height);
    assert!(
        (B1_BPP_THRESHOLD..LOSSY_FUTILE_BPP).contains(&bpp),
        "fixture assumption: B1-range bpp, got {bpp}"
    );

    let o = EncodeOpts { target_quality: 70.0, ..opts() };
    let out = encode_auto(&path, ImageExt::Jpeg, &baseline, &o, src_bytes)
        .unwrap()
        .expect("B1 camera JPEG should find a passing candidate");
    let dec = decode(&out.tmp_path, ImageExt::Jpeg).unwrap();
    let identical = judge::pixels_identical(&baseline, &dec);
    let score = judge::ssimulacra2_score(&baseline, &dec).unwrap();
    let _ = std::fs::remove_file(&out.tmp_path);
    assert!(identical || score >= 70.0, "winner must clear the gate, got {score:.2}");
}

#[test]
fn auto_futile_bpp_jpeg_skips_lossy_search_and_transcodes() {
    // 顆粒/雜訊型超高 bpp 來源：有損候選全滅（large_photo q85 只得 16.1 分）
    // 且每發數秒——唯一出路是無損 DCT 轉碼，輸出像素必須與基準圖相同。
    // 合成 q100 噪點 JPEG 重現 large_photo 的 bpp 級別（fixture 是 gitignored）。
    let (w, h) = (256u32, 256u32);
    let mut state = 0x12345678u32;
    let rgba: Vec<u8> = (0..w * h * 4)
        .map(|i| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            if i % 4 == 3 { 255 } else { (state >> 24) as u8 }
        })
        .collect();
    let noise = DecodedImage { rgba, width: w, height: h, icc_profile: None };
    let q100 = jpeg::encode(&noise, 100, true).unwrap();
    let tmp = tempfile::Builder::new().suffix(".jpg").tempfile().unwrap();
    std::fs::copy(&q100.tmp_path, tmp.path()).unwrap();
    let _ = std::fs::remove_file(&q100.tmp_path);

    let baseline = decode(tmp.path(), ImageExt::Jpeg).unwrap();
    let src_bytes = std::fs::metadata(tmp.path()).unwrap().len();
    let bpp = judge::bits_per_pixel(src_bytes, w, h);
    assert!(bpp >= LOSSY_FUTILE_BPP, "q100 noise should be futile-high bpp, got {bpp}");

    let out = encode_auto(tmp.path(), ImageExt::Jpeg, &baseline, &opts(), src_bytes)
        .unwrap()
        .expect("lossless transcode should be available");
    let dec = decode(&out.tmp_path, ImageExt::Jpeg).unwrap();
    let identical = judge::pixels_identical(&baseline, &dec);
    let _ = std::fs::remove_file(&out.tmp_path);
    assert!(identical, "futile-bpp JPEG must come from the lossless transcode");
}
```

- [ ] **Step 2: 跑測試確認失敗**

Run: `cd src-tauri && cargo test auto_futile_bpp auto_b1_jpeg`
Expected: `auto_futile_bpp...` FAIL — `LOSSY_FUTILE_BPP` 還不存在，先是編譯錯誤；這就是本步的失敗訊號。

- [ ] **Step 3: 實作**（`auto.rs`）

在 `B1_BPP_THRESHOLD` 常數（auto.rs:23）後面加：

```rust
/// At/above this bits-per-pixel a lossy re-encode is futile (2026-06-11 goal
/// loop, large_photo bpp≈10): ssimulacra2 punishes grain removal so hard that
/// no quality clears any gate (q85 scored 16.1 vs target 80) while each probe
/// on such poorly-compressed pixels costs seconds. Camera JPEGs sit at
/// 2–5 bpp, so they stay on the normal B1 path.
const LOSSY_FUTILE_BPP: f64 = 8.0;
```

把 `encode_auto` Jpeg 分支的 `let lossy = if bpp >= B1_BPP_THRESHOLD || !upright {`（auto.rs:110）改成：

```rust
            let lossy = if bpp >= LOSSY_FUTILE_BPP {
                // Grain-heavy, barely-compressed source: every lossy probe
                // fails the gate and burns seconds — transcode only. For a
                // rotated source this leaves no candidate at all; the caller
                // reports SkippedNoGain and the original is preserved.
                None
            } else if bpp >= B1_BPP_THRESHOLD || !upright {
```

（其餘分支不動。）

- [ ] **Step 4: 跑測試確認通過**

Run: `cd src-tauri && cargo test --lib encoder::auto`
Expected: 全 PASS。若 `auto_futile_bpp...` 在 bpp assert 掛掉（q100 噪點 bpp < 8 的意外情況），把噪點圖放大到 512×512 並重跑——噪點 JPEG q100 實務上 >10 bpp，這是保險說明不是預期路徑。

- [ ] **Step 5: CHANGELOG**（`### Changed` 加一行）

```markdown
- **超高 bpp JPEG（≥8 bpp）的 auto 模式直走無損轉碼** — 顆粒照片（如 24MP、bpp≈10 的 large_photo）有損候選全滅：q75→q85 分數只 5.27→16.10（ssimulacra2 重罰顆粒被抹掉），每發 encode+judge 還要 3–8s；現在 bpp ≥ 8 跳過整輪有損搜尋直接 DCT 轉碼，單張省下數十秒白燒，相機直出 JPEG（2–5 bpp）不受影響
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/encoder/auto.rs CHANGELOG.md
git commit -m "perf(encoder): skip futile lossy search for grain-heavy >=8bpp JPEGs

ssimulacra2 punishes grain removal so hard that no quality clears the
gate on these sources (large_photo q85 scored 16.1 vs target 80), while
each 24MP probe costs 3-8s. Go straight to the lossless DCT transcode.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: P1 — 2MP+ 大圖的 auto 模式旋鈕守門（avif speed 9、oxipng cap 2）

bench 實測：avif speed 8 在 ≥2MP 必破 3s（realphoto 3063ms→speed 9 變 1579ms、體積只多 ~6%）；oxipng level 4 在 2MP 照片級 PNG 單發 3694ms，auto 搜尋一張要跑多發。只動 auto 模式（manual 模式單發、是使用者明示的旋鈕，不覆寫）。

**Files:**
- Modify: `src-tauri/src/encoder/auto.rs`（helper + 三個呼叫點 + tests）
- Modify: `CHANGELOG.md`

- [ ] **Step 1: 寫失敗測試**（`auto.rs` tests 內）

```rust
#[test]
fn large_image_knob_guards() {
    // 2026-06-11 goal loop：2MP+ 來源 avif speed <9 / oxipng >2 會把單發
    // 編碼時間撐破預算（搜尋還要乘上發數）；小圖維持使用者旋鈕。
    let dims = |w: u32, h: u32| DecodedImage {
        rgba: Vec::new(),
        width: w,
        height: h,
        icc_profile: None,
    };
    let small = dims(320, 240);
    let large = dims(1920, 1080); // 2,073,600 px ≥ 2MP
    let o = opts(); // avif_speed 8, png_oxipng_level 4

    assert_eq!(auto_avif_speed(&small, &o), 8, "small keeps user speed");
    assert_eq!(auto_avif_speed(&large, &o), 9, "large floors at speed 9");
    let fast = EncodeOpts { avif_speed: 10, ..opts() };
    assert_eq!(auto_avif_speed(&large, &fast), 10, "never slows a faster user choice");

    assert_eq!(auto_oxipng_level(&small, &o), 4, "small keeps user level");
    assert_eq!(auto_oxipng_level(&large, &o), 2, "large caps at level 2");
    let light = EncodeOpts { png_oxipng_level: 1, ..opts() };
    assert_eq!(auto_oxipng_level(&large, &light), 1, "never raises a lighter user choice");
}
```

- [ ] **Step 2: 跑測試確認編譯失敗**

Run: `cd src-tauri && cargo test large_image_knob_guards`
Expected: 編譯錯誤 — `auto_avif_speed` / `auto_oxipng_level` 不存在。

- [ ] **Step 3: 實作 helper**（`auto.rs`，放在 `LOSSY_FUTILE_BPP` 之後）

```rust
/// Pixel count at/above which the large-image knob guards kick in
/// (2MP ≈ 1080p, docs/goal-learnings.md).
const LARGE_IMAGE_PIXELS: u64 = 2_000_000;

fn is_large(decoded: &DecodedImage) -> bool {
    decoded.width as u64 * decoded.height as u64 >= LARGE_IMAGE_PIXELS
}

/// AVIF speed for auto-mode searches: 2MP+ sources start at speed 9 — half
/// the encode time for ~6% size (realphoto avif@85: 3063ms → 1579ms). Never
/// slows down a faster user-chosen speed.
fn auto_avif_speed(decoded: &DecodedImage, opts: &EncodeOpts) -> u8 {
    if is_large(decoded) { opts.avif_speed.max(9) } else { opts.avif_speed }
}

/// oxipng level for auto-mode candidates: photo-grade 2MP+ PNGs take >3s per
/// probe at level 4 (realphoto: 3694ms) and the quality search multiplies
/// that — cap at 2. Never raises a lighter user-chosen level.
fn auto_oxipng_level(decoded: &DecodedImage, opts: &EncodeOpts) -> u8 {
    if is_large(decoded) { opts.png_oxipng_level.min(2) } else { opts.png_oxipng_level }
}
```

- [ ] **Step 4: 接上三個呼叫點**

`encode_auto` Png 分支（auto.rs:89-96）改成：

```rust
        ImageExt::Png => {
            // A-class: lossless and quantized candidates compete (rubric §5)
            let level = auto_oxipng_level(decoded, opts);
            let lossless = png::encode(decoded, 100, level).ok();
            let quantized = search_min_quality(decoded, gate, ImageExt::Png, |q| {
                png::encode(decoded, q, level)
            });
            Ok(pick_smaller(lossless, quantized))
        }
```

`encode_auto` Avif 分支（auto.rs:124-129）改成：

```rust
        ImageExt::Avif => {
            // No lossless transcode path exists for AVIF; tightly-gated re-encode
            let speed = auto_avif_speed(decoded, opts);
            Ok(search_min_quality(decoded, gate, ImageExt::Avif, |q| {
                avif::encode(decoded, q, speed)
            }))
        }
```

`encode_companion_auto` 的 Avif 分支（auto.rs:145-147）改成：

```rust
        ImageExt::Avif => {
            let speed = auto_avif_speed(decoded, opts);
            search_min_quality(decoded, gate, ImageExt::Avif, |q| {
                avif::encode(decoded, q, speed)
            })
        }
```

- [ ] **Step 5: 跑測試確認通過**

Run: `cd src-tauri && cargo test --lib encoder::`
Expected: 全 PASS。

- [ ] **Step 6: CHANGELOG**（`### Changed` 加一行）

```markdown
- **2MP+ 大圖的 auto 模式旋鈕守門** — AVIF 搜尋速度起手抬到 max(使用者值, 9)：實測時間砍半（realphoto avif@85 3063ms→1579ms）、體積只多 ~6%；PNG 候選的 oxipng 壓到 min(使用者值, 2)：level 4 在 2MP 照片級 PNG 單發 3.7s，乘上搜尋發數會把單張時間撐爆。manual 模式不受影響（單發、使用者明示旋鈕）
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/encoder/auto.rs CHANGELOG.md
git commit -m "perf(encoder): large-image knob guards for auto-mode searches

2MP+ sources floor AVIF at speed 9 (half the time, ~6% size) and cap
oxipng at level 2 (level 4 costs 3.7s per probe on photo-grade PNGs,
multiplied by the quality search).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: P2 — 三個 UI preset（S=70/80/90）行為 smoke test

目前只在 S=80 驗過。加一個跨 preset 的 smoke test：每個 preset 對代表性 fixtures 跑 encode_auto，結果只能是「過門檻的 Encoded」或「Skip」，不准 error、不准低於門檻。標 `#[ignore]`（跑滿要數分鐘），發版前手動跑。

**Files:**
- Modify: `src-tauri/src/encoder/auto.rs`（tests）
- Modify: `CHANGELOG.md`

- [ ] **Step 1: 寫測試**

```rust
#[test]
#[ignore = "release check: ~minutes of encode+judge; run with --ignored before shipping"]
fn auto_mode_presets_smoke() {
    // §3 發版前驗證：三檔 preset（90 視覺無損 / 80 平衡 / 70 激進）對代表性
    // 來源只能是「過 effective gate 的 Encoded」或「skip」，不准 error。
    for target in [70.0, 80.0, 90.0] {
        for (name, ext) in [
            ("screenshot.png", ImageExt::Png),     // A 類合成 UI
            ("transparent.png", ImageExt::Png),    // A 類含 alpha
            ("landscape2.jpg", ImageExt::Jpeg),    // B1 真實相機 JPEG
            ("landscape.jpg", ImageExt::Jpeg),     // B2 低 bpp → 只走轉碼
            ("with_icc.jpg", ImageExt::Jpeg),      // B2 + ICC
        ] {
            let path = fixture(name);
            let baseline = decode(&path, ext).unwrap();
            let src_bytes = std::fs::metadata(&path).unwrap().len();
            let gate = effective_gate(ext, &path, src_bytes, &baseline, target);
            let o = EncodeOpts { target_quality: target, ..opts() };
            match encode_auto(&path, ext, &baseline, &o, src_bytes).unwrap() {
                Some(out) => {
                    let dec = decode(&out.tmp_path, ext).unwrap();
                    let ok = judge::pixels_identical(&baseline, &dec)
                        || judge::ssimulacra2_score(&baseline, &dec).unwrap() >= gate;
                    let _ = std::fs::remove_file(&out.tmp_path);
                    assert!(ok, "{name} @ S={target}: winner must clear gate {gate}");
                }
                None => {} // skip 是合法結果（沒有候選過得了門檻）
            }
        }
    }
}
```

- [ ] **Step 2: 跑滿（含 ignored）**

Run: `cd src-tauri && cargo test auto_mode_presets_smoke -- --ignored --nocapture`
Expected: PASS（耗時數分鐘屬正常）。

- [ ] **Step 3: CHANGELOG**（`### Added` 加一行）

```markdown
- **三 preset 發版前 smoke test** — `auto_mode_presets_smoke`（`#[ignore]`，發版前 `cargo test -- --ignored` 跑）：S=70/80/90 三檔對 A 類 / B1 / B2 / ICC 代表 fixtures 跑 auto 搜尋，結果只能是「過 effective gate 的 Encoded」或 skip；補上先前只驗過 S=80 的缺口
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/encoder/auto.rs CHANGELOG.md
git commit -m "test(encoder): preset smoke test across S=70/80/90

Loop findings were only validated at S=80; this ignored release-check
test runs all three UI presets over A/B1/B2/ICC fixtures.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: 收尾 — B 類 fixtures 入 repo、已知限制記錄、全套驗證

**Files:**
- Add: `src-tauri/tests/fixtures/compressed.jpg`, `jpg-as-png.png`, `web-section.png`（loop 產的 B 類樣本，目前 untracked）
- Modify: `src-tauri/tests/fixtures/README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: fixtures README 補三列**（表格尾端）

```markdown
| `compressed.jpg` | 1920×1281 | landscape2 重壓到低 bpp 的 B2 樣本（checked in）— 已壓 JPEG 的轉碼路徑驗證 |
| `jpg-as-png.png` | 576×384 | JPEG 另存成 PNG 的偽裝樣本（checked in）— blockiness 指紋已知極限案例（1.16 未過 1.25 門檻） |
| `web-section.png` | 1920×1164 | 真實網頁區段截圖（checked in）— 中型 UI 內容，webp 有損贏家的代表 |
```

- [ ] **Step 2: 已知限制記到 CHANGELOG**（`[Unreleased]` 區新開 `### Known limitations`，放在 Added 之後）

```markdown
### Known limitations
- **jpg-as-png（PNG 偽裝 JPEG）可能被當 A 類有損壓** — blockiness 指紋 1.16 未達觸發門檻 1.25，本批實測結果可接受（88.2% 省幅、過門檻），屬 rubric §1 第二道防線的已知極限；門檻調整待下輪 loop 收
```

- [ ] **Step 3: 全套測試**

Run: `cd src-tauri && cargo test`
Expected: 全 PASS（ignored 的 smoke test 已在 Task 4 跑過）。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/fixtures/compressed.jpg src-tauri/tests/fixtures/jpg-as-png.png src-tauri/tests/fixtures/web-section.png src-tauri/tests/fixtures/README.md CHANGELOG.md
git commit -m "chore(fixtures): track B-class corpus samples from the goal loop

compressed.jpg (B2 recompressed), jpg-as-png.png (disguised-lossy known
limit), web-section.png (mid-size UI) back the loop findings and future
S=70/90 validation runs.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## 範圍外（明確不做，避免 scope creep）

- **跨格式主輸出**（PNG 來源輸出 .webp 檔）：app 架構是 main 原地取代同名檔（commands.rs），換格式是產品層決策，0.2 不動。bench 的 webp@100 / png@100 對應表已以 knob guard 形式轉譯。
- **§4 工具改動**（bench 二分容錯、fast-first 排序、fingerprint 門檻、eval-transcode）：不擋 0.2，留給下輪 loop。
- **AVIF companion 的 ICC 守門**：companion 是額外產物、原檔（含 ICC）仍在，丟 profile 已記錄於 rubric §0.5；P0 規則只涵蓋 main 贏家。
- **S=90 / S=70 的 bench 語料庫重跑**：Task 4 的 smoke test 蓋住 app 行為；完整 bench 重跑屬 /goal loop 的工作。
