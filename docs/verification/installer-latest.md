# Windows installer verification

Generated: 2026-09-29. The final 0.1.12 artifact was packaged and its checksum, license payload and desktop icon contracts were verified. The package is unsigned. Current-user replacement installation is BLOCKED in the isolated environment because the protected old installation remained at 0.1.11 after both uninstaller and installer returned 0. GitHub publication requires successful Actions for the pushed commit; consult [Releases](https://github.com/yangjing6213-dev/Codex-Backup/releases) for published assets.

## Latest 0.1.12 package and replacement-install attempt

| Item | Result |
| --- | --- |
| Target/version | Windows x64 / 0.1.12; build metadata and installer package name verified |
| Candidate filename | `ENHE Codex Backup_0.1.12_x64-setup.exe` |
| Release filename | `ENHE.Codex.Backup_0.1.12_x64-setup.exe` |
| Size | 51,295,450 bytes |
| SHA-256 | `8859c4ba3f3f829133430096e5d3f2419e52081695d14b85142cad5864e879ed` |
| Package/checksum/licenses | PASS; direct Tauri build exit 0, sidecar matches, bundled notices/source/relinking materials and generated NSIS inclusion checks passed |
| Code signing | NotSigned; unsigned |
| Current-user uninstall/install | BLOCKED; old 0.1.11 process stopped, uninstaller exit 0 but did not remove protected files, installer exit 0 but installed executable remained 0.1.11 |
| Existing settings preservation | NOT_RECHECKED; no application-data deletion option or backup-folder cleanup was used |
| Native launch/visual test | NOT_RUN; the new executable was not confirmed installed |
| Scope boundary | Native backup/restore, real account, second device and cloud NOT_RUN |

Only this installer and its matching SHA-256 sidecar are intended as release assets. Source, README, changelog and verification notes are committed normally; local audit logs/screenshots are not published. The prior installer and older releases are retained.

## Latest 0.1.11 package and replacement installation

| Item | Result |
| --- | --- |
| Target/version | Windows x64 / 0.1.11; registry and installed executable version confirmed |
| Candidate filename | `ENHE Codex Backup_0.1.11_x64-setup.exe` |
| Release filename | `ENHE.Codex.Backup_0.1.11_x64-setup.exe` |
| Size | 51,295,145 bytes |
| SHA-256 | `e9f02039d3cdc8a868b317cb5101ddd28a11da3c2f9f5172b3ec2ece5b121655` |
| Package/checksum/licenses | PASS; package exit 0, sidecar matches, bundled notices and corresponding source/relinking materials verified |
| Code signing | NotSigned; unsigned |
| Current-user uninstall/install | PASS; 0.1.10 uninstall exit 0 and 0.1.11 install exit 0; no application-data deletion selected |
| Installed executable SHA-256 | `3da4f4d6fca8e8102aa165d4caad924fe0cb91bab8d48a5d85ef418f8024aa93` |
| Existing settings preservation | PASS; three application config/DPAPI files unchanged by hash; backup folders not removed |
| Native language test | PASS; both sidebar directions, Settings save despite unavailable scheduled-task status, English after restart, Projects and Backups navigation |
| Isolation | Temporary synthetic profile only, cloud off and automatic backup disabled; test instance closed afterwards |
| Scope boundary | Native backup/restore, real account, second device and cloud NOT_RUN |

Only this installer and its matching SHA-256 sidecar are release assets. Source, README, changelog and verification notes are committed normally; local audit logs/screenshots are not published. The prior installer and older releases are retained.

## Historical 0.1.10 package and replacement installation

| Item | Result |
| --- | --- |
| Target/version | Windows x64 / 0.1.10; registry and installed executable file/product version confirmed |
| Candidate filename | `ENHE Codex Backup_0.1.10_x64-setup.exe` |
| Size | 51,295,021 bytes |
| SHA-256 | `b68a5fc0946c7fdc3c06b49ee25a2c4e2e1da91a01d3856a2d801dbd7dab6a6c` |
| Candidate checksum sidecar | PASS; correct hash and exact candidate filename |
| Package command | PASS; exit 0, Cargo offline, existing MSVC and cached runtime binaries |
| License payload | PASS; bundled notices, source/relinking materials and generated NSIS inclusion checks passed |
| Code signing | NotSigned; unsigned |
| Current-user uninstall/install | PASS; 0.1.9 uninstall exit 0 and 0.1.10 install exit 0; no application-data deletion selected |
| Installed version/resources | PASS; registry and installed executable FileVersion/ProductVersion report 0.1.10 |
| Installed executable SHA-256 | `bbc475710f5487c7f9ed5a3f6778597e623c1f7719fcfb406fae7cd62ff2779d` |
| Configuration / DPAPI password file | PASS; `config.json` and `recovery-password.dpapi` remained present after replacement installation |
| Native launch | PASS; installed executable launched and exposed window title `ENHE Codex Backup · v0.1.10` |
| Native visual click-through | NOT_RUN; the native automation bridge did not expose the running window for screenshot/AX inspection |
| Real profile/account/second-device continuation | NOT_RUN; no personal profile, account, token or migration package was used |

The published Release contains only this installer and its `.sha256` sidecar; no other assets are in scope.

## Historical 0.1.10 publication readiness

| Field | Value |
| --- | --- |
| TASK_TYPE | `PROJECT_UPDATE + RELEASE_PUBLICATION` |
| READY_TO_PUBLISH | `YES` |
| NORMAL_PUSH | `YES; SSH 443, fast-forward only` |
| RELEASE_SCOPE | `v0.1.10; installer and SHA-256 sidecar only` |
| SIGNING | `NotSigned; unsigned package` |

## Published v0.1.10

| Item | Result |
| --- | --- |
| Release code commit | PASS; non-draft `v0.1.10` points to reviewed code commit `f3e220fbc54980124ad1e22b9c7e7e35d4c84214` |
| GitHub Actions | PASS; `ENHE Codex Backup CI` run `36255620465` completed successfully |
| Release/tag | PASS; non-draft, non-prerelease `v0.1.10` |
| Published installer asset | `ENHE.Codex.Backup_0.1.10_x64-setup.exe`, 51,295,021 bytes, GitHub SHA-256 `b68a5fc0946c7fdc3c06b49ee25a2c4e2e1da91a01d3856a2d801dbd7dab6a6c` |
| Published checksum asset | `ENHE.Codex.Backup_0.1.10_x64-setup.exe.sha256`, 106 bytes; GitHub asset digest `649e6e6895df3c435e2fa22ded22fde61f071b7cf576e2cf2a01c890e3045d6a` |
| Asset count | PASS; exactly two assets, older releases retained |
| Release URL | https://github.com/yangjing6213-dev/Codex-Backup/releases/tag/v0.1.10 |

## Historical published v0.1.9

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
