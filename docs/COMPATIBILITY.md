# Compatibility matrix

This matrix separates implemented format intent from checks actually run in the current environment.

| Component | Declared scope | Current result |
| --- | --- | --- |
| Application | ENHE Codex Backup 0.1.2, Windows 11 x64 | Source implemented; release Tauri compilation/NSIS bundling and authorized current-user installation/startup passed. Full desktop UI flow remains NOT_RUN. |
| Windows | Windows 11 x64, current user | Resource binaries were executed on Windows 11 Pro x64. The 0.1.2 current-user installer reported version 0.1.2 and launched a window titled `ENHE Codex Backup · v0.1.2`; clean-profile launch is NOT_RUN. |
| WebView2 | Tauri desktop runtime dependency | Presence on a clean target device is NOT_RUN. The installer documentation must retain an official WebView2 installation path if the target lacks it. |
| restic | 0.19.1 Windows amd64, encrypted local repository | Real temporary-repository init, backup, check, snapshot listing, and restore passed in `docs/verification/local-restic-acceptance.md`; Rust tests and release wiring compile passed. |
| rclone | 1.75.1 Windows amd64, OneDrive adapter | Binary version, official archive hash, and adapter tests verified; live authorization remains NOT_RUN. |
| ReHome core | Pinned commit `cc5daae501b22b86e1b4c2dafbd1b4af61992b48` | Applicable upstream core is embedded; identity/updater integration was removed and Rust integration tests passed. |
| Backup schema | `enhe-codex-backup` schema 1, restic payload root `enhe-payload` | Manifest/path checks, Rust integration tests, and isolated restic storage acceptance passed. |
| Codex data | Local files including JSONL, SQLite/WAL/SHM, archived and handoff material | File staging and issue reporting are implemented. Current Codex-version compatibility, live session visibility, and original-thread continuation are NOT_RUN. |
| OneDrive | rclone OneDrive remote, explicit user authorization | Non-interactive question/answer adapter is implemented. No account was authorized and no data was uploaded: LIVE E2E NOT_RUN. |

Not supported by this release scope: automatic cloud synchronization, multi-cloud routing, telemetry, auto-updater, macOS/Linux packages, automatic account migration, or unattended public release.
