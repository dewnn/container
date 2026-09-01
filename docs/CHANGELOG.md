# Changelog

All notable user-facing changes will be documented here.

This project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] - 2026-09-02

### Added

- Social Ratio / Crop for images, with common post, portrait, story, landscape and Pinterest ratios.
- Lossless PNG and high-quality JPEG output choices for social crops.

### Changed

- Discord Compressor's detailed quality explanation is now tucked into an optional help panel so its controls stay in focus.
- The update action now sits with language and theme controls.
- The start screen is cleaner: only the FFmpeg readiness indicator remains below the drop area.

## [0.2.2] - 2026-09-01

### Fixed

- The Original/Rendered image comparison handle now uses the full preview bounds, no longer jumps to an edge when pressed, and stays isolated from image panning.

## [0.2.1] - 2026-09-01

### Added

- A startup FFmpeg requirement dialog with an official download link and an in-app dependency recheck.
- Automatic background update checks on every launch; an available release opens its update screen immediately.

### Changed

- The updater manifest now lives on a dedicated updater branch, leaving GitHub Release assets limited to Setup and Portable executables. Moving from v0.2.0 requires one manual v0.2.1 installation.

## [0.2.0] - 2026-09-01

### Added

- Image previews now open without upscaling small files and support cursor-centered wheel zoom, left-button panning, zoom controls and fit reset.
- In-app update checks, release notes, signed downloads and restart installation through GitHub Releases.
- Clear startup guidance and dependency recheck when system FFmpeg or FFprobe is missing.

### Changed

- FFmpeg and FFprobe are no longer bundled; CONTAINER uses the full build installed on the user's `PATH`.
- Windows releases now contain a smaller Setup and standalone Portable executable.

## [0.1.0] - 2026-09-01

### Added

- Toolbox workspaces for video, audio and images.
- SmartCut speech detection and editable keep regions.
- Visual timeline controls for cutting, screenshots and GIF creation.
- Target-size compression with hardware detection and CPU fallback.
- Batch processing workspace.
- English/Turkish interface and light/dark themes.
- Windows installer and portable build support.
- Automated GitHub build checks and Windows release packaging.
