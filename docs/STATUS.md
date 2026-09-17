# Current status

Date of this source checkpoint: 2026-09-17.

Implementation is present for the five-view bilingual UI, automatic/manual ordinary-folder selection, local restic workflow, complete payload staging, manifest/issue reporting, JSONL integrity marking, restore conflict handling, Git worktree pointer repair, SQLite online snapshots, DPAPI secret storage, current-user scheduler, rclone OneDrive question protocol, upload/download connection checks, explicit cloud copy, scripts, and documentation. The source is not a release claim.

Verified in this environment:

- Frontend TypeScript build and Vitest UI shell tests: PASS.
- Native directory picker, configured-path-first startup, actionable Codex path diagnostics, bounded fixed-disk project discovery, metadata-only Codex-home/conversation discovery, and automatic fallback retry: PASS in focused React/Rust tests.
- Frontend DOM tests cover the five navigation entries, operation-guide flowchart, aggregated permission warning, administrator rescan action, and version label; Tauri bridge/WebView2 full UI behavior is not claimed from DOM tests alone.
- Real restic 0.19.1 temporary encrypted repository init/backup/check/list/restore with Git/worktree/Codex fixture: PASS for the CLI storage engine; see `verification/local-restic-acceptance.md`.
- Official restic/rclone Windows amd64 archive hashes and executable version output: PASS.
- `scripts/verify.ps1` completed the frontend, bilingual, source-scan, Rust formatting, Rust tests, and isolated-restic checks: PASS. The Rust run covered 56 library tests plus all project integration suites; the one explicitly environment-dependent restore test and two local Windows acceptance tests remained ignored.
- `scripts/package.ps1 -SkipBootstrap` completed the Windows x64 release build and NSIS bundling for 0.1.2: PASS. The generated installer is `desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/ENHE Codex Backup_0.1.2_x64-setup.exe` (32,847,573 bytes), with matching SHA-256 `f7021cd41cc8aa55685d74919673848a1ab7f4c09130f48b768c06a538de12f4` in the adjacent `.sha256` file.
- The prior current-user installation was uninstalled, the 0.1.2 installer was executed with the authorized current-user silent install path, and the installed binary reported file version 0.1.2: PASS. The launched window reported `ENHE Codex Backup · v0.1.2` and remained running.

Not verified:

- Full native UI click-through, native directory-dialog selection, clean-profile WebView2 flow, and real Task Scheduler worker remain NOT_RUN; the desktop control surface was unavailable to the evidence harness. The installed-app backup button flow was not exercised through the UI in this run; isolated local restic acceptance remains PASS.
- OneDrive authorization/upload/download, second physical device, real Codex account/session visibility, and thread continuation: NOT_RUN by scope and authorization.

Overall release decision: `RELEASE_READY: NO` until the remaining desktop UI and release checks are completed.
