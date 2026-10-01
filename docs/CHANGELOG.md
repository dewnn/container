# Changelog

All notable user-facing changes will be documented here.

This project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.19.1] - 2026-10-02

- Hardened release validation by compiling the native harness before real-media browser tests; made font fixtures portable across Windows and Linux. The v0.19.0 build attempt was not published.

- Added Processing Stack with editable, reorderable steps and combined rendering. Compatible SmartCut/Clipper chains use a fused FFmpeg graph to reduce intermediate encoding; source-time cuts and overlay geometry are preserved.
- Improved automatic AMD, NVIDIA, Intel and CPU encoder profiling and fallback contracts. NVIDIA and Intel profiles remain unverified on physical hardware in this development cycle; performance depends on the workload and device.
- Made Source history available for the current source and restored selected Stack steps, source settings and Cut Video filmstrip resources when returning to earlier sources.
- Improved Blur playback with a single-decoder backdrop and clarified Stack result versus tool preview playback.
- Simplified Frame Extractor around the current player position, frame count and spacing, with examples and guidance.
- Added live global accent colors, refined light-theme contrast and interface typography, and kept engine availability indicators independent of the selected accent.
- Moved Reset panel layout into General settings, compacted SmartCut controls, aligned landing status in the bottom action row and centered media metadata on wide screens.
- Bundled Montserrat ExtraBold Italic (800) for Text preview and export without installing a Windows font.
- Corrected Remove Audio output categorization, improved no-audio guidance, and fixed English tool hints and language switching.
- Hardened job cancellation, stale responses and atomic project saves, with expanded source-history, geometry, media and responsive-interface regression coverage.

- PNG compression now embeds a local lossless optimizer and an optional quality-guarded palette encoder. PNG target sizes are no longer blocked in Toolbox or Batch; unreachable targets save the best result without resizing or forcing severe degradation. Colour profiles, alpha endpoints, 16-bit samples and APNG frames are covered by regression tests.

- Fixed Image Compressor size-summary layout, explained lossless formats, blocked unsupported target sizes before rendering, and added retryable/stale-safe trial-encode estimates. All still-image output formats now share the `image` output folder across tools.
- Color sliders now work directly without enable checkboxes, with neutral displays, individual resets and undo; refined the link-download button to a balanced rectangular control.

- Added global language/theme/completion-alert preferences without closing the current media; compacted tool headings and clarified multi-video import, link downloads, empty searches and empty queues.
- Unified output action wording, added Play/Open to SmartCut and Downloader, and kept completed Batch outputs playable/revealable while later jobs run. Active-row cancellation now has its own label and stop icon.
- Renamed SmartCut's default quality to Very high / Çok yüksek and ordered quality choices without changing encoding settings; clarified Text's Add from preset action and improved keyboard focus and Turkish player labels.

- Clipper Watermark now accepts Text presets, provides compact font/size/color/opacity controls, and supports dragging/resizing in output preview. Advanced preset styling is preserved and composited in the same Clipper render.

- Added Text preset update/rename actions without expanding the default compact controls; duplicate names no longer create extra presets.
- Added estimated remaining time to Toolbox, SmartCut and the current Batch file, plus opt-in completion messages/taskbar alerts and Batch outcome counts.
- Render failures now show actionable disk-space, file-lock, missing-file and permission guidance with expandable technical details.
- Expanded font pixel checks to Impact, Arial and Georgia in portrait/landscape media, and Clipper encoded-output comparisons to 31% banner distance.

- Isolated Text preview typography from UI OpenType alternates, fixing changed letter shapes and widths in fonts such as Impact.

- Text presets now remember source width and scale font, outline, shadow and padding for different video resolutions. Legacy presets retain their saved pixel size and explain how to enable scaling.

- Added persistent Text presets that save the selected layer's text, position, font and appearance, and apply it as a new editable layer.

- Fixed gray Kick banner backgrounds on full-range video by converting the pixel range before drawing; preview/export tests now include the encoded MP4.
- Text and Image / Logo Overlay now encode video losslessly to avoid accumulating compression loss while editing. Intermediate files may be larger.

## [0.18.7] - 2026-09-28

- Opening or dropping multiple videos now enters Batch directly with every file queued; a single file still opens the editor.
- Added Change FPS to Batch, with lossless encoding of retained frames by default and a clear warning about discarded frames and file size.
- Batch now shows source metadata, blocks incompatible media before processing, offers a smaller high-quality FPS mode, retries failed rows without resetting successful results, and opens or reveals each completed output.
- Refined Batch output actions with consistent Play, folder, edit and remove icons across light and dark themes.
- Batch now starts with Convert Audio for audio files and Image Compressor for images, while preserving Encode as the video default.
- Batch operation choices now follow the loaded media type, hiding video-only actions for audio and images and image-only actions for video.
- Aligned Process actions with the status and Play Render line, keeping frame, speed and elapsed metrics centered.
- Clipper preview/export regression tests now generate their own real video fixture and compare Split, Squares and Freecam at 0% and 16% banner distance.
- Clipped the Kick.com prefix to the banner bar to remove a one-pixel black edge in preview and export; verified the embedded banner art and font regenerate from the app cache.

- Added an embedded Kick.com banner with reference-matched artwork and typography, proportional sizing, and space for social-platform controls beside long usernames.
- Added an output-layout preview for Split, Squares and Freecam, with a separate Source Regions view for camera/content selection. Fixed Social Tag preview/export placement differences.
- Added resettable Social Tag distance controls to every Clipper layout and all three tag styles. Boxed badges now stay centered by default.
- Added mouse-wheel zoom and drag framing to Original Size, Blur and Fill. Original Size and Blur retain their foreground frame while zooming, with center alignment guides and a zoom reset.
- Added subtly rounded Freecam camera corners and replaced per-frame corner calculations with a reusable mask. Camera placement controls now have resets.
- Kept Clipper defaults within the selected layout, moved Split advanced settings below overlays, and improved slider undo behavior.
- Added Play Render beside the process status to open the last output in the default player.
- Moved DWLNDR's Back button beneath downloads and balanced its icon/text spacing.

## [0.18.6] - 2026-09-27

- Separated runtime caches and user data from the Windows installation folder. Existing download history and interrupted-session detection migrate without losing records; embedded Social Tag/model copies are regenerated in the dedicated cache.
- Embedded the Windows application and CPROJ document icons in the executable, removing loose ICO files from new installations and migrating existing shortcuts on upgrades.
- Consolidated bundled notices into one `licenses` folder and development tools/tests under `tooling`, without removing regression coverage.

## [0.18.5] - 2026-09-26

- Added a DWLNDR history that lists files still on disk and moves a selected download to the Recycle Bin after an in-app confirmation.
- Replaced `.containerproject` with `.cproj` and its new Windows document icon. The old project format now shows a clear unsupported-format message.
- Rounded the Windows app icon and refreshed the desktop, Start menu and installer icons. Replaced the output cleanup symbol with a trash icon.
- Fixed a delayed history refresh hiding the action to open a completed download in the timeline, and made history writes safer.

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
