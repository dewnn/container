# GitHub publication setup

Use these settings when the licensing review is resolved and the repository is ready to become public.

## Repository

- Suggested name: `container`
- Visibility: Public
- Default branch: `main`
- Description: `Local-first FFmpeg media toolbox with SmartCut, batch processing, previews, and Windows packaging. Rust + Tauri + Svelte 5.`
- Website: leave empty until a project page exists

Suggested topics:

```text
ffmpeg
video-processing
video-editing
audio-processing
image-processing
tauri
rust
svelte
smartcut
media-toolbox
windows
```

## GitHub sidebar sections

- **Languages** is calculated automatically from tracked source files. `.gitattributes` excludes generated lock files and bundled binaries from the percentage.
- **Contributors** is calculated automatically from commit authors. Set the correct Git author before the first commit.
- **Releases** is populated by `.github/workflows/release.yml` when a version tag is pushed.

## First publication

Set your Git identity before committing:

```powershell
git config user.name "YOUR GITHUB NAME"
git config user.email "YOUR VERIFIED GITHUB EMAIL"
```

After adding the final license:

```powershell
git add .
git commit -m "feat: initial public release"
git remote add origin https://github.com/YOUR-ACCOUNT/container.git
git push -u origin main
```

Do not push a release tag until CI passes.

## First release

1. Set the same version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
2. Move the release notes from `docs/CHANGELOG.md` out of Unreleased.
3. Commit and push the version change.
4. Create and push the tag:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

The Windows workflow publishes a stable **Latest** release containing the installer, portable ZIP and SHA-256 checksums. Review its generated notes and FFmpeg attribution after publishing.

## Recommended repository settings

- Enable Issues and private vulnerability reporting.
- Add branch protection for `main` after the first CI run.
- Require the `Frontend checks` and `Rust checks` jobs before merging.
- Disable force-pushes and branch deletion on `main`.
- Keep Actions permissions at read-only by default; the release workflow declares the write permission it needs.
