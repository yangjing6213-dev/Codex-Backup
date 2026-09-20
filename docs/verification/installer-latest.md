# Windows installer verification

Generated: 2026-09-20

| Item | Result |
| --- | --- |
| Target | Windows x64 / `x86_64-pc-windows-msvc` |
| Bundle | `desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/ENHE Codex Backup_0.1.6_x64-setup.exe` |
| Size | 33,247,826 bytes |
| SHA-256 | `9ddf7bb14a54153ecc23dd46b2197b31bc9cd75f67fab06d8a17bb8d7d16c058` |
| Adjacent checksum file | PASS; value matches the installer |
| Tauri release build and NSIS bundling | PASS |
| Code signing | NOT_RUN; package is unsigned |
| Current-user uninstall/reinstall | PASS; prior installation removed and 0.1.6 installed using the authorized current-user path |
| Installed version/startup | PASS; registry and binary file/product version reported 0.1.6, and the captured running window showed `ENHE Codex Backup · v0.1.6` in both title bar and sidebar |
| Synthetic application backup round trip | PASS; bundled restic completed backup, snapshot listing, and restore through asynchronous application commands using temporary synthetic data only |
| Native picker and full desktop UI flow | NOT_RUN; native startup and overview visual verification passed, but directory-picker and full backup-button click-through were not exercised with personal data |
| Clean-profile WebView2 flow | NOT_RUN |
| Real account, OneDrive, second device, and live Codex continuation | NOT_RUN |

Package generation is not evidence of installed-app startup or live migration success.
