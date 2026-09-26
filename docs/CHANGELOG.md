# Changelog

All notable user-facing changes will be documented here.

This project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.18.4] - 2026-09-26

- Fixed black, unplayable previews when reopening projects saved outside their media folder, including earlier Go Back stages.

## [0.18.2] - 2026-09-26

- Fixed full-length Cut Video exports when the displayed end time rounds past the source duration.
- Added a direct path from completed downloads to the video timeline and a way back to the current editor.
- Improved Windows output recycling when a file is locked and brought an existing window forward on a second launch.

## [0.18.1] - 2026-09-25

- Refresh Windows desktop and Start menu icons during installation and updates, avoiding the cached old logo.

## [0.18.0] - 2026-09-25

- Refreshed CONTAINER branding and logo across the editor, app icon, installer and project files.
- Simplified SmartCut around automatic silence detection, with re-detect and fine-tuning controls.
- Expanded the SmartCut timeline and improved resizable panel behavior.
- Refined typography, light-theme accents and Turkish interface text across Toolbox, SmartCut and Batch.
- Added a Windows notification-area menu: closing the window preserves the session and active jobs, while Exit fully quits.

## [0.17.0] - 2026-09-24

- Added resizable panels with remembered widths in Toolbox, SmartCut and Batch, plus a theme-matched reset confirmation.
- Improved text and image overlay resizing, preserved text wrapping when scaling, and added multiline text editing with compact layer removal.
- Added emoji text export for video and images.
- Improved Social Tag alignment and camera-relative previews for plain and boxed styles.
- Fixed media-specific favorite counts and spacing in image crop controls.
- Simplified the editor header and prevented panel controls and confirmation dialogs from activating player shortcuts.

## [0.16.0] - 2026-09-24

- Added Go Back history after Continue Editing.
- Added more Social Tag positions and improved small logos.
- Improved interface readability, colors and compact layouts.
- Fixed output continuation, Batch controls, project relinking and special-character exports.

## [0.15.0] - 2026-09-23

- Added Kick/Twitch Social Tags to Clipper.
- Improved experimental Auto Camera boundaries.
- Improved project opening, recovery and missing-file warnings.
- Fixed compact editor layouts and streamlined the header.
- Corrected the Windows installer publisher to `dewn`.

## [0.14.1] - 2026-09-23

- Added experimental Auto Camera to Clipper.
- Kept DEV builds separate from production updates.

## [0.14.0] - 2026-09-23

- Improved interface readability and Cut, GIF and overlay timelines.
- Improved crash recovery and project-file opening.
- Added automatic encoder tuning and experimental camera detection.

## [0.13.0] - 2026-09-20

- Added optional Clipper watermarks and improved preview/export consistency.
- Improved Cut Video timeline selection and time entry.

## [0.12.0] - 2026-09-19

- Added Vertical Clipper and project save/open support.
- Improved font, HEIC/HEIF and image previews.
- Improved editing controls and updated a security dependency.

## [0.11.1] - 2026-09-07

- Fixed TikTok downloads and improved DWLNDR's quality options and layout.
- Added download completion feedback and matched the Windows title bar to the app theme.

## [0.11.0] - 2026-09-06

- Added image compression and expanded image editing tools.
- Improved image export, downloader reliability and job cancellation.
- Bundled Deno for downloader compatibility.

## [0.10.0] - 2026-09-06

- Added video merging, stabilization, subtitles, image overlays and blur/pixelate regions.
- Improved downloader navigation and media test coverage.
- Removed Duplicate Frame Removal, Deep Fry and Video Corruption.

## [0.9.5] - 2026-09-05

- Simplified release updates and downloader thumbnail cleanup.
- Removed the scheduled media-tool version watcher.

## [0.9.4] - 2026-09-05

- Added media-tool version checks and centralized verified build versions.

## [0.9.3] - 2026-09-05

- Added DWLNDR with bundled yt-dlp and improved download controls.
- Added clearer error notifications and improved security and compact layouts.

## [0.9.2] - 2026-09-05

- Reduced update downloads while keeping full Setup and Portable packages self-contained.
- Improved dark theme and Output cleanup.

## [0.9.1] - 2026-09-05

- Fixed SmartCut detection in installed and portable builds.
- Improved work recovery, Cut Video controls and compact layouts.
- Added one-click Output cleanup through the Recycle Bin.

## [0.9.0] - 2026-09-05

- Bundled FFmpeg and FFprobe with Windows downloads.
- Added work recovery and tool favorites.
- Improved compact-window and display-scaling support.

## [0.8.0] - 2026-09-04

- Added local Silero speech detection, editing controls and app-wide undo/redo.
- Improved color, text and compression tools.
- Improved SmartCut accuracy, exports and timeline behavior.
- Improved OBS capture and compact layouts.

## [0.7.0] - 2026-09-03

- Expanded Color Adjustment and multi-layer Text overlays.
- Added Windows font matching and optional VMAF quality analysis.
- Simplified tool organization and removed duplicate controls.

## [0.6.1] - 2026-09-03

- Made Cut Video lossless by default and improved SmartCut detection.
- Improved Transform controls and theme consistency.
- Removed the browser-style right-click menu.

## [0.6.0] - 2026-09-02

- Added standard Upscale targets.
- Fixed rotated previews, video-player sizing and Transform controls.

## [0.5.2] - 2026-09-02

- Improved video previews and unified image/video Transform controls.

## [0.5.1] - 2026-09-02

- Added completion sounds and a unified video Transform workspace.
- Improved preview controls, cropping and compact layouts.

## [0.5.0] - 2026-09-02

- Added the MIT license and Image Potatoify profiles.
- Tightened file access and cleaned up unused repository assets.

## [0.4.2] - 2026-09-02

- Fixed the Windows Setup icon.

## [0.4.1] - 2026-09-02

- Updated Windows app and installer icons.

## [0.4.0] - 2026-09-02

- Added Image Potatoify profiles and Windows “Send to → CONTAINER”.
- Moved outputs to `Downloads/CONTAINER Output`.
- Improved the title bar and update dialog.

## [0.3.1] - 2026-09-02

- Restored the short start-screen motto.

## [0.3.0] - 2026-09-02

- Added social image crops and PNG/JPEG output choices.
- Simplified the start screen and compression controls.

## [0.2.2] - 2026-09-01

- Fixed the image comparison handle.

## [0.2.1] - 2026-09-01

- Added FFmpeg setup guidance and automatic update checks.
- Moved updater metadata to a dedicated branch. Updating from v0.2.0 requires one manual installation.

## [0.2.0] - 2026-09-01

- Added image zoom/pan previews and in-app updates.
- Switched to system-installed FFmpeg for smaller Windows downloads.

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
