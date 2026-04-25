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
| `with_icc.jpg` | — | ICC profile test (requires manual sourcing — no test uses it currently) |

Generated with `cargo run --bin generate-fixtures` from `desktop/src-tauri/`.
