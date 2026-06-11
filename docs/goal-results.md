# Goal loop 結果表

- **日期**：2026-06-11
- **參數**：preset = 平衡（S=80）；corpus = `src-tauri/tests/fixtures`（迷你語料庫，流程驗證跑）
- **量測**：全部數字來自本 session 的 `bench eval` 實際輸出（68 發，每張 ≤12、守 §5 停止條件）。標「未量測」者為量測層邊界（B2-JPEG 無損轉碼不經 bench，見 goal.md §1.2）。
- **門檻**：ssimulacra2 ≥ S_eff、encode_ms ≤ 3000、解碼成功；無損候選另須逐像素相等。S_eff：A/B1 = 80；B2 非 JPEG／EXIF 旋轉／偽裝有損之有損候選 = 90。

| image | class（§3 分流） | winner | out_bytes | ratio | ssimulacra2 | encode_ms | 判定 |
|---|---|---|---:|---:|---:|---:|---|
| compressed.jpg | B2-JPEG（bpp 0.54） | —（app `optimize_lossless`） | 未量測 | 未量測 | n/a | n/a | 無損轉碼(B2-JPEG) |
| jpg-as-png.png | A（指紋 1.16 未觸發，rubric 已知極限） | avif@80 s8 | 57,044 | 88.2% | 80.27 | 413 | 有損贏 |
| landscape.jpg | B2-JPEG（bpp 0.25） | —（app `optimize_lossless`） | 未量測 | 未量測 | n/a | n/a | 無損轉碼(B2-JPEG) |
| landscape2.jpg | B1（bpp 2.59） | webp@87 m4 | 380,238 | 52.3% | 80.08 | 196 | 有損贏 |
| large_photo.jpg | B1（bpp 10.0，細顆粒） | 無通過候選（q85 僅 s16.1，且 4 發全破 3s） | — | — | ≤19.33 | 3202–8057 | 交 app 轉碼 / skip |
| portrait_exif_rotated.jpg | B2-EXIF（S_eff=90） | avif@95 s8 | 11,173 | 45.3% | 90.00 | 759 | 有損贏 |
| realphoto.png | A | avif@85 **speed 9** | 119,728 | 98.1% | 82.47 | 1579 | 有損贏 |
| screenshot.png | A | webp@100（無損） | 184 | 99.1% | 100（pixel-identical） | 9 | 無損贏 |
| tiny.png | A | webp@100（無損） | 44 | 54.2% | 100（pixel-identical） | 0 | 無損贏 |
| transparent.png | A | webp@100（無損） | 2,502 | 98.3% | 100（pixel-identical） | 85 | 無損贏 |
| web-section.png | A | webp@88 m4 | 203,452 | 82.4% | 81.83 | 121 | 有損贏 |
| with_icc.avif | B2-非JPEG（S_eff=90） | avif@87 s8 ⚠ | 1,982 | 2.8% | 90.31 | 128 | 有損贏（邊緣；AVIF 輸出丟 ICC，建議實務判 skip） |
| with_icc.jpg | B2-JPEG（bpp 0.65） | —（app `optimize_lossless`） | 未量測 | 未量測 | n/a | n/a | 無損轉碼(B2-JPEG) |
| with_icc.png | B-偽裝有損（blockiness 1.44） | png@100（無損） | 948 | 67.7% | 100（pixel-identical） | 47 | 無損贏 |
| corrupt.jpg | —（decode 失敗，input gate 正確擋下） | — | — | — | — | — | 淘汰於門檻 #1 |

## 值得記的淘汰案例（全部綁回實際 eval 輸出）

- **realphoto avif@85 speed 8**：s 82.47 過畫質、3063ms 破 T_lossy 63ms 被淘汰 → speed 9 重試 1579ms 過，成為贏家。
- **realphoto png@100**：無損合格但 3694ms 破 T_lossless → 無損名額由 webp@100（936ms, 1.88MB）遞補，最終仍輸給 avif。
- **web-section avif@85 speed 8**：s 83.34 過畫質、3433ms 破時間 → speed 9 過（1603ms, 211,999B）但仍比 webp@88（203,452B）大。
- **landscape2 avif 全格**：q60/q75 畫質不過（62.6 / 71.9）且 3.5–3.8s 破時間，雙殺。
- **門檻附近非單調**：landscape2 webp q88=79.03 不過、q87=80.08 過；jpg-as-png avif q78=79.86 不過、q80=80.27 過。
- **with_icc.avif avif@88/90**：過 90 門檻但輸出比原檔大（2106B / 2217B > 2040B），ratio 為負。

學到的 pattern 已寫入 [`goal-learnings.md`](goal-learnings.md)。
