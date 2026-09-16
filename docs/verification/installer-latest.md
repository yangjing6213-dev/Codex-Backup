# Windows installer verification

Generated: 2026-09-17

| Item | Result |
| --- | --- |
| Target | Windows x64 / `x86_64-pc-windows-msvc` |
| Bundle | `desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/ENHE Codex Backup_0.1.0_x64-setup.exe` |
| Size | 32,838,095 bytes |
| SHA-256 | `ff3e2e8d003250fa8b4e89fc37707522b86590f821a15b36c6991a543691836a` |
| Adjacent checksum file | PASS; value matches the installer |
| Tauri release build and NSIS bundling | PASS |
| Code signing | NOT_RUN; package is unsigned |
| Current-user installer execution | PASS; installer exit code 0; installed `enhe-codex-backup.exe` started with window title `ENHE Codex Backup`, remained responsive, and had no observed TCP connections |
| Native picker and full desktop UI flow | NOT_RUN; native app control surface was not available to the evidence harness |
| Clean-profile WebView2 flow | NOT_RUN |
| Real account, OneDrive, second device, and live Codex continuation | NOT_RUN |

Package generation is not evidence of installed-app startup or live migration success.
