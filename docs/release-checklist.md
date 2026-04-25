# Release Checklist

## v0.1.0

### Done

- [x] refactor1–4 完成（Rust native encoder pipeline）
- [x] `docs/superpowers/` plan + spec committed to main

### Pending — manual tests required

- [x] **Tag-push smoke test**
      Push `v0.1.0-test`, confirm both Windows and macOS runners in `release.yml` produce artifacts and a draft release is created; then delete the tag + draft.

- [x] **macOS build**
      Never built or tested locally. CI matrix result is the first validation.

- [ ] **Clean-machine smoke test (Windows)**
      Copy `Hando-win-x64-v0.1.0.exe` to a machine without dev tools and double-click to verify the portable installer runs.
