# Current status

Date of this source checkpoint: 2026-09-20.

Implementation is present for the five-view bilingual UI, automatic/manual ordinary-folder selection, local restic workflow, complete payload staging, manifest/issue reporting, JSONL integrity marking, restore conflict handling, Git worktree pointer repair, SQLite online snapshots, DPAPI secret storage, current-user scheduler, rclone OneDrive question protocol, upload/download connection checks, explicit cloud copy, scripts, and documentation. The source is not a release claim.

Verified in this environment:

- Frontend TypeScript build and 44 Vitest tests: PASS. Error presentation tests verify that backup and restore failures remain distinct and include a cause and an actionable solution through the production translation path in both languages.
- Native directory picker, configured-path-first startup, actionable Codex path diagnostics, bounded fixed-disk project discovery, metadata-only Codex-home/conversation discovery, and automatic fallback retry: PASS in focused React/Rust tests.
- Frontend DOM tests cover the five navigation entries, operation-guide flowchart, aggregated permission warning, administrator rescan action, and version label; Tauri bridge/WebView2 full UI behavior is not claimed from DOM tests alone.
- Real restic 0.19.1 temporary encrypted repository init/backup/check/list/restore with Git/worktree/Codex fixture: PASS for the CLI storage engine; see `verification/local-restic-acceptance.md`.
- Official restic/rclone Windows amd64 archive hashes and executable version output: PASS.
- `scripts/verify.ps1` completed the frontend, bilingual, source-scan, Rust formatting, Rust tests, and isolated-restic checks: PASS. The full Rust run covered 72 library tests and all non-environment-dependent integration suites with no failures.
- A bundled-restic round trip through the asynchronous application commands, using only temporary synthetic Codex/project data and a temporary encrypted repository, completed successfully: PASS.
- Repository validation now rejects a non-empty ordinary folder before invoking restic unless the complete restic repository layout is present. Focused tests also verify incomplete/config-only lookalikes, password/repository/disk-space classification, and redaction of the temporary password-file path: PASS.
- `scripts/package.ps1 -SkipBootstrap` completed the Windows x64 release build and NSIS bundling for 0.1.6: PASS. The generated installer is `desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/ENHE Codex Backup_0.1.6_x64-setup.exe` (33,247,826 bytes), with matching SHA-256 `9ddf7bb14a54153ecc23dd46b2197b31bc9cd75f67fab06d8a17bb8d7d16c058` in the adjacent `.sha256` file.
- The prior current-user installation was uninstalled and 0.1.6 was installed through the authorized current-user path: PASS. The registry, binary file/product version, native window title, visible sidebar version, and captured startup UI all reported 0.1.6.

Not verified:

- Full native UI click-through, native directory-dialog selection, clean-profile WebView2 flow, and real Task Scheduler worker remain NOT_RUN. The installed-app startup and primary overview were visually verified, but the backup button was not used with personal data; the synthetic asynchronous backup round trip and isolated local restic acceptance are PASS.
- OneDrive authorization/upload/download, second physical device, real Codex account/session visibility, and thread continuation: NOT_RUN by scope and authorization.

Overall source/package decision: `RELEASE_READY: PARTIAL`. The defect-specific checks, canonical verification, package build, replacement installation, startup, and synthetic backup pass; external GitHub publication and the broader account/device/cloud acceptance matrix remain separately gated.
