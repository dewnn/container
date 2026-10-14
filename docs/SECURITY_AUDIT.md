# Security audit

Review date: 2026-10-10 (v0.19.2 plus the Unreleased changes). Earlier review: 2026-09-23.

This document records security checks performed for CONTAINER. It is not a guarantee that software can never contain a vulnerability.

## Runtime behavior

- No telemetry, analytics or media-upload endpoint is implemented. Network access is limited to the signed GitHub updater, user-requested yt-dlp downloads and HTTPS thumbnail retrieval for analyzed links.
- Media processing is local through the verified FFmpeg and FFprobe programs included with Windows packages. Installed builds retain them in a versioned Local AppData runtime; Portable builds retain them beside the application executable.
- External programs are launched directly with argument arrays; CONTAINER does not construct commands through `cmd.exe`, PowerShell or a shell interpreter.
- Windows worker processes use `CREATE_NO_WINDOW`, preventing FFmpeg/FFprobe/yt-dlp console flashes without hiding their captured error/progress output. Workers are terminated on cancellation, timeout and application exit.
- Opener permissions reveal completed output files and permit only the exact official FFmpeg download URL used by the interface.
- Tauri permissions are limited to core defaults, file selection, window icon changes, the signed updater and the two opener actions above.
- The Content Security Policy blocks arbitrary remote scripts and connections.
- Downloader URLs must use HTTPS, reject credentials, control characters, localhost (including a trailing-dot form) and literal private, link-local, carrier-grade NAT, `0.0.0.0/8`, multicast, IPv4-mapped and NAT64 IPv6 addresses. Host names are not resolved before the check, so this is a first-hop guard rather than full DNS-rebinding protection. yt-dlp is launched with configuration and plugin loading disabled and never imports browser cookies.

## Filesystem behavior

- The user chooses input files through the system picker or drag and drop.
- Asset-protocol access starts empty and is granted at runtime only to user-selected, drag-and-dropped, Send To, and newly rendered files.
- Processing creates a unique output under `Downloads/CONTAINER Output/<category>`.
- Output cleanup reserves the job slot for its whole duration, so it cannot recycle files while a job is still writing between FFmpeg stages, and no job can start mid-cleanup.
- Values that enter FFmpeg filter graphs are escaped or allowlisted; SmartCut export resolution uses the same allowlist as the Processing Stack.
- Downloader thumbnails are held only in a single system-temporary file and removed immediately after the preview loads; the former thumbnail cache is cleaned on startup.
- Source media is not overwritten.
- Repository rules exclude build output, environment files, logs, FFmpeg executables and temporary files.

## Supply-chain controls

- pnpm lifecycle builds are limited to `esbuild`; unrestricted dependency build scripts are disabled.
- GitHub Actions are pinned to full commit hashes.
- CI and Windows packages use the tested FFmpeg 9.0.1 full build. FFmpeg and FFprobe are distributed as separate GPLv3 programs with their notices and source links included.
- CI downloads the official yt-dlp 2026.08.19 Windows executable from its immutable GitHub release and rejects it unless its SHA-256 is `66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a`. Its own and bundled third-party license texts ship with Windows packages.
- Update packages are signed with a dedicated Tauri updater key; only its public verification key is committed.
- Lockfiles are committed for pnpm and Cargo.
- The production npm dependency audit reported no known vulnerabilities on the review date.
- The Rust dependency audit reported no known vulnerabilities on the review date. It reports six unmaintained transitive crates; these are maintenance warnings rather than known vulnerabilities and remain tracked through Tauri's dependency tree.
- Dependabot's `glib` 0.18 advisory was reviewed and dismissed as not used: `cargo tree --target x86_64-pc-windows-msvc` confirms the affected Linux GTK dependency is absent from CONTAINER's Windows build graph.
- Optional camera-region analysis uses the bundled MIT-licensed YuNet model entirely in memory; sampled frames are piped from FFmpeg and are not written or uploaded.
- First-run encoder tuning uses locally generated benchmark media, removes its temporary directory after measurement, and caches only the selected profile and hardware/FFmpeg fingerprint.

## Repository checks

- Common API-token/private-key patterns: none found.
- Unignored files larger than 10 MB: none found.
- Frontend type/diagnostic check: 0 errors and 0 warnings across 170 files.
- Rust tests: 135 defined. On a Linux check with FFmpeg 6.1, 110 passed and the 20 real-media/Windows-path tests that target the bundled FFmpeg 9 build were not applicable; the full suite runs on Windows CI. The 5 opt-in real-media camera and VAD parity reports are ignored unless their media paths are supplied.
- Clean Windows release build and NSIS packaging: passed in the 2026-09-23 review; not re-run for this update.

Security reports should use GitHub's private vulnerability reporting and avoid attaching private media or personal paths.
