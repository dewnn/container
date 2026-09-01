# Open-source publication review

## Current status

The repository layout, documentation, automated checks and release packaging are ready for a public GitHub repository. An OSI-approved project license has deliberately not been added yet.

## Blocking item: SmartCut provenance

CONTAINER's SmartCut workflow was built to closely follow `cobanov/autocut`. The upstream repository was checked on 2026-09-01 and did not contain a license file or package-level license declaration. Publicly visible source code is not automatically permission to copy, modify or redistribute it.

Before calling the complete repository “open source”, choose one of these routes:

1. Obtain written permission/license terms from the Autocut copyright holder and preserve the required notices.
2. Remove or independently reimplement the SmartCut-derived code and interface without copying protected source or assets.
3. Publish only the independently owned Toolbox/Batch portions until the SmartCut review is resolved.

After that decision, add an appropriate root `LICENSE` file and matching license metadata to `package.json` and `src-tauri/Cargo.toml`.

## Release checklist

- [ ] Resolve SmartCut provenance and select the project license.
- [ ] Add the final root `LICENSE` file.
- [ ] Confirm every image, icon, font and model can be redistributed.
- [x] Do not bundle FFmpeg; document the tested system dependency and official source link.
- [ ] Run secret and large-file scans.
- [ ] Run frontend checks, Rust formatting/tests and a clean Windows installer build.
- [ ] Test the missing-FFmpeg guidance and dependency recheck on a clean Windows machine.
- [ ] Create the public GitHub repository and set its About text/topics.
- [x] Verify matching version fields before every release tag in GitHub Actions.
