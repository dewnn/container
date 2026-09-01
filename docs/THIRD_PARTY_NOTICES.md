# Third-party notices

CONTAINER uses open-source libraries listed in `package.json`, `pnpm-lock.yaml`, `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`. Their individual licenses remain in effect.

## FFmpeg

Release packages do not bundle FFmpeg or FFprobe. CONTAINER invokes the user's system-installed copies as separate programs. CI and release verification currently use FFmpeg 9.0.1 full build; that software is not relicensed by this project.

Users and redistributors remain responsible for the license terms of the FFmpeg build they install separately.

- Project: https://ffmpeg.org/
- License information: https://ffmpeg.org/legal.html
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

The SmartCut workflow and interface were inspired by Mert Cobanov's Autocut project:

- Repository: https://github.com/cobanov/autocut

At the time of this review, that repository did not publish a `LICENSE` file or package-level license declaration. This notice is attribution only; it does not claim or imply a license grant from the upstream author. See `OPEN_SOURCE_REVIEW.md` before public release.
