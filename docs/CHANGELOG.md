# Changelog

All notable user-facing changes will be documented here.

This project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.5.2] - 2026-09-02

### Changed

- The video preview now uses a dedicated canvas with a separate fixed control bar, keeping rotated media centered and removing the oversized overlay-style letterboxing.
- Image Transform now shares the video crop, social presets, live rotation, flips and output sizing controls, with lossless PNG/WebP and optional JPEG output.
- The separate Social Ratio / Crop image tool has been replaced by the unified Transform workflow.

## [0.5.1] - 2026-09-02

### Added

- A short two-note completion chime after successful Toolbox renders, SmartCut exports and completed Batch queues.
- A unified video Transform workspace with social crop presets, a draggable freeform crop frame, rotation, horizontal/vertical flip and output sizing.

### Fixed

- Long SHA-256 values now wrap inside the result card instead of widening the interface and creating a horizontal scrollbar.
- Free crop now starts at the complete video frame instead of an inset rectangle.
- Transform rotation and horizontal/vertical flips are now reflected live in the video preview and crop coordinates.

### Changed

- The TR/EN language selector now appears only on the landing screen; media workspaces keep only the theme controls in the top bar.
- Video playback controls now remain visible instead of hiding when the pointer leaves the preview.
- Ratio / Crop and Resize are now combined into one Transform tool so framing and output resolution are configured together.
- Transform no longer asks for a quality setting; it uses lossless H.264 video encoding and copies audio without re-encoding.
- Transform controls now begin directly below the tool title, and the parameter panel scrollbar is visually hidden while wheel scrolling remains available.

## [0.5.0] - 2026-09-02

### Added

- MIT license and matching package metadata for the public source repository.
- Decent, Bad, Terrible, Unbearable, Custom and Random profiles for Image Potatoify.

### Changed

- File previews now use runtime asset access granted only to selected, dropped, Send To and rendered files.
- External URL access is restricted to the official FFmpeg download page, and output reveal access is restricted to `Downloads/CONTAINER Output`.
- Removed obsolete publication notes, old screenshot sources and unused mobile, macOS and Microsoft Store icon variants from the Windows-only repository.
- Replaced the landing drop-zone glyph with a clearer themed upload icon.

## [0.4.2] - 2026-09-02

### Fixed

- The Setup executable now explicitly uses the existing black-background `logo-dark` CONTAINER icon instead of Windows' generic NSIS disc icon.

## [0.4.1] - 2026-09-02

### Changed

- Windows executables, installer and installed shortcuts now use the black CONTAINER mark on its light background.

## [0.4.0] - 2026-09-02

### Added

- QualityMuncher-style Decent, Bad, Terrible, Unbearable, Custom and Random Potatoify profiles.
- A Setup-managed “Send to → CONTAINER” Windows shortcut for opening media directly in the app.

### Changed

- Outputs now use `Downloads/CONTAINER Output/<category>` instead of creating a folder beside each source file.
- The Windows title bar now shows only `CONTAINER`.

### Fixed

- Improved update-dialog title contrast in dark mode.

## [0.3.1] - 2026-09-02

### Fixed

- Restored the short “one file. every tool.” start-screen motto while keeping the longer promotional and status copy removed.

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
