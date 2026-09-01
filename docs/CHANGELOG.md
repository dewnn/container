# Changelog

All notable user-facing changes will be documented here.

This project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
