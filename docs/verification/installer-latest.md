# Windows installer verification

Generated: 2026-09-17

| Item | Result |
| --- | --- |
| Target | Windows x64 / `x86_64-pc-windows-msvc` |
| Bundle | `desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/ENHE Codex Backup_0.1.1_x64-setup.exe` |
| Size | 32,824,356 bytes |
| SHA-256 | `947c400e7e365428064bb71a8dde829a92c8353eef464643f5de824a80f04450` |
| Adjacent checksum file | PASS; value matches the installer |
| Tauri release build and NSIS bundling | PASS |
| Code signing | NOT_RUN; package is unsigned |
| Current-user installer execution | NOT_RUN for 0.1.1; system-level installation was not authorized in this task |
| Native picker and full desktop UI flow | NOT_RUN; native app control surface was not available to the evidence harness |
| Clean-profile WebView2 flow | NOT_RUN |
| Real account, OneDrive, second device, and live Codex continuation | NOT_RUN |

Package generation is not evidence of installed-app startup or live migration success.
