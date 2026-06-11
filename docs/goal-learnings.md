# Goal loop learnings

> **一行摘要**：照片給 avif（2MP+ 用 speed 9 才過 3s 門檻）、UI/合成圖給 webp 無損、細顆粒高 bpp 照片有損全滅只能轉碼；ssimulacra2 在門檻附近非單調，二分搜尋要容錯。

（2026-06-11，S=80 平衡檔，corpus = src-tauri/tests/fixtures，14 張 68 發 eval）

**avif speed 8 在 ≥2MP 必破 T_lossy=3s，改 speed 9 時間砍半、體積只多 ~6%。**
realphoto（2MP）avif@85：speed 8 = 3063ms（破門檻 63ms 被淘汰）、speed 9 = 1579ms / +7KB，直接過。web-section 同樣 3433ms → 1603ms。照片類 2MP 以上一律從 speed 9 起手。

**內容類型決定贏家格式，rubric §5 的自動分流實測成立。**
照片內容 → avif（realphoto 98.1%、jpg-as-png 88.2%、portrait 45.3%）；中型 UI 截圖 → webp 有損（web-section@88 82.4%）；合成/平色圖 → webp@100 無損碾壓一切（screenshot 184B、transparent 2.5KB，比任何有損候選都小）。例外：with_icc.png 這種小圖 png@100（948B）反而比 webp@100（3168B，比原檔還大）小——小圖無損別只試 webp。

**ssimulacra2 對品質非單調，門檻附近 ±1 格會跳。**
landscape2：webp q88=79.03（不過）但 q87=80.08（過）。二分搜尋不能假設單調，碰到門檻附近要兩側各 probe 一格。

**細顆粒、高 bpp 的照片（large_photo，bpp 10）有損候選全滅。**
q75→q85 分數只從 5.27 到 16.10，離 80 天差地遠——ssimulacra2 重罰顆粒被抹掉；且 24MP 下每發 encode 3.2–8s 全破時間門檻。這類圖唯一出路是 JPEG 無損 DCT 轉碼（app 路徑），loop 看到「bpp 高 + 前兩發分數 <30」就直接收手別燒預算。

**AVIF 輸出丟 ICC，邊緣收益不值得。**
with_icc.avif 的 avif@87 過 90 門檻但只省 2.8%，又會丟 profile（ravif 只支援 nclx，rubric §0.5）——帶 ICC 的來源若贏家是 AVIF 且省幅 <10%，實務上建議判 skip。

**png oxipng4 在 2MP 照片級 PNG 上會破 T_lossless=3s。**
realphoto png@100 = 3694ms 被時間門檻淘汰；webp@100 無損 936ms 沒事。大 PNG 的無損候選優先派 webp@100。

**時間預算實測**：jpg/webp 每發 0.1–0.3s、avif（≤2MP, s8）1–3.8s、24MP 任何格式 3–8s。每張 12 發上限在 2MP 級素材約落在 10–25s，60s 預算夠；24MP 素材 4 發就吃掉 23s，candidate 順序要把快的（jpg/webp）排前面探路。
