# Windows installer verification

Generated: 2026-09-17

| Item | Result |
| --- | --- |
| Target | Windows x64 / `x86_64-pc-windows-msvc` |
| Bundle | `desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/ENHE Codex Backup_0.1.2_x64-setup.exe` |
| Size | 32,847,573 bytes |
| SHA-256 | `f7021cd41cc8aa55685d74919673848a1ab7f4c09130f48b768c06a538de12f4` |
| Adjacent checksum file | PASS; value matches the installer |
| Tauri release build and NSIS bundling | PASS |
| Code signing | NOT_RUN; package is unsigned |
| Current-user uninstall/reinstall | PASS; prior installation removed and 0.1.2 installed using the authorized current-user path |
| Installed version/startup | PASS; registry and binary reported 0.1.2, and the running window title was `ENHE Codex Backup · v0.1.2` |
| Native picker and full desktop UI flow | NOT_RUN; native app control surface was not available to the evidence harness |
| Clean-profile WebView2 flow | NOT_RUN |
| Real account, OneDrive, second device, and live Codex continuation | NOT_RUN |

Package generation is not evidence of installed-app startup or live migration success.
