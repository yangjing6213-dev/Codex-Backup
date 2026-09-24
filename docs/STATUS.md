# Current status

Source checkpoint: 2026-09-25. Current version: **0.1.8; project-root scan guidance candidate**. Packaging, installation and GitHub publication evidence for this candidate is recorded below after the final checks. Historical 0.1.7 evidence remains dated and is not transferred automatically. See [installer evidence](verification/installer-latest.md); neither statement is a GitHub publication claim until the release is verified.

## 0.1.8 project-root scan improvements

- Explicit scan roots enumerate every accessible direct child folder under each configured root, while recursive counting continues to cover all readable regular files below each selected project.
- Empty scan roots remain a bounded whole-drive discovery mode and now expose an explicit incomplete-results warning; the Projects page provides a native root picker, current-versus-retained grouping and select-all-current action.
- No backup inclusion rule changed: hidden files, dependencies, build outputs, `.env`, private keys and tokens remain included for selected project backups, while Codex credential exclusions remain separate.

Ordinary Git HTTPS connectivity failed during preflight; an earlier read-only SSH check at `ssh.github.com:443` succeeded for the verified repository/account, but the final read-only recheck returned `Permission denied (publickey)` without any write. No commit, push, release creation or asset upload has been performed for this update. The pinned SDDL discrepancy is handled on the conservative LGPL path with exact corresponding source, relinking instructions and build materials; this is technical redistribution evidence, not legal certification. Fresh exact-manifest/tag approval and SSH 443 authentication revalidation are required before any ordinary fast-forward push or release. Real-account, second-device, native UI and model/cloud acceptance remain NOT_RUN.

## 0.1.7 license-only rebuild

- Scope: offline program/third-party notices, native bundle resources, packaging guards and documentation only. Application source, tests, version and dependency/runtime versions remain unchanged from the prior 0.1.7 checkpoint.
- Fresh canonical verification: all 11 configured checks PASS; frontend 85 passed; Rust 400 passed with 9 ignored; isolated restic 8 PASS checks. The final notice-hash check was repeated successfully before and after packaging, following the final inventory update.
- Final local candidate after the conservative LGPL source/relinking update: 51,249,947 bytes, SHA-256 `825e74e59be3954f091d829761cb01f673f9c9330a05d910946ddf3ba10170c0`; unsigned Windows x64, package command exit 0. Five notice files plus the exact rclone/SDDL source bundle, relinking notice and build manifest are present in the generated NSIS inclusion instructions. No installer extraction/installation was run for this candidate.
- Ten notice/source-material guard fixture cases passed: valid payload, missing/altered notices and LGPL source materials, omitted resource, stale dependency input, LF/CRLF portability, and duplicate-input rejection.
- This correction does not repeat uninstallation, installation or native startup. The historical installation evidence below applies only to its stated earlier hash.
- Technical publication readiness is YES after the conservative LGPL treatment. The material is redistribution evidence, not legal clearance; exact user authorization and post-push Actions success remain required.

## Earlier 0.1.7 local verification and replacement installation

- Fresh `scripts/verify.ps1`: exit 0, all 10 configured checks PASS; frontend 85 passed; Rust 400 passed, 0 failed, 9 ignored; isolated restic 8 PASS checks. No real profiles, credentials, model calls or cloud accounts were used.
- Fresh `scripts/package.ps1 -SkipBootstrap`: exit 0, Windows x64 NSIS, 33,292,697 bytes, SHA-256 `0678d177527c9f8f54b83468d158b275572d80055c899401f0cdbf68bcb373e8`. Matching sidecar; executable version 0.1.7; unsigned. Existing cached runtimes/MSVC were reused, with Cargo offline.
- Current-user 0.1.6 uninstall and 0.1.7 install both exited 0. Registry and executable product version report 0.1.7. Main executable matches the build after the documented Tauri UNK-to-NSS bundle marker; restic/rclone match byte-for-byte.
- Existing application configuration and DPAPI password file were unchanged by hash. No application-data deletion option or backup-directory cleanup was used; old 0.1.6 installer retained for recovery.
- Desktop and Start Menu shortcuts point to the new executable and inherit its icon. This is a shortcut metadata check, not visual confirmation of Windows icon rendering.
- The new installed application was **not launched**. Native UI, live-account conversation display/continuation, second-device and OneDrive checks remain **NOT_RUN**. Prior synthetic browser evidence below is not a native acceptance result.
- Both verification and packaging retain the linker-output warning about creating the import library/exports. These are successful-command diagnostics, not omitted failures.

