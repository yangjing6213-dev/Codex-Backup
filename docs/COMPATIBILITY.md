# Compatibility matrix

This matrix separates implemented format intent from checks actually run in the current environment.

| Component | Declared scope | Current result |
| --- | --- | --- |
| Application | ENHE Codex Backup 0.1.10, Windows 11 x64 | Fresh verification, NSIS build, current-user replacement installation and installed process launch PASS; see STATUS and installer verification. Native visual UI and live-account acceptance remain NOT_RUN. |
| Windows | Windows 11 x64, current user | Authorized 0.1.9 uninstall/0.1.10 install passed. Registry, installed binary/resources and preserved configuration verified; visual click-through is NOT_RUN. |
| WebView2 | Tauri desktop runtime dependency | Presence on a clean target device is NOT_RUN. The installer documentation must retain an official WebView2 installation path if the target lacks it. |
| restic | 0.19.1 Windows amd64, encrypted local repository | Real temporary-repository init, backup, check, snapshot listing, and restore passed in `docs/verification/local-restic-acceptance.md`; Rust tests and release wiring compile passed. |
| rclone | 1.75.1 Windows amd64, OneDrive adapter | Binary version, official archive hash, and adapter tests verified; live authorization remains NOT_RUN. |
| ReHome core | Pinned commit `cc5daae501b22b86e1b4c2dafbd1b4af61992b48` | Applicable upstream core is embedded; identity/updater integration was removed and Rust integration tests passed. |
| Backup schema | `enhe-codex-backup` schema 1, restic payload root `enhe-payload` | Manifest/path checks, Rust integration tests, and isolated restic storage acceptance passed. |
| Codex data | Local files including JSONL, SQLite/WAL/SHM, archived and handoff material | File staging/classification and opt-in migrate-and-connect checks are implemented. Synthetic protocol tests and an offline CLI schema inspection do not establish a supported Codex version range, native visibility or real-account continuation; these remain NOT_RUN. |
| OneDrive | rclone OneDrive remote, explicit user authorization | Non-interactive question/answer adapter is implemented. No account was authorized and no data was uploaded: LIVE E2E NOT_RUN. |

Not supported by this release scope: automatic cloud synchronization, multi-cloud routing, telemetry, auto-updater, macOS/Linux packages, automatic account migration, or unattended public release.
