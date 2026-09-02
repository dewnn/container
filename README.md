<h1 align="center">CONTAINER</h1>

<p align="center">
  A local-first FFmpeg media toolbox for Windows.<br>
  Convert, edit, compress and smart-cut video, audio and images without uploading your files.
</p>

<p align="center">
  <a href="https://github.com/dewnn/container/releases/latest"><img alt="Release" src="https://img.shields.io/github/v/release/dewnn/container?label=release&color=blue"></a>
  <a href="https://github.com/dewnn/container/actions/workflows/ci.yml"><img alt="Build" src="https://github.com/dewnn/container/actions/workflows/ci.yml/badge.svg?branch=main"></a>
  <a href="https://github.com/dewnn/container/actions/workflows/ci.yml"><img alt="Tests" src="https://img.shields.io/badge/tests-15%20passing-brightgreen"></a>
  <a href="https://github.com/dewnn/container/releases/latest"><img alt="Windows" src="https://img.shields.io/badge/Windows-shipping-0078D4?logo=windows11&logoColor=white"></a>
</p>

<p align="center">
  <a href="#download">Download</a> ·
  <a href="#features">Features</a>
</p>

![CONTAINER Toolbox](docs/container-toolbox-v2.png)

## What is CONTAINER?

CONTAINER brings practical FFmpeg workflows into one desktop app. The source file is never overwritten: every operation creates a new result under `Downloads/CONTAINER Output/<category>`.

Processing happens locally. CONTAINER does not upload your video, audio or images to a server.

## Features

### Toolbox

- Context-aware Video, Audio and Image categories
- Ratio/crop, resize, FPS conversion, frame interpolation and frame blending
- Duplicate-frame removal, speed control and bitrate/quality controls
- Discord size-targeted compression and smart quality analysis
- Text overlays, color controls, noise, distortion and creative effects
- Cut, screenshot and GIF tools with visual timeline selection
- Audio extraction, replacement, removal and processing
- Social-media image crops (1:1, 4:5, 9:16, 16:9, 1.91:1, 2:3 and more) with lossless PNG or high-quality JPEG output
- Image conversion, scaling, quality reduction and before/after preview
- Hardware encoder detection with safe CPU fallback
- Batch workspace for repeated jobs

### SmartCut

- Local Silero V5 speech detection
- Automatic settings based on the selected recording
- Editable keep regions and cut-skipping preview
- Timeline navigation and external microphone/audio analysis
- MP4 export with quality and resolution presets
- FCPXML 1.11 export for DaVinci Resolve and Premiere-compatible workflows
- Linked camera/audio tracks aligned from embedded timecode

## Download

Windows builds are published on the repository's **Releases** page:

- `CONTAINER-Setup-<version>-x64.exe` — recommended installer
- `CONTAINER-Portable-<version>-x64.exe` — standalone app executable

The Setup build also adds **Send to → CONTAINER** to Windows Explorer. Right-click a video, audio file or image and send it directly to CONTAINER. The shortcut is removed when the app is uninstalled; the Portable build does not modify this menu.

## Install FFmpeg

CONTAINER requires both `ffmpeg` and `ffprobe` on `PATH`. Install a current **full build** using one of the following PowerShell commands:

### Winget — recommended

```powershell
winget install --id Gyan.FFmpeg --exact --accept-package-agreements --accept-source-agreements
```

### Chocolatey

Run PowerShell as Administrator:

```powershell
choco install ffmpeg-full -y
```

### Scoop

```powershell
scoop install ffmpeg
```

You can also use the [official FFmpeg Windows download page](https://ffmpeg.org/download.html#build-windows) for a manual installation. After installation, open a new PowerShell window and confirm that both commands work:

```powershell
ffmpeg -version
ffprobe -version
```

Restart CONTAINER after installation. The app checks both tools on every launch; if either one is missing, it displays a setup notice with a download link and a **Check again** action. FFmpeg 9.0.1 is the tested reference version.

The app is currently not Authenticode-signed. Windows SmartScreen may therefore show an “Unknown publisher” warning until Windows code signing is added.

## Updates

Installed builds automatically check for a new version in the background every time CONTAINER starts, without sending media or analytics. When an update exists, the update screen opens with its release notes and an install action; the **Update** button can also run a manual check. Update packages are verified with CONTAINER's dedicated Tauri updater key before installation.

`v0.2.1` moves the update channel out of Release assets, so it must be installed manually once when coming from `v0.2.0`. Installed releases after `v0.2.1` can update from inside the app. Portable builds should be replaced manually.

## Development

Requirements:

- Windows 10 or 11
- Node.js 22+
- pnpm 10+
- Rust stable with the MSVC toolchain
- Visual Studio Build Tools with Desktop development with C++
- FFmpeg and FFprobe available on `PATH`

```powershell
pnpm install --frozen-lockfile
pnpm check
pnpm build
pnpm tauri dev
```

The development environment also requires `ffmpeg` and `ffprobe` on `PATH`.

## Build a Windows release

```powershell
pnpm install --frozen-lockfile
pnpm tauri build --no-sign
```

To create a GitHub release, update the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, then push a matching tag. The release workflow signs the updater artifact and publishes only the installer and portable executable. The machine-readable updater manifest is maintained separately on the `updater` branch so it does not clutter Release assets.

## Project layout

```text
src/                  Svelte 5 interface
src/lib/              Toolbox, SmartCut and Batch workspaces
src-tauri/src/        Rust commands and FFmpeg orchestration
src-tauri/icons/      Application icons
.github/workflows/    CI and Windows release automation
docs/                 Screenshots and Turkish documentation
```

## Privacy and safety

- Media stays on your computer.
- Outputs are written to a new folder; the source is not modified.
- Hardware acceleration is used only when supported and falls back to CPU encoding.
- FFmpeg command failures are returned to the interface instead of silently replacing files.

The pre-publication review is documented in [docs/SECURITY_AUDIT.md](docs/SECURITY_AUDIT.md).

## Attribution and licensing status

The SmartCut workflow and interface were inspired by [cobanov/autocut](https://github.com/cobanov/autocut). See [third-party notices](docs/THIRD_PARTY_NOTICES.md) for dependencies and attribution.

This repository is being prepared for open-source publication, but an OSI license is intentionally not attached yet. The referenced Autocut repository currently contains no license file, so rights for any derivative SmartCut portions must first be clarified or those portions must be independently reimplemented. See the [open-source review](docs/OPEN_SOURCE_REVIEW.md).

## Author

Built by **dewn** — vibe-coded with Codex.