Overall task status: **PARTIAL** — local build and publication preflight complete; GitHub publication and native/live acceptance outstanding.

## Current behavior and evidence scope

- Backup/restore results distinguish `complete`, `warning` and `partial`, with aggregated security exclusions and automatic handling, and named data warnings/missing paths. Dependency links may need package-manager reinstallation; offline operation is not guaranteed. Codex credentials require sign-in again; malformed JSONL may affect the named conversation. A missing project root remains missing. Old manifests are read without rewriting their recorded classification.
- Encrypted restic full project backups preserve readable regular project files, Git/worktree data and sensitive project files. Codex data has separate exclusions. ReHome packages contain selected content with security/Git exclusions and are not encrypted restic repositories.
- ReHome has file-only and migrate-and-connect modes. File checks alone do not prove Codex access. Connect mode checks every planned target ID through App Server, then sends one neutral message on one user-selected ephemeral fork using that conversation's context. Original conversations receive no probe message. This is not a test of every conversation's full history/tools or pixel-level desktop visibility. Explicit online consent covers context transmission and possible usage; local rollback cannot undo server processing or usage.
- Confirmed helper shutdown permits ordinary failure rollback. Unconfirmed shutdown stops database checkpoint/rollback writes and retains transaction materials for manual recovery. Errors provide cause/remedy and History guidance; neither a status-query error nor an earlier files-verified stage proves the final rollback outcome.
- Development checked the installed CLI schema offline at `0.155.0-alpha.16`. This records a protocol shape, not a minimum support version or a requirement to install an alpha. Missing capabilities or unconfirmed safety settings fail closed. Protocol background: [official Codex App Server reference](https://learn.chatgpt.com/docs/app-server).
- Controller's Task 5 synthetic browser review and its fix recheck passed for backup warning/partial results, two modes, selection/consent, progress/navigation, success, rollback, cleanup-unconfirmed and no-conversation outcomes in Chinese/English. All IPC was mocked; this is browser evidence, not backend E2E or installed-native acceptance. It predates M3 and was not duplicated; M3's wording is covered by the real-translation tests below.

## Historical development artifact — M3 guidance-fix refresh (0.1.6)

M3 is implemented: the bilingual verification-failure remedy now conditionally advises checking enabled MCP/other integrations manually in Codex or explicitly selecting file-only restore. It does not attribute every failure to integrations, disable integrations automatically, or introduce an automatic fallback. Only three frontend files changed; the 30-file post-fix source manifest matches exactly.

On 2026-09-23, the full frontend command ran once: **6 test files, 85 passed, 0 failed, no skipped tests reported**. The offline package command also ran once and exited 0, including a successful TypeScript/Vite build (1,589 modules). No redundant standalone build, Rust test run or full `verify.ps1` rerun was performed for this frontend guidance change. Earlier Rust/restic evidence remains dated to the first checkpoint below.

The refreshed unsigned Windows x64 installer is `desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/ENHE Codex Backup_0.1.6_x64-setup.exe`: **33,303,013 bytes**, SHA-256 **`27845f98dddf1a1501260933479f85026c9af470f63b657f89fd82beca0cb2fc`**. An independent hash calculation matches the adjacent `.sha256` and exact filename; FileVersion/ProductVersion remain 0.1.6. Packaging retained one release linker-output warning plus its summary. This file replaces the first-checkpoint artifact at the same development output path; it was not installed or launched.

The first checkpoint's 13 evidence files (brief, rulings, report and 10 logs) are preserved unchanged. The scoped refresh scan covered 98 publishable/source/configuration files; its 18 matches are the same synthetic literals, field references and pre-existing contact-like entries already classified, with no new credential identified. Independent M3 fix review is **PASS**: M3 addressed, no new Critical/Important/Minor findings in the three-file delta. That review inspected source identity and existing focused logs without rerunning tests. The separate final documentation/evidence review is also **PASS**, with no Critical/Important findings; its minor evidence-file naming correction is reflected here.

## Historical first frozen checkpoint — local verification (0.1.6)

The final shared run on 2026-09-23 used existing runtimes and MSVC x64, with Cargo offline and locked tests. The first three canonical commands passed: frontend tests (6 files, 83 tests), production build, and `scripts/verify.ps1` (all 10 checks). Verification included 400 Rust tests passed, 0 failed, 9 ignored, and 8 isolated restic fixture checks passed. See [verification/local-restic-acceptance.md](verification/local-restic-acceptance.md); its only tracked change is the regenerated timestamp.

The 9 ignored Rust tests comprise 5 bundled-restic application tests, 1 large synthetic count fixture, 2 real-profile tests and 1 desktop fixture generator. They were not enabled. Windows privilege-dependent early-return branches are not separately exposed by the captured default test output; no claim is made that every symlink branch ran. Verification retained one linker-output warning about creating the debug import library/exports; this was not a test failure. Detailed counts and limits are in [ACCEPTANCE](ACCEPTANCE.md).

`powershell -ExecutionPolicy Bypass -File ./scripts/package.ps1 -SkipBootstrap` also exited 0: Windows x64 release build and one unsigned current-user NSIS bundle produced. That first output was 33,304,651 bytes, SHA-256 `5e9ed5b40fc10317595132627472a537880554db24a2fb871b16a2634c5073b8`, with matching `.sha256` and file/product version metadata 0.1.6 at inspection. Its output path has since been replaced by the refreshed artifact above. Packaging retained one release linker-output warning plus its summary. No installation or native launch was performed.

These four command results identify the **first frozen source checkpoint**, before Minor M3. Controller reported independent source review PASS with 0 Critical/Important findings. M3 and its affected frontend/package refresh are recorded above; the first-checkpoint results are not relabeled as a new full verification of that fix.

## Historical development-checkpoint limits (before 0.1.7 packaging)

- Installation, native app launch/click-through, native folder dialogs, clean-profile WebView2, and real Task Scheduler execution: **NOT_RUN**. Windows x64 compilation and synthetic/browser evidence do not cover these operations.
- Real Codex profiles/accounts, second physical device, original-conversation desktop visibility, and real online continuation/model usage: **NOT_RUN**. No personal migration package or real profile was audited or tested.
- OneDrive authorization/upload/download and external/cloud recovery: **NOT_RUN**. Cloud remained off in local tests.
- Broader submodule/LFS, disk-full/large-resource and installed-app interruption matrices remain unverified; see [ACCEPTANCE](ACCEPTANCE.md).
- Independent final documentation/evidence review passed separately from the executor's self-review and the scoped M3 source review. It does not authorize release or change the NOT_RUN limits above. No staging, commit, push, publication or release was performed.

Overall release readiness: **PARTIAL**. Local development verification is distinct from release and live-account/device acceptance.

## Historical published baseline — 0.1.6, 2026-09-20 checkpoint

These are retained prior results from the original 2026-09-20 status checkpoint, not rerun installation evidence for the 2026-09-23 development build:

- Frontend build and 44 Vitest tests, canonical verification (including 72 Rust library tests and non-environment-dependent integration suites), isolated restic acceptance, and an asynchronous bundled-restic synthetic backup/restore round trip were recorded PASS.
- The earlier Windows x64 NSIS installer was 33,247,826 bytes with SHA-256 `9ddf7bb14a54153ecc23dd46b2197b31bc9cd75f67fab06d8a17bb8d7d16c058`. That historical package and checksum must not identify the new artifact.
- The prior current-user installation was removed and the baseline 0.1.6 installed through the then-authorized path. Registry, binary file/product version, window title, sidebar version and captured startup UI were recorded PASS. Only startup/primary overview was visually verified, not full native click-through or backup with personal data.
- The historical release-readiness decision was PARTIAL; cloud, account and second-device continuation remained NOT_RUN. Those gaps are not closed by retaining these records.
