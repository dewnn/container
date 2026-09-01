# Third-party notices

CONTAINER uses open-source libraries listed in `package.json`, `pnpm-lock.yaml`, `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`. Their individual licenses remain in effect.

## FFmpeg

Release packages bundle FFmpeg and FFprobe. The currently tested Windows build reports FFmpeg 9.0.1 with GPLv3-enabled configuration. FFmpeg is a separate program invoked by CONTAINER and is not relicensed by this project.

Redistributors must comply with the exact FFmpeg build's license and source-code offer requirements. Release notes should identify the FFmpeg provider/version and link to its corresponding source and license information.

- Project: https://ffmpeg.org/
- License information: https://ffmpeg.org/legal.html
- Current Windows build provider: https://www.gyan.dev/ffmpeg/builds/

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
