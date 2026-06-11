# Test Fixtures

| File | Dimensions | Purpose |
|---|---|---|
| `screenshot.png` | 1440×900 | Solid bands — exercises palette quantization |
| `transparent.png` | 512×512 | RGBA with alpha gradient |
| `tiny.png` | 128×128 | Solid gray — already near-optimal, should trigger SkippedNoGain |
| `corrupt.jpg` | n/a | JPEG SOI + garbage — must fail decode |
| `landscape.jpg` | 1920×1080 | Synthetic photo gradient — main JPEG test |
| `portrait_exif_rotated.jpg` | 800×600 pixels, EXIF orientation=6 | Tests EXIF rotation; displays as 600×800 |
| `large_photo.jpg` | 6000×4000 | Memory-pressure stress test (gitignored, regenerate with `cargo run --bin generate-fixtures`) |
| `with_icc.jpg` | 320×240 | Synthetic 3000-byte ICC profile in APP2 — ICC passthrough tests |
| `with_icc.png` | 320×240 | Same synthetic ICC profile in an iCCP chunk |
| `with_icc.avif` | 320×240 | Real Display P3 profile in a `colr`/`prof` box (macOS-generated, checked in — see note) |

Generated with `cargo run --example generate-fixtures --features generate-fixtures` from `src-tauri/`.

`with_icc.avif` is not produced by the generator; it was created once on macOS with
`sips -s format avif --embedProfile '/System/Library/ColorSync/Profiles/Display P3.icc' with_icc.png --out with_icc.avif`.
The source must stay small: sips tiles larger images into grid AVIFs, which `avif-decode` rejects.
