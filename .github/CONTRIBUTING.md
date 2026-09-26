# Development

## Requirements

- Windows 10 or 11
- Node.js 22+
- pnpm 10+
- Rust stable with the MSVC toolchain
- Visual Studio Build Tools with Desktop development with C++
- FFmpeg and FFprobe available on `PATH`
- yt-dlp and Deno (versions pinned in `tooling/config/bundled-tools.env`)

## Run locally

```powershell
pnpm install --frozen-lockfile
pnpm check
pnpm build
pnpm tauri dev
```

## Build a Windows release

```powershell
pnpm install --frozen-lockfile
pnpm tauri build --no-sign
```

To create a GitHub release, update the version in `package.json`, `src-tauri/Cargo.toml` (and lockfile) and `src-tauri/tauri.conf.json`, then push a matching tag. The release workflow signs the updater artifact and publishes only the installer and portable ZIP. The machine-readable updater manifest is maintained separately on the `updater` branch so it does not clutter Release assets.

## Checks

```powershell
pnpm test:ui
pnpm test:e2e
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm tauri:build:dev
pnpm test:installer-hooks
```

The DEV build also checks both embedded Windows icon groups against every source
ICO frame and asks Windows Shell to extract them. `pnpm dev:register-association`
registers that verified DEV executable for `.cproj` in the desktop session.
Tests and generators are development-only; they are not shipped in the installer.
The installer-hook test needs NSIS (prepared by `pnpm tauri bundle --no-sign`),
and exercises fresh/upgrade paths using isolated registry keys and temporary
shortcuts, without changing the installed app or real file associations.

## Project layout

```text
src/                  Svelte 5 interface
src/lib/              Toolbox, SmartCut and Batch workspaces
src/public/           Theme-aware brand assets (served at unchanged URLs)
src-tauri/src/        Rust commands and FFmpeg orchestration
src-tauri/icons/      Sources for the two icons embedded in the Windows EXE
.github/workflows/    CI and Windows release automation
docs/                 Screenshots and project notices
tooling/              Tests, generators and pinned build configuration
```

Installed Windows layout: the app, its four required media/downloader helper
executables, the uninstaller, and one `licenses/` folder. App/project icons and
the VAD/face models are embedded; no separate ICO files are installed.

On Windows, runtime files belong to `%LOCALAPPDATA%/dev.dean.container/`:
`data/` holds download history and the session marker, and `cache/` holds
materialized Social Tag images and the VAD model. DEV uses the separate
`dev.dean.container.dev` namespace. Existing runtime/downloader subfolders stay
unchanged. Migration preserves history and recovery detection, removes only
unmodified known embedded cache copies, and leaves unknown files alone.
