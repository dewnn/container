# Security audit

Review date: 2026-09-02

This document records security checks performed for CONTAINER. It is not a guarantee that software can never contain a vulnerability.

## Runtime behavior

- No telemetry, analytics or media-upload endpoint is implemented. The only application network request checks the public GitHub Release updater manifest.
- Media processing is local through the system-installed `ffmpeg.exe` and `ffprobe.exe` programs.
- External programs are launched directly with argument arrays; CONTAINER does not construct commands through `cmd.exe`, PowerShell or a shell interpreter.
- Windows worker processes use `CREATE_NO_WINDOW`, preventing FFmpeg/FFprobe console flashes without hiding their captured error/progress output.
- Opener permissions reveal completed output files and permit only the exact official FFmpeg download URL used by the interface.
- Tauri permissions are limited to core defaults, file selection, window icon changes, the signed updater and the two opener actions above.
- The Content Security Policy blocks arbitrary remote scripts and connections.

## Filesystem behavior

- The user chooses input files through the system picker or drag and drop.
- Asset-protocol access starts empty and is granted at runtime only to user-selected, drag-and-dropped, Send To, and newly rendered files.
- Processing creates a unique output under `Downloads/CONTAINER Output/<category>`.
- Source media is not overwritten.
- Repository rules exclude build output, environment files, logs, FFmpeg executables and temporary files.

## Supply-chain controls

- pnpm lifecycle builds are limited to `esbuild`; unrestricted dependency build scripts are disabled.
- GitHub Actions are pinned to full commit hashes.
- CI uses the tested FFmpeg 9.0.1 full build. Release packages do not redistribute FFmpeg.
- Update packages are signed with a dedicated Tauri updater key; only its public verification key is committed.
- Lockfiles are committed for pnpm and Cargo.
- The production npm dependency audit reported no known vulnerabilities on the review date.
- Dependabot's `glib` 0.18 advisory was reviewed and dismissed as not used: `cargo tree --target x86_64-pc-windows-msvc` confirms the affected Linux GTK dependency is absent from CONTAINER's Windows build graph.

## Repository checks

- Common API-token/private-key patterns: none found.
- Unignored files larger than 10 MB: none found.
- Frontend type/diagnostic check: 0 errors and 0 warnings.
- Rust tests: 15 passed, 0 failed.
- Clean Windows release build and NSIS packaging: passed.

Security reports should use GitHub's private vulnerability reporting and avoid attaching private media or personal paths.
