<h1><img src="src-tauri/icons/128x128.png" width="36" height="36" align="center" alt=""> Hando</h1>

A single-file desktop image optimizer for Windows and macOS. Drag images in, get smaller files out. No installer, no Node runtime, no companion folders.

**Language:** English UI

## Features

- **Drag-and-drop** — drop images onto the window or click to browse
- **JPEG, PNG, WebP, AVIF** — all four formats supported for both input and output
- **Lossless-aware** — skips encoding if the result would be larger than the original
- **Undo** — one-click restore; originals are moved to Trash, not deleted
- **Optional companion output** — export WebP or AVIF alongside the original format
- **Portable** — single executable, no installation required

## Demo

![Hando desktop screenshot](docs/demo.png)

## Download

Pre-built binaries are published in [Releases](../../releases). Single executable; double-click to launch.

| Platform | File |
|---|---|
| Windows x64 | `Hando-win-x64-v*.exe` |
| macOS Universal | `Hando-mac-universal-v*.app.zip` |

### macOS first launch

Hando is currently distributed without an Apple Developer ID signature, so on first launch macOS Gatekeeper will refuse to open it with: *"Apple cannot verify that 'Hando' is free of malicious software..."*

To allow it, run the following once in **Terminal**:

```bash
xattr -cr /path/to/Hando.app
```

Replace `/path/to/Hando.app` with the actual location, e.g. `~/Downloads/Hando.app` or `/Applications/Hando.app`. After this, double-click works as normal.

> **中文：** 由於 Hando 目前未通過 Apple 公證，首次開啟會出現「Apple 無法驗證『Hando』是否為惡意軟體」警告。請在「終端機」執行 `xattr -cr /Hando.app 的路徑`（例如 `~/Downloads/Hando.app`），之後即可正常使用。
>
> Apple Developer Program 收費 $99 美金/年。若你願意贊助以支援未來正式公證，請參考下方 [Sponsor](#sponsor) 區塊。

## Build from source

### Prerequisites

- **Rust** stable, ≥ 1.85.1
- **Node.js** 20+ (only for the desktop frontend dev server — not bundled into the app)
- Platform toolchain:
  - **Windows**: Visual Studio 2022 Build Tools (Desktop development with C++) + **NASM** (`winget install nasm`)
    - **Important**: Run `cargo` commands from the *"x64 Native Tools Command Prompt for VS 2022"* shell, or first invoke `vcvarsall.bat x64`. Otherwise `mozjpeg-sys` and `webp-sys` linker steps will fail with cryptic errors.
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`) + **NASM** (`brew install nasm`)

### Commands

```bash
npm install              # frontend deps
npm run tauri dev        # dev mode (Vite + Tauri)
npm run tauri build      # release build
npm run dist             # release build + rename + zip
```

Outputs land in `dist-final/`.

## Architecture

See [`docs/superpowers/specs/2026-04-25-rust-native-encoder-design.md`](docs/superpowers/specs/2026-04-25-rust-native-encoder-design.md) for the full design.

Briefly:
- WebView (TypeScript + Vite) for UI
- Tauri 2 (Rust) host
- In-process Rust encoders: `mozjpeg`, `oxipng`, `imagequant`, `webp`, `ravif`
- No sidecars, no native runtime dependencies

## Release process

1. Bump version in `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
2. Commit and tag: `git tag v0.2.0 && git push origin v0.2.0`.
3. GitHub Actions builds Windows + macOS-universal artifacts and creates a draft release.
4. Edit the draft release notes, then publish.

## Sponsor

Hando is open source and free. If it saved you time, a small tip helps cover Apple Developer Program fees ($99/year) so future builds can be properly signed and notarized — removing the macOS Gatekeeper warning for everyone.

[![ko-fi](https://img.shields.io/badge/Ko--fi-Support-FF5E5B?logo=ko-fi&logoColor=white)](https://ko-fi.com/homershie)

## License

Copyright (C) 2026 謝昇運 (homershie) <homerxworkshop@gmail.com>

This program is free software: you can redistribute it and/or modify it under the terms of the **GNU Affero General Public License v3.0 or later** (AGPL-3.0-or-later) as published by the Free Software Foundation. See [`LICENSE`](./LICENSE) for the full text.

AGPL's network clause means anyone who runs a modified version as a network service must also make their source available to users of that service. If that is incompatible with your use case, please open an issue to discuss commercial licensing.

### Third-party components

- [mozjpeg](https://github.com/mozilla/mozjpeg) — IJG (BSD-style)
- [oxipng](https://github.com/shssoichiro/oxipng) — MIT
- [imagequant](https://github.com/ImageOptim/libimagequant) — GPL-3.0 / commercial dual-licensed
- [libwebp](https://chromium.googlesource.com/webm/libwebp) — BSD-3-Clause
- [ravif / rav1e](https://github.com/xiph/rav1e) — BSD-2-Clause
- [Tauri](https://tauri.app) — MIT / Apache-2.0
- [trash (Rust crate)](https://crates.io/crates/trash) — MIT

`imagequant` is GPL-3.0; combining it with our AGPL-3.0-or-later codebase is permitted because AGPL-3.0 is a strict superset of GPL-3.0's obligations. The other dependencies' permissive licenses are AGPL-compatible as downstream components.
