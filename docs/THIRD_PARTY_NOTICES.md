# Third-party notices

CONTAINER uses open-source libraries listed in `package.json`, `pnpm-lock.yaml`, `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`. Their individual licenses remain in effect.

## FFmpeg

Windows release packages bundle FFmpeg and FFprobe 9.0.1 full build as separate programs. They are not relicensed by CONTAINER and remain under the GNU GPL version 3. The exact release build is produced by Gyan.dev; corresponding FFmpeg source and build information are available from the links below.

- Project: https://ffmpeg.org/
- License information: https://ffmpeg.org/legal.html
- Source: https://ffmpeg.org/download.html#get-sources
- Windows build links: https://ffmpeg.org/download.html#build-windows

## Geist and Geist Mono

The interface uses Geist and Geist Mono through Fontsource packages. The font families are distributed under the SIL Open Font License 1.1.

- Project: https://github.com/vercel/geist-font
- Packaging: https://fontsource.org/fonts/geist

## Silero VAD

SmartCut uses `voice_activity_detector` 0.2.1 and its embedded Silero V5 ONNX model for local speech detection. The crate ships with the MIT License and attributes Nicholas Keenan (2024). Its runtime dependencies retain their respective licenses.

- Crate repository: https://github.com/nkeenan38/voice_activity_detector
- Model project: https://github.com/snakers4/silero-vad

## Autocut attribution

The SmartCut interface and Silero-based voice-detection workflow were inspired by Mert Cobanov's Autocut project:

- Repository: https://github.com/cobanov/autocut

SmartCut is implemented within CONTAINER's own Rust and Svelte architecture; this attribution credits the original inspiration and does not relicense third-party code or assets.
