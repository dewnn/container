<h1 align="center">CONTAINER</h1>

<p align="center">
  A local-first FFmpeg media toolbox for Windows.<br>
  Convert, edit, compress and smart-cut video, audio and images without uploading your files.
</p>

<p align="center">
  <a href="https://github.com/dewnn/container/releases/latest"><img alt="Release" src="https://img.shields.io/github/v/release/dewnn/container?label=release&color=blue"></a>
  <a href="https://github.com/dewnn/container/actions/workflows/ci.yml"><img alt="Build" src="https://github.com/dewnn/container/actions/workflows/ci.yml/badge.svg?branch=main"></a>
  <a href="https://github.com/dewnn/container/releases/latest"><img alt="Windows" src="https://img.shields.io/badge/Windows-shipping-0078D4?logo=windows11&logoColor=white"></a>
</p>

<p align="center">
  <a href="#download">Download</a> ·
  <a href="#features">Features</a>
</p>

## What is CONTAINER?

CONTAINER brings practical FFmpeg workflows into one desktop app. The source file is never overwritten: every operation creates a new result under `Downloads/CONTAINER Output/<category>`.

Processing happens locally. CONTAINER does not upload your video, audio or images to a server.

On Windows, closing the window with **X** keeps CONTAINER in the notification area so an active export or download can continue. Preview playback pauses while hidden. Left-click the tray icon to reopen; right-click for **Open CONTAINER**, **Check for Updates** (release builds), and **Exit**. Choose **Exit** to stop background work and fully quit.

## Features

### Toolbox

- Video, audio and image tools with favorites and batch processing
- Crop, resize, speed, quality and color controls
- Cut, screenshot, GIF, subtitle and overlay tools
- Clipper layouts with optional Kick/Twitch Social Tags
- Image editing and social-media crops
- Hardware encoding with a safe CPU fallback
- Work recovery and Output cleanup
- Built-in DWLNDR with bundled yt-dlp

### First-run encoder tuning

On first launch, CONTAINER quietly tests available CPU and GPU encoders and chooses the fastest option that meets its quality threshold. The result is reused until the hardware, driver or bundled FFmpeg changes. Test media stays local and is deleted afterward.

### Experimental camera-region detection

Clipper's Auto Camera is still experimental and can choose the wrong region, especially when the camera moves or multiple faces appear. Check its selection before exporting; manual placement remains available. Detection runs locally.

### SmartCut

- Local speech detection with editable keep regions
- Timeline preview and external audio analysis
- MP4 and FCPXML export, including linked camera/audio tracks

## Download

Windows builds are published on the repository's **Releases** page:

- `CONTAINER-Setup-<version>-x64.exe` — recommended installer
- `CONTAINER-Portable-<version>-x64.zip` — portable folder (extract before running)

Setup adds **Send to → CONTAINER** in Windows Explorer; Portable does not change this menu.

## FFmpeg included

Windows packages include FFmpeg and FFprobe; no separate installation or `PATH` setup is needed. Keep all Portable ZIP files together after extraction.

The app is currently not Authenticode-signed. Windows SmartScreen may therefore show an “Unknown publisher” warning until Windows code signing is added.

## yt-dlp included

Windows packages include yt-dlp for DWLNDR. If a website changes, yt-dlp can be updated from the DWLNDR screen.

## Updates

Installed builds check for verified updates without uploading media or analytics. Portable builds must be replaced manually.

## Privacy and safety

- Media stays on your computer.
- Outputs are written to a new folder; the source is not modified.
- Hardware acceleration is used only when supported and falls back to CPU encoding.
- FFmpeg command failures are returned to the interface instead of silently replacing files.

Security design and verification notes are documented in [docs/SECURITY_AUDIT.md](docs/SECURITY_AUDIT.md).

## License and attribution

CONTAINER is released under the [MIT License](LICENSE).

SmartCut's interface and Silero-based voice-detection workflow were inspired by [cobanov/autocut](https://github.com/cobanov/autocut). Bundled FFmpeg remains licensed separately under GPLv3. See [third-party notices](docs/THIRD_PARTY_NOTICES.md) for dependency licenses and source links.

## Author

Built by **dewn** — vibe-coded with Codex.
