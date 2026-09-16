# Current status

Date of this source checkpoint: 2026-09-17.

Implementation is present for the four-view bilingual UI, manual ordinary-folder selection, local restic workflow, complete payload staging, manifest/issue reporting, JSONL integrity marking, restore conflict handling, Git worktree pointer repair, SQLite online snapshots, DPAPI secret storage, current-user scheduler, rclone OneDrive question protocol, upload/download connection checks, explicit cloud copy, scripts, and documentation. The source is not a release claim.

Verified in this environment:

- Frontend TypeScript build and Vitest UI shell tests: PASS.
- Native directory picker, configured-path-first startup, actionable Codex path diagnostics, bounded fixed-disk project discovery, metadata-only Codex-home/conversation discovery, and automatic fallback retry: PASS in focused React/Rust tests.
- Local browser preview showed the four navigation entries, settings form, theme choices, and Chinese/English switch; Tauri bridge/WebView2 behavior is not claimed from this preview.
- Real restic 0.19.1 temporary encrypted repository init/backup/check/list/restore with Git/worktree/Codex fixture: PASS for the CLI storage engine; see `verification/local-restic-acceptance.md`.
- Official restic/rclone Windows amd64 archive hashes and executable version output: PASS.
- `scripts/verify.ps1` completed the frontend, bilingual, source-scan, Rust formatting, Rust tests, and isolated-restic checks: PASS. The Rust run covered 56 library tests plus all project integration suites; the one explicitly environment-dependent restore test and two local Windows acceptance tests remained ignored.
- `scripts/package.ps1 -SkipBootstrap` completed the Windows x64 release build and NSIS bundling: PASS. The generated installer is `desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/ENHE Codex Backup_0.1.0_x64-setup.exe` (32,838,095 bytes), with matching SHA-256 `ff3e2e8d003250fa8b4e89fc37707522b86590f821a15b36c6991a543691836a` in the adjacent `.sha256` file.

Not verified:

- Current-user installation and installed application startup: PASS; the newly generated installer exited with code 0, and the installed process exposed the `ENHE Codex Backup` window and remained responsive with no TCP connections at the time of inspection. Clean-profile WebView2 flow, full UI actions, native dialog click-through, and real Task Scheduler worker remain NOT_RUN.
- OneDrive authorization/upload/download, second physical device, real Codex account/session visibility, and thread continuation: NOT_RUN by scope and authorization.

Overall release decision: `RELEASE_READY: NO`.
