# Security audit

Review date: 2026-09-01

This document records the checks performed before CONTAINER's first public source publication. It is not a guarantee that software can never contain a vulnerability.

## Runtime behavior

- No telemetry, analytics, updater, upload endpoint or application-level network client is implemented.
- Media processing is local through the bundled `ffmpeg.exe` and `ffprobe.exe` programs.
- External programs are launched directly with argument arrays; CONTAINER does not construct commands through `cmd.exe`, PowerShell or a shell interpreter.
- Windows worker processes use `CREATE_NO_WINDOW`, preventing FFmpeg/FFprobe console flashes without hiding their captured error/progress output.
- The only opener permission reveals a completed output in Windows Explorer.
- Tauri permissions are limited to core defaults, file selection, window icon changes and revealing output files.
- The Content Security Policy blocks arbitrary remote scripts and connections.

## Filesystem behavior

- The user chooses input files through the system picker or drag and drop.
- Processing creates a unique output under `CONTAINER Output/<category>` beside the input.
- Source media is not overwritten.
- Repository rules exclude build output, environment files, logs, FFmpeg executables and temporary files.

## Supply-chain controls

- pnpm lifecycle builds are limited to `esbuild`; unrestricted dependency build scripts are disabled.
- GitHub Actions are pinned to full commit hashes.
- Release FFmpeg is pinned to the tested 9.0.1 full build.
- Lockfiles are committed for pnpm and Cargo.
- The production npm dependency audit reported no known vulnerabilities on the review date.
- Dependabot's `glib` 0.18 advisory was reviewed and dismissed as not used: `cargo tree --target x86_64-pc-windows-msvc` confirms the affected Linux GTK dependency is absent from CONTAINER's Windows build graph.

## Repository checks

- Common API-token/private-key patterns: none found.
- Unignored files larger than 10 MB: none found.
- Frontend type/diagnostic check: 0 errors and 0 warnings.
- Rust tests: 15 passed, 0 failed.
- Clean Windows release build and NSIS packaging: passed.

Security reports should follow `SECURITY.md` and avoid attaching private media or personal paths.
