# Development

## Requirements

- Windows 10 or 11
- Node.js 22+
- pnpm 10+
- Rust stable with the MSVC toolchain
- Visual Studio Build Tools with Desktop development with C++
- FFmpeg and FFprobe available on `PATH`

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

To create a GitHub release, update the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, then push a matching tag. The release workflow signs the updater artifact and publishes only the installer and portable executable. The machine-readable updater manifest is maintained separately on the `updater` branch so it does not clutter Release assets.

## Project layout

```text
src/                  Svelte 5 interface
src/lib/              Toolbox, SmartCut and Batch workspaces
src-tauri/src/        Rust commands and FFmpeg orchestration
src-tauri/icons/      Windows application icons
.github/workflows/    CI and Windows release automation
docs/                 Screenshots and project notices
```
