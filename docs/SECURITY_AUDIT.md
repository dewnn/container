# Security audit

Review date: 2026-09-05

This document records security checks performed for CONTAINER. It is not a guarantee that software can never contain a vulnerability.

## Runtime behavior

- No telemetry, analytics or media-upload endpoint is implemented. Network access is limited to the signed GitHub updater, user-requested yt-dlp downloads and HTTPS thumbnail retrieval for analyzed links.
- Media processing is local through the verified FFmpeg and FFprobe programs included with Windows packages. Installed builds retain them in a versioned Local AppData runtime; Portable builds retain them beside the application executable.
- External programs are launched directly with argument arrays; CONTAINER does not construct commands through `cmd.exe`, PowerShell or a shell interpreter.
- Windows worker processes use `CREATE_NO_WINDOW`, preventing FFmpeg/FFprobe/yt-dlp console flashes without hiding their captured error/progress output. Workers are terminated on cancellation, timeout and application exit.
- Opener permissions reveal completed output files and permit only the exact official FFmpeg download URL used by the interface.
- Tauri permissions are limited to core defaults, file selection, window icon changes, the signed updater and the two opener actions above.
- The Content Security Policy blocks arbitrary remote scripts and connections.
- Downloader URLs must use HTTPS, reject credentials, control characters, localhost and literal private/link-local addresses. yt-dlp is launched with configuration and plugin loading disabled and never imports browser cookies.

## Filesystem behavior

- The user chooses input files through the system picker or drag and drop.
- Asset-protocol access starts empty and is granted at runtime only to user-selected, drag-and-dropped, Send To, and newly rendered files.
- Processing creates a unique output under `Downloads/CONTAINER Output/<category>`.
- Source media is not overwritten.
- Repository rules exclude build output, environment files, logs, FFmpeg executables and temporary files.

## Supply-chain controls

- pnpm lifecycle builds are limited to `esbuild`; unrestricted dependency build scripts are disabled.
- GitHub Actions are pinned to full commit hashes.
- CI and Windows packages use the tested FFmpeg 9.0.1 full build. FFmpeg and FFprobe are distributed as separate GPLv3 programs with their notices and source links included.
- CI downloads the official yt-dlp 2026.08.19 Windows executable from its immutable GitHub release and rejects it unless its SHA-256 is `66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a`. Its own and bundled third-party license texts ship with Windows packages.
- A weekly read-only upstream check compares the pinned yt-dlp and `ffmpeg-full` versions with their official release/package sources. It only synchronizes a maintenance issue; binary downloads, hash changes and releases remain deliberate, tested actions.
- Update packages are signed with a dedicated Tauri updater key; only its public verification key is committed.
- Lockfiles are committed for pnpm and Cargo.
- The production npm dependency audit reported no known vulnerabilities on the review date.
- Dependabot's `glib` 0.18 advisory was reviewed and dismissed as not used: `cargo tree --target x86_64-pc-windows-msvc` confirms the affected Linux GTK dependency is absent from CONTAINER's Windows build graph.

## Repository checks

- Common API-token/private-key patterns: none found.
- Unignored files larger than 10 MB: none found.
- Frontend type/diagnostic check: 0 errors and 0 warnings.
- Rust tests: 41 passed, 0 failed; the opt-in real-media VAD parity report is intentionally ignored unless its media path is supplied.
- Clean Windows release build and NSIS packaging: passed.

Security reports should use GitHub's private vulnerability reporting and avoid attaching private media or personal paths.
