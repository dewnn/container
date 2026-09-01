# Contributing

Thanks for helping improve CONTAINER.

## Before opening a pull request

1. Open an issue for large behavior or UI changes.
2. Keep media processing local and never overwrite the source file.
3. Preserve the Video, Audio and Image category separation.
4. Add or update English and Turkish UI text together.
5. Do not commit media samples, generated builds, FFmpeg executables or secrets.

## Local checks

```powershell
pnpm install --frozen-lockfile
pnpm check
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml
```

FFmpeg and FFprobe must be available on `PATH` for Rust builds. Hardware-only code paths must keep a software encoder fallback.

## Pull requests

- Explain the problem and the user-visible result.
- List the checks you ran.
- Include before/after screenshots for interface changes.
- Keep unrelated formatting or refactoring out of the same pull request.
- Only contribute code and assets that you have the right to license.

By submitting a contribution, you certify that you have the right to submit it under the repository's eventual project license. Until the license review is complete, maintainers may hold contributions that touch SmartCut-derived areas.
