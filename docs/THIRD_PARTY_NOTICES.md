# Third-party notices

CONTAINER uses open-source libraries listed in `package.json`, `pnpm-lock.yaml`, `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`. Their individual licenses remain in effect.

## FFmpeg

Windows release packages bundle FFmpeg and FFprobe 9.0.1 full build as separate programs. They are not relicensed by CONTAINER and remain under the GNU GPL version 3. The exact release build is produced by Gyan.dev; corresponding FFmpeg source and build information are available from the links below.

- Project: https://ffmpeg.org/
- License information: https://ffmpeg.org/legal.html
- Source: https://ffmpeg.org/download.html#get-sources
- Windows build links: https://ffmpeg.org/download.html#build-windows

## yt-dlp

Windows release packages bundle the official yt-dlp 2026.08.19 executable. yt-dlp is released under the Unlicense; its PyInstaller executable also contains components under the licenses reproduced in `yt-dlp-THIRD_PARTY_LICENSES.txt`. The bundled `yt-dlp.exe` SHA-256 is `66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a`.

- Project: https://github.com/yt-dlp/yt-dlp
- Release: https://github.com/yt-dlp/yt-dlp/releases/tag/2026.08.19
- Source: https://github.com/yt-dlp/yt-dlp/tree/2026.08.19

## Deno

Windows release packages bundle Deno 2.9.6 as the isolated JavaScript runtime used by yt-dlp for YouTube challenge processing. Deno is released under the MIT License reproduced in `deno-LICENSE.md`. The bundled Windows x64 archive SHA-256 is `15e5300b0ba3c3695a7621d90160a746ec9e710228cee639afa9d580f6e3cd11`.

- Project: https://github.com/denoland/deno
- Release: https://github.com/denoland/deno/releases/tag/v2.9.6
- Source: https://github.com/denoland/deno/tree/v2.9.6

## Geist and Geist Mono

The interface uses Geist and Geist Mono through Fontsource packages. The font families are distributed under the SIL Open Font License 1.1.

- Project: https://github.com/vercel/geist-font
- Packaging: https://fontsource.org/fonts/geist

## Animated Icons / Lucide

The landing-page output cleanup control adapts the animated Lucide `trash-2` SVG from Animated Icons. Animated Icons is ISC licensed and Lucide icons are ISC licensed.

- Animated Icons: https://github.com/gorkem-bwl/animated-icons
- Lucide: https://github.com/lucide-icons/lucide

## trash

CONTAINER uses the MIT-licensed Rust `trash` crate to move cleaned output to the operating system Recycle Bin instead of permanently deleting it.

- Project: https://github.com/Byron/trash-rs

## Silero VAD

SmartCut uses `silero-vad-rust` 6.2.2 and embeds its Silero V6 ONNX model for local speech detection. The crate and model retain their MIT licenses. The bundled model SHA-256 is `1a153a22f4509e292a94e67d6f9b85e8deb25b4988682b7e174c65279d8788e3`.

- Crate repository: https://github.com/sheldonix/silero-vad-rust
- Model project: https://github.com/snakers4/silero-vad

## Autocut attribution

The SmartCut interface and Silero-based voice-detection workflow were inspired by Mert Cobanov's Autocut project:

- Repository: https://github.com/cobanov/autocut

SmartCut is implemented within CONTAINER's own Rust and Svelte architecture; this attribution credits the original inspiration and does not relicense third-party code or assets.
