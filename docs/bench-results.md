# Parameter golf 結果（rubric §8.5）

> 量測：`cd src-tauri && cargo run --release --example bench -- grid`
> 機器：Apple Silicon（macOS），release build。2026-06-11。
> 完整 sweep（格式 × 品質階梯 × fixtures）：`cargo run --release --example bench`

## 結論（已落地的預設值變更）

| 旋鈕 | 舊值 | 新值 | 依據 |
|---|---|---|---|
| `png_oxipng_level` | 2（寫死） | **4（預設，可調 0–6）** | level 4 比 2 再省 8–28%，時間約 2 倍但每張仍 <2.5s；level 6 比 4 只再省 ~1% 卻要再 2 倍時間，不值 |
| `avif_speed` | 8（寫死） | **8（預設，可調 1–10）** | speed 6/4 跟 8 同 size 同分卻多花 60%+ 時間；speed 10 檔案大 41%。8 就是甜蜜點 |
| `webp_method` | 4（寫死） | **4（預設，可調 0–6）** | method 6 跟 4 size 幾乎相同卻更慢；method 2 大 ~5%。4 正確 |
| `jpeg_progressive` | 永遠開 | **true（預設，可關）** | 關掉（baseline profile）檔案大 ~3 倍但編碼快 2–9 倍；只對「要快不要小」的少數場景有用，預設維持開 |

## 關鍵數據

### oxipng level（PNG 無損路徑 q100）

| fixture | level 2 | level 4 | level 6 |
|---|---:|---:|---:|
| landscape | 28,885 B / 594ms | 25,530 B / 1,357ms | 25,204 B / 2,362ms |
| screenshot | 1,364 B / 66ms | **982 B** / 173ms | 982 B / 253ms |
| transparent | 4,499 B / 74ms | 4,153 B / 121ms | 4,151 B / 184ms |

### AVIF speed（landscape，q50）

| speed | bytes | ssimulacra2 | ms |
|---|---:|---:|---:|
| 4 | 3,410 | 73.77 | 2,233 |
| 6 | 3,410 | 73.77 | 2,071 |
| **8** | 3,411 | 73.57 | **1,315** |
| 10 | 4,820 | 77.22 | 623 |

### WebP method（landscape，q75）

| method | bytes | ms |
|---|---:|---:|
| 0 | 12,620 | 21 |
| 2 | 9,816 | 40 |
| **4** | **9,326** | 69 |
| 6 | 9,326 | 76 |

### JPEG progressive（q75）

| fixture | progressive | baseline |
|---|---:|---:|
| landscape | 14,776 B / 60ms | 44,374 B / 26ms |
| screenshot | 11,698 B / 37ms | 37,645 B / 4ms |

（修正插曲：原本 `progressive=false` 是 no-op——mozjpeg 預設本來就是 progressive；現在 false 走 `set_fastest_defaults()` baseline profile，開關才有真實效果。）

## 注意事項

- fixtures 是合成圖，偏「好壓」；實拍照片的絕對數字會不同，但**旋鈕之間的相對關係**（哪個值是甜蜜點）通常穩定。
- 換新的代表性圖片後重跑 `grid` 即可重新驗證，rubric 門檻不變。
