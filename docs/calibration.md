# S 值校準（rubric §6 / §8.2）

> 狀態：**已人眼定案（2026-06-11，homer）：90 / 80 / 70 成立。**
> 觀察：可見差異約從 s ≈ 70–75 開始——90/80 落在「看不出差」這側，70（激進檔）踩在可見線上，符合各檔定位。
> 本次定案基於合成 fixtures；實拍照片複核仍列待辦（見 rubric「後續」），複核後若要改只動 `PRESET_TARGETS` 與本檔。
> 產生方式：`cd src-tauri && cargo run --release --example bench -- calibrate ../docs/calibration`
> 樣本在 `docs/calibration/`（gitignored），檔名格式 `主題_格式_q品質_s分數.副檔名`，與 `*_ORIGINAL.*` 並排看。

## 怎麼看

1. 打開 `docs/calibration/`，把 `landscape_ORIGINAL.jpg`（照片）和各等級樣本並排比。
2. 找到「開始看得出差」的那一張，它檔名裡的 `s` 分數就是你的下限。
3. `screenshot_*`（文字/UI）同樣做一次——文字邊緣的 ringing 比照片更早露餡，最終 S 取兩者中較嚴的。
4. 把定案值填回 `docs/rubric.md` §6 與 `src/ui/settings.ts` 的 `PRESET_TARGETS`（preset → S 的對照表在前端）。

## 量測到的階梯（2026-06-11，合成 fixtures）

照片（landscape.jpg, 1920×1080）關鍵點：

| 候選 | ssimulacra2 | 大小 |
|---|---:|---:|
| jpg@q90 | 92.0 | 20.7 KB |
| jpg@q85 | 90.1 | 18.0 KB |
| jpg@q75 | 89.4 | 14.8 KB |
| jpg@q70 | 77.9 | 13.9 KB |
| avif@q95 | 90.2 | 15.1 KB |
| avif@q85 | 85.4 | 7.3 KB |

文字截圖（screenshot.png, 1440×900）關鍵點：

| 候選 | ssimulacra2 | 大小 |
|---|---:|---:|
| avif@q90 | 91.6 | 1.8 KB |
| avif@q75 | 90.2 | 1.4 KB |
| webp@q85 | 90.1 | 3.5 KB |
| webp@q50 | 84.3 | 4.4 KB |

觀察：
- JPEG 在 q70→q75 之間有個明顯的分數懸崖（77.9 → 89.4）——q75 預設值剛好站在崖上，蠻幸運的。
- 合成 fixtures 偏「好壓」，真實照片的分數會更低一些；正式校準建議再丟幾張實拍照片進 `tests/fixtures/` 重跑。

## 定案 preset 值（程式內 `QualityPreset`）

| Preset | S | 依據 |
|---|---:|---|
| 視覺無損（visually lossless） | **90** | ssimulacra2 作者定義的 90+ ≈ 肉眼不可辨；Hando 會取代原始檔，預設要保守。人眼複核：遠在可見線（~70–75）之上 |
| 平衡（balanced） | **80** | 高品質；人眼複核：仍在可見線之上，實際看不出差 |
| 激進（aggressive） | **70** | 中高品質，可感知但不破相；人眼複核：踩在可見線上，符合定位 |

> 2026-06-11 由 homer 人眼並排複核定案（合成樣本）。實拍照片複核後若要改，只動 `src/ui/settings.ts` 的 `PRESET_TARGETS` 和本檔。
