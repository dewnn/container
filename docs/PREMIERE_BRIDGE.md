# Optional Premiere bridge (DEV)

Windows / Premiere Pro 2025–2026 (25.x–26.x), CEP 12. Real-host transfers have been checked on 2025 (25.5) and 2026; keep only one Premiere version running at a time. Other patches and workstations still require validation. This is an original CONTAINER extension, not copied from AutoSubs. The design uses Adobe's public CEP/ExtendScript APIs.

## User workflow

1. General settings → Set up Premiere connection. Read and accept the setup warning.
2. Restart Premiere. The hidden bridge starts on Adobe's `com.adobe.csxs.events.ApplicationActivate` event. If the bridge was already installed, Premiere may be launched after rendering; CONTAINER polls every two seconds and exposes send actions for existing outputs when connected.
3. CONTAINER displays Premiere Connected only after a fresh host response. An open executable alone is not considered connected.
4. Render a video in Toolbox/Stack or SmartCut. Each Send creates a separate uniquely named project using the rendered filename, and `createNewSequenceFromClips` derives settings from the rendered file. Existing timelines receive no appended clips. Existing projects are never auto-saved or overwritten; Premiere may show its own unsaved-change prompt when switching projects. New projects are saved after creation. No automatic send or Premiere launch. No extra FFprobe process is required for this new-project workflow.
5. New projects use the existing parent folder of Premiere's `BE.Prefs.MRU.Document.0`, read from the active host preferences only at transfer time. This is the **last project folder**, not a claim that Adobe exposes its New Project dialog's exact default. Import/export directory preferences are deliberately ignored. If unavailable, use the active `getPProPrefPath` profile's version folder, then Documents/Adobe/Premiere Pro/CONTAINER Projects as a version-neutral final fallback. Missing MRU folders are not recreated. Existing project filenames are never overwritten.
6. If the active project/sequence changed since the displayed connection state, sending fails without inserting. Connection is represented by a small green dot on Settings and a separate output row. Setup uses a theme-aware HTML modal with explicit confirmation of the shared Adobe debug preference.

## Setup and removal

Setup embeds and installs only four bridge assets and an ownership marker in `%APPDATA%/Adobe/CEP/extensions/dev.dean.container.premiere`. It enables **HKCU/Software/Adobe/CSXS.12/PlayerDebugMode = "1"**. This is a shared Adobe preference: it also permits other unsigned CEP extensions. Setup is never performed on app launch or rendering. The original raw registry value is backed up first; updating the bridge does not overwrite that backup.

Remove Premiere bridge deletes only named owned files, not Adobe folders recursively. It restores the previous registry value if the current value is still the one CONTAINER set. A setting changed afterward is left untouched. Restart Premiere after installation, update or removal. Existing projects and renders are never deleted.

## Local protocol

Settings → Clean project files requires confirmation and Premiere fully closed. New names are `CONTAINER-{video stem}-{timestamp}-{12 hex digits}.prproj`, eligible only through recorded ownership paths. Legacy timestamp-only names directly in Documents/Adobe/Premiere Pro/26.0 remain recognized. No recursive search, render deletion or following links/junctions. Files move to Recycle Bin, including user edits inside those generated projects. Renamed projects and differently named Save As copies are retained. Older custom-folder projects without ownership records are intentionally not guessed or deleted. Sender requires host protocol 3, so an outdated bridge cannot accidentally append instead of creating a separate project.

Same-user mailbox at `%APPDATA%/dev.dean.container/premiere-bridge`, no network port. Session ID changes for every CONTAINER process. Commands must start within 30 seconds; the native result wait allows 90 seconds for slow project creation. One outstanding transfer and escaped string arguments remain enforced; claimed requests are never automatically replayed. The legacy operation name remains `append`, with `separate_project: true` required by the current sender. Only videos canonicalized under Downloads/CONTAINER Output are accepted. Frontend status refresh pauses during sending so the connection indicator stays stable; it refreshes immediately afterward. This is a pending state, not proof Premiere stayed responsive. Local processes with this user's file access share the same trust boundary.

Timeout is **uncertain**, not a guaranteed failure: the host may have inserted the clip but failed to respond. The UI explicitly tells users to check Premiere before retrying. The native async runtime/UI is not blocked while waiting.

Read-only status calls have a five-second recovery watchdog. Late replies to abandoned status calls are ignored using generation tokens. An insertion is never retried by this watchdog; an unconfirmed insertion remains blocked in the host bridge until its callback returns or Premiere restarts, to avoid duplicate clips.

## Verification and limitations

Protocol 3 separates project creation from import/timeline creation using a once-only host continuation token. CEP yields for 250 ms between these calls; the continuation verifies the project has not changed and cannot be replayed. A hung status query permits only one replacement rather than building an unbounded evalScript queue. Success requires a fresh post-transfer heartbeat for the returned project and an active sequence (up to five seconds); a missing reply warns of uncertain completion instead of announcing success. These changes do not bypass the synchronous Adobe `newProject` call or guarantee a speedup/crash fix. Latest observed project creation was ~16.1 seconds; import/sequence/open/save combined were under half a second. Windows Application log checks returned no matching crash event during the investigation. AutoSubs imports subtitles into an existing project, so it is not evidence that creating a separate Premiere project can be equally fast. Do not change Adobe workspace/cache preferences automatically.

Save regression: Adobe's CEP type definition declares `Project.save(): void`. An undefined return is accepted, as is numeric zero; false, nonzero numeric returns and exceptions produce a partial-completion warning, not an import-failed message. Sequence-open failure also warns that creation already happened. Never automatically resend either case. The last response records host operation timings for create/import/sequence/open/save/insert; these diagnose completed calls, not a host call that never returns. Cleanup ownership bookkeeping failure cannot suppress the transfer acknowledgement. These changes do not prove that Premiere UI stalls are fixed.

- Rust tests: fresh/current-session heartbeat and rendered-video path boundaries.
- `node tooling/scripts/test-premiere-bridge.mjs`: mocked Adobe host and mailbox; append, reuse, missing timeline/file, import/insert failure, changed project, expiry, string escaping and no replay.
- Browser E2E: hidden when disconnected/no render, explicit send only, correlated output and destination, matching-timeline fallback, setup confirmation cancellation and a separate output row.
- These mocks do **not** prove native CEP loading or insertion in a real Premiere installation. Those require a manual smoke test after optional setup, using a disposable sequence. No claims of full Premiere 2026 compatibility until that smoke test passes.
- Current scope excludes macOS/Linux, batch-wide send, editable SmartCut cuts, other NLEs, saving existing projects and creating additional tracks. An import may remain in the Project panel if later insertion fails. A newly created project may remain if a later step fails; it is never silently deleted.

References: [Adobe CEP 12 cookbook](https://github.com/Adobe-CEP/CEP-Resources/blob/master/CEP_12.x/Documentation/CEP%2012%20HTML%20Extension%20Cookbook.md), [Adobe Premiere sample APIs](https://github.com/Adobe-CEP/Samples/blob/master/PProPanel/jsx/PPRO/Premiere.jsx).
