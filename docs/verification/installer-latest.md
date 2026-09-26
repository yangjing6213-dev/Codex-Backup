# Windows installer verification

Generated: 2026-09-26. The final 0.1.9 artifact was packaged, installed over the current-user 0.1.8 installation, and verified locally. The package is unsigned; GitHub publication is verified below after the ordinary fast-forward push and Actions gate.

## Latest 0.1.9 package and replacement installation

| Item | Result |
| --- | --- |
| Target/version | Windows x64 / 0.1.9; registry and installed executable file/product version confirmed |
| Candidate filename | `ENHE Codex Backup_0.1.9_x64-setup.exe` |
| Size | 51,296,811 bytes |
| SHA-256 | `acbb35e19fe3a7627cb5ba5bf12d72a19d65a6c6a81fcacb43873df833baf9c0` |
| Candidate checksum sidecar | PASS; correct hash and exact candidate filename |
| Package command | PASS; exit 0, Cargo offline, existing MSVC and cached runtime binaries |
| License payload | PASS; bundled notices, source/relinking materials and generated NSIS inclusion checks passed |
| Code signing | NotSigned; unsigned |
| Current-user uninstall/install | PASS; 0.1.8 uninstall exit 0 and 0.1.9 install exit 0; no application-data deletion selected |
| Installed version/resources | PASS; registry and installed executable FileVersion/ProductVersion report 0.1.9 |
| Installed executable SHA-256 | `6971957db92e03275abe2b56dbd36fc7483efd981e3efff6c4b64effafaa4eda` |
| Configuration / DPAPI password file | PASS; `config.json` and `recovery-password.dpapi` remained present after replacement installation |
| Native launch | PASS; installed executable launched and exposed window title `ENHE Codex Backup · v0.1.9` |
| Native visual click-through | NOT_RUN; the native automation bridge did not expose the running window for screenshot/AX inspection |
| Real profile/account/second-device continuation | NOT_RUN; no personal profile, account, token or migration package was used |

The published Release contains only this installer and its `.sha256` sidecar; no other assets are in scope.

## Published v0.1.9

| Item | Result |
| --- | --- |
| Release code commit | PASS; non-draft `v0.1.9` points to reviewed code commit `1a6a1ffe7edf3b9fea6683cd9a118588c3f41089` |
| Current main head | PASS; GitHub `main` points to documentation-only publication commit `0c3c1573c9557796fc44fe480d394467cdbd86e4` |
| GitHub Actions | PASS; `ENHE Codex Backup CI` runs `36251351095` and `36252068603` completed successfully |
| Release/tag | PASS; non-draft `v0.1.9` points to the reviewed code commit |
| Published installer asset | `ENHE.Codex.Backup_0.1.9_x64-setup.exe`, 51,296,811 bytes, GitHub SHA-256 `acbb35e19fe3a7627cb5ba5bf12d72a19d65a6c6a81fcacb43873df833baf9c0` |
| Published checksum asset | `ENHE.Codex.Backup_0.1.9_x64-setup.exe.sha256`, 105 bytes; content contains the matching installer hash |
| Asset count | PASS; exactly two assets, older releases retained |
| Release URL | https://github.com/yangjing6213-dev/Codex-Backup/releases/tag/v0.1.9 |

## Historical 0.1.7 license-only candidate (not installed)

| Item | Result |
| --- | --- |
| Target/version | Windows x64 / 0.1.7; application file/product version confirmed |
| Candidate filename | `ENHE.Codex.Backup_0.1.7_x64-setup.exe` |
| Size | 51,249,947 bytes |
| SHA-256 | `825e74e59be3954f091d829761cb01f673f9c9330a05d910946ddf3ba10170c0` |
| Candidate checksum sidecar | PASS; correct hash and exact candidate filename |
| Package command | PASS; exit 0, Cargo offline, existing MSVC and runtime binaries |
| License payload | Five offline notice files plus `manifest.json`, exact rclone/SDDL source and relinking materials, and `RCLONE-BUILD-MANIFEST.json`; exact hashes and generated-NSIS inclusion checks PASS |
| License regression fixtures | Ten cases PASS, including missing/altered notices and LGPL source materials, stale inputs, omitted resources, duplicate input and LF/CRLF portability |
| Application behavior | No application source, application test, dependency version or runtime binary change in this conservative LGPL packaging correction |
| Code signing | NotSigned; unsigned |
| Uninstall/install/startup | NOT_RUN by current instruction; previously installed executable remains unchanged |
| Installer extraction/native UI | NOT_RUN; compilation and generated inclusion instructions are not an installed-file or pixel-level verification |
| Technical publication readiness | READY; the pinned SDDL discrepancy is handled through the documented conservative LGPL path, with corresponding source, relinking instructions and build materials included. This is not a legal certification; exact user authorization is still required. See [third-party review](../THIRD_PARTY.md#017-lgpl-source-and-relinking-material) |

The earlier package is retained locally. No commit, push, tag, release or asset upload was performed. The output bundle path below now contains the final local candidate; historical installation proof is bound to the earlier hash, not that reused path.

## Earlier 0.1.7 installation (historical evidence)

| Item | Result |
| --- | --- |
| Target | Windows x64 / `x86_64-pc-windows-msvc` |
| Bundle | `desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/ENHE Codex Backup_0.1.7_x64-setup.exe` |
| Earlier local copy | `ENHE.Codex.Backup_0.1.7_x64-setup.exe` and matching `.sha256`; retained but superseded as a publication candidate |
| Size | 33,292,697 bytes |
| SHA-256 | `0678d177527c9f8f54b83468d158b275572d80055c899401f0cdbf68bcb373e8` |
| Adjacent checksum file | PASS; matches installer, release copy and filename |
| Tauri release build and NSIS bundling | PASS; exit 0, cached tools, Cargo offline |
| Code signing | NOT_RUN; package is unsigned |
| Current-user uninstall/reinstall | PASS; 0.1.6 removed and 0.1.7 installed, both exit 0; no application-data deletion selected |
| Installed version/resources | PASS; registry and executable version 0.1.7; main executable matches the single documented Tauri UNK-to-NSS bundle marker; restic/rclone SHA-256 match exactly |
| Installed main executable SHA-256 | `e0c4448f86fc4edd8b30d4a45ee4b9753c297ac59db1959ea89b0bb9ceda1f84` |
| Configuration / DPAPI password file | PASS; unchanged by hash; configuration recovery copy retained locally only |
| Desktop / Start Menu shortcuts | PASS for target metadata; both target the new executable and inherit its icon; visual icon rendering NOT_RUN |
| Installed startup | NOT_RUN; not launched against a real Codex profile |
| Synthetic application backup round trip | Default canonical run does not enable the ignored bundled-restic application test; separate isolated restic CLI acceptance is reported in STATUS |
| Native picker and full desktop UI flow | UI NOT VISUALLY VERIFIED; do not transfer historical native startup evidence |
| Clean-profile WebView2 flow | NOT_RUN |
| Real account, OneDrive, second device, and live Codex continuation | NOT_RUN |

Package generation is not evidence of installed-app startup or live migration success.
