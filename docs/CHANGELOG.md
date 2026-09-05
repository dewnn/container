# Changelog

All notable user-facing changes will be documented here.

This project follows [Semantic Versioning](https://semver.org/).

## [0.9.5] - 2026-09-05

- Removed the separate automatic-update executable from GitHub Releases; signed in-app updates now use the normal Setup package.
- Downloader thumbnails now use a short-lived system temporary file that is removed as soon as the preview loads, and the former thumbnail cache is cleaned on startup.
- Removed the scheduled FFmpeg/FFprobe and yt-dlp version watcher while retaining centrally pinned, checksum-verified build versions.

## [0.9.4] - 2026-09-05

- Added a weekly supply-chain check for newer FFmpeg/FFprobe and yt-dlp releases without silently replacing unverified binaries.
- Centralized the tested media-tool versions and yt-dlp checksum so CI, release packaging and the Windows runtime remain aligned.

## [0.9.3] - 2026-09-05

- Added the secure DWLNDR workspace with link analysis, thumbnail previews, practical quality and codec choices, and live percentage, transferred-size and speed feedback.
- Bundled a checksum-verified official yt-dlp build so DWLNDR works without a separate executable download, while retaining manual replacement for future compatibility.
- Downloads now stop with the application, cancellation removes only newly created partial files, and the highest available quality is selected automatically.
- Added short, dismissible and user-friendly error notifications across Toolbox, SmartCut, Batch and DWLNDR.
- Hardened downloader URL and thumbnail validation, restored Windows SmartScreen protection, narrowed folder permissions and consolidated the HTTPS client dependency.
- Improved progress-bar alignment and compact-window layout behavior.

## [0.9.2] - 2026-09-05

- Separated the verified FFmpeg runtime from automatic app updates, reducing subsequent Windows update downloads from about 130 MB to about 11 MB while keeping full Setup and Portable packages self-contained.
- Improved the editor’s dark theme, typography and compact process display.
- Output cleanup now clears generated contents while retaining the `CONTAINER Output` folder.

## [0.9.1] - 2026-09-05

- Fixed the bundled Silero model so SmartCut detection works in installed and portable builds.
- Work recovery now appears only after an unexpected shutdown and is readable in both themes.
- Fixed Cut Video timeline handles jumping after a drag; added fine Shift-drag and keyboard adjustment.
- Hidden browser-style scrollbars while keeping wheel, touchpad and keyboard scrolling.
- Added safe one-click cleanup for `Downloads/CONTAINER Output` using the Windows Recycle Bin.
- Improved compact project-header and SmartCut panel layouts.

## [0.9.0] - 2026-09-05

- FFmpeg and FFprobe are now included with Windows downloads; no separate setup or PATH configuration is needed.
- Added automatic work recovery for Toolbox, SmartCut and Batch sessions.
- Added persistent tool favorites with a quick Favorites filter.
- Improved compact-window and Windows display-scaling support.

## [0.8.0] - 2026-09-04

### Added

- Silence Cutter now uses the fully local Silero V6 model with Natural, Balanced, Tight and automatic editing presets.
- Added separate Minimum Pause, Keep Before Speech and Keep After Speech controls with conservative waveform boundary refinement.
- Added application-wide undo and redo with `Ctrl+Z` and `Ctrl+Shift+Z` across Toolbox, Silence Cutter and Batch workflows.
- Color Adjustment now includes quick looks and a before/after preview; Text overlays support outline, shadow and background styling.
- Quality / Compression now offers approachable High Quality, Balanced and Small File profiles alongside advanced controls.

### Changed

- Silence Cutter keeps one shared speech segmentation engine for manual, preset and Auto modes, while keeping technical detection controls under Advanced.
- Timeline frame generation is faster and keeps the preview geometry stable when switching tools; timeline hover now shows the target timestamp.
- Slider fills and handles now follow the active light or dark theme consistently.

### Fixed

- SmartCut export now normalizes duplicate, contained, overlapping and touching source ranges before rendering, preventing replayed audio or video around cuts.
- MP4 SmartCut exports now trim audio and video from one source through an exact timestamp-reset concat graph instead of keyframe-sensitive concat-demuxer ranges.
- Short, high-confidence words and interjections are preserved while natural micro-pauses remain intact.
- SmartCut timestamps are displayed cleanly without floating-point artifacts, and preview, timeline and export share the same normalized ranges.
- CONTAINER can be captured by OBS Window Capture, and the Show Output action remains visible in compact window layouts.

## [0.7.0] - 2026-09-03

### Added

- Color Adjustment now provides live brightness, contrast, saturation, gamma, hue, temperature, sharpening, blur, denoise, deband, vignette, grayscale and deinterlace controls.
- Text overlays now support multiple independently selectable, draggable and resizable layers with per-layer font, color, size and opacity controls.
- Installed Windows fonts are detected automatically and used consistently in the preview and rendered output.
- Quality / Compression now includes optional VMAF analysis, a recommended CRF result and one-click recommendation application.

### Changed

- Text colors now use a compact preset palette and editable HEX value instead of the native browser color field.
- CRF guidance now clearly distinguishes mathematically lossless CRF 0 from visually high-quality values such as 16 and 17.
- File Hash and Fix Timestamps are grouped under Utilities, with Utilities placed last in the Image tool list.
- Unapplied Color Adjustment and Text workspace changes reset when leaving the tool; rendered results become the active working source.

### Removed

- Removed the duplicate Convert to CFR tool, the Negate tool and the separate Smart Quality Analysis sidebar entry.
- Removed the explanatory and recommendation cards from the compact Text and Color Adjustment parameter panels.

## [0.6.1] - 2026-09-03

### Changed

- Cut Video now defaults to fast lossless stream copying, preserves the source container and keeps frame-accurate re-encoding as an optional mode.
- SmartCut automatic detection now smooths brief probability spikes, keeps words together across short dips and uses more stable automatic silence and padding values.
- Transform controls use larger, clearer labels and a cyan selection color.

### Fixed

- Removed the browser-style right-click menu that exposed Back, Refresh, Save As and Print commands.
- Light and dark themes now apply before the interface is painted, eliminating mixed-theme frames during startup and switching.
- Theme logos are preloaded and the in-app logo switches without waiting for another image load.
- Transform crop borders and resize handles remain fully visible inside the preview, including when the media fills one axis.
- Reduced the crop border thickness for a cleaner preview.

## [0.6.0] - 2026-09-02

### Added

- Added a dedicated Upscale category that detects the source orientation and offers only larger standard 720p, 1080p, 2K, 4K and 8K targets with aspect-ratio preservation, unchanged frame rate and high-quality Lanczos scaling.

### Fixed

- 90° and 270° live previews now size the original media box correctly instead of shrinking or misaligning rotated videos and images.
- The redesigned video control bar now spans the full player width.
- The video canvas now stretches to the complete preview area instead of collapsing around its initial media size.
- SmartCut video and its controls now fill the complete player area instead of collapsing into a small centered box.
- Transform previews refit immediately when their canvas is resized, and Horizontal/Vertical flips now follow the visible axes after rotation.
- Transform controls now use proper light-theme surfaces, text and active-state colors for both video and image tools.
- File Hash is listed last in the Image tool menu.

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
