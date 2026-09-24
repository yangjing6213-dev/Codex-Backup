# Acceptance mapping

Checkpoint: 2026-09-23; **0.1.7 locally installed, publication pending**. The complete acceptance baseline is the supplied `ENHE-Codex-Backup-完整开发执行指令.md`. This table separates observable local evidence from unverified native/account/device behavior. Fresh 0.1.7 verification passed all 10 checks (frontend 85, Rust 400 passed/9 ignored, restic 8); authorized uninstall/install, registry/binary/resource/shortcut checks and configuration preservation passed. The installed app was not launched. See [STATUS](STATUS.md) and [installer verification](verification/installer-latest.md). Earlier 0.1.6 development runs below remain dated evidence, not native acceptance.

| Case | Evidence and remaining boundary | Status |
| --- | --- | --- |
| A01 local offline | Local backup defaults to cloud off; local adapter tests and isolated restic CLI acceptance pass. The asynchronous bundled-restic application test is ignored in this canonical run. This build's installed-app offline flow is NOT_RUN. | PARTIAL |
| A02 Git/worktree | Synthetic staging/restore tests and isolated restic fixture cover Git metadata, uncommitted/untracked files and worktree data. Complete restic backup differs from selective ReHome security/Git exclusions. Submodule/LFS and installed-app matrix remain open. | PARTIAL |
| A03 conversations/WAL | Synthetic tests cover JSONL warnings, SQLite snapshots/sidecars, index/metadata restoration, all planned target IDs and one ephemeral continuation probe through a fake protocol. Real account, full historical/tool usability and desktop visibility are NOT_RUN. | PARTIAL |
| A04 history/retention | Retention and snapshot listing exist; UI tests cover migration History navigation and busy rollback controls. Full installed history/retention/deletion/source-missing matrix is NOT_RUN. | PARTIAL |
| A05 paths/resources | Synthetic validation/classification tests keep truly missing project roots visible and distinguish dependency links from ordinary redirects. Long-path, disk-full and large-resource matrix is NOT_RUN. | PARTIAL |
| A06 transaction/rollback | Synthetic transaction tests cover verified commit, ordinary-error rollback, WAL/sidecars, conflict preservation and unconfirmed helper exit with no database checkpoint/rollback writes. Interrupted installed-app recovery remains NOT_RUN; manual recovery is not guaranteed. | PARTIAL |
| A07 package/security | Synthetic path/schema/symlink and credential exclusion tests, probe restrictions and scoped publishable-file secret review apply. Complete malicious archive/hooks/MCP runtime matrix remains NOT_RUN; no real profile audit. | PARTIAL |
| A08 scheduler/concurrency | Current-user scheduler/DPAPI and operation serialization have local tests; real scheduled task execution is NOT_RUN. | PARTIAL |
| A09 cloud switch | Default-off and no-remote local adapter behavior have synthetic coverage. Opt-in online Codex verification is distinct from restic cloud backup. Installed-app isolation is NOT_RUN. | PARTIAL |
| A10 cloud failure/live recovery | rclone protocol implementation exists; no OneDrive account authorization or remote recovery was performed. | NOT_RUN |
| A11 bilingual UI | Refreshed React suite: 85 passed, including M3's two real bilingual remedy regressions. Controller's pre-M3 synthetic browser PASS plus Task 5 fix recheck covers warning/partial results, modes, consent/context, progress, failure/History and zero-conversation states. M3 was not browser-rechecked. Native folder picker, full native click-through and screen-reader runtime checks are NOT_RUN. | PARTIAL |
| A12 installer/device | Fresh 0.1.7 Windows x64 packaging and authorized current-user uninstall/install PASS; registry, payload hashes, shortcuts and preserved configuration verified. Exact artifact is in STATUS. Installed launch/UI, second device and real-account continuation are NOT_RUN. | PARTIAL |

## Consolidated feature acceptance

- Backup classification is evidence of omissions/impact, not a promise of zero work after restore. Rebuildable dependencies may need installation and network; Codex re-login is required for excluded credentials. Malformed JSONL is retained with affected-conversation guidance; red missing roots mean the project is absent. Old manifests are not rewritten or silently upgraded to complete.
- File-only restore never claims recognition or continuation. Connect mode checks all planned target IDs but probes only one selected ephemeral fork, without messaging an original thread. No-conversation packages remain file-only. Recognition is an App Server response, not native pixel verification; tools and full history are not certified.
- Online consent includes selected conversation context, configured model service and possible usage. Local rollback cannot undo transmitted processing or usage. Required capabilities and safe settings must be confirmed; the offline schema check at CLI `0.155.0-alpha.16` does not define a supported version range.
- Ordinary failures after confirmed helper termination attempt rollback. Unconfirmed termination preserves materials and stops database writes for manual recovery; no forced overwrite or unconditional rollback-success claim. A temporary status-query failure does not establish a transaction outcome. Cause/remedy and transaction ID guide the user to History.

## First frozen checkpoint — shared local run

The two feature plans share one final run, not duplicate per-plan verification. The scripts' own repeated frontend builds/tests are included in their respective commands.

| Command | Exit/result | Observed evidence |
| --- | --- | --- |
| `pnpm --dir desktop test -- --run` | 0 / PASS | 6 test files, 83 passed, no failed/skipped tests reported. |
| `pnpm --dir desktop run build` | 0 / PASS | TypeScript and Vite production build, 1,589 modules transformed. |
| `powershell -ExecutionPolicy Bypass -File ./scripts/verify.ps1` | 0 / PASS | All 10 checks passed: frontend typecheck/tests/build, bilingual docs, version, installer icon, Rust formatting/tests, updater scan and isolated restic. Frontend: 83 passed. Rust: 400 passed, 0 failed, 9 ignored, 0 filtered. Restic report: 8 PASS rows. |
| `powershell -ExecutionPolicy Bypass -File ./scripts/package.ps1 -SkipBootstrap` | 0 / PASS | Existing MSVC x64 and cached tools; Cargo offline, no bootstrap/install. One NSIS installer, 33,304,651 bytes; independently calculated SHA-256 matches sidecar, file/product version 0.1.6. See STATUS for path/hash. |

Ignored Rust cases: `backup_async_test` (1 bundled-restic round trip); `full_project_backend_test` (4 bundled-restic cases and 1 large count fixture); `local_windows_acceptance_test` (2 real-profile cases); `restore_test` (1 synthetic desktop fixture generator). They remain NOT_RUN in this final run even when prerequisites exist. Windows symlink tests can return early if privilege is unavailable; successful-test output is captured by the default harness, so exercised versus early-return branches cannot be counted from this log. No extra runtime skips were reported; this is not a claim of complete platform coverage.

Verification and packaging each emitted one Rust linker-output warning for creating the respective debug/release import library/exports, plus a warning-summary line. Both exited successfully. No canonical command was rerun to hide diagnostics. No native app, Codex account or model request was launched by Task 6.

This is the first frozen checkpoint, before the now-implemented M3 bilingual MCP/integration refusal-remedy improvement. Controller reported independent source review PASS with no Critical/Important findings. The separate refresh below covers the affected frontend/package surfaces; the first-checkpoint results are not a rerun for M3, and its former installer has been replaced at the same output path.

## Subsequent M3 guidance-fix refresh

| Check | Exit/result | Observed evidence |
| --- | --- | --- |
| `pnpm --dir desktop test -- --run` — once | 0 / PASS | 6 files, 85 passed, no failed/skipped tests reported; includes both real-translation remedy regressions. |
| `powershell -ExecutionPolicy Bypass -File ./scripts/package.ps1 -SkipBootstrap` — once | 0 / PASS | Includes TypeScript/Vite production build, 1,589 modules, and one Windows x64 NSIS bundle. No extra standalone build. |
| Installer metadata/checksum | PASS | 33,303,013 bytes; SHA-256 `27845f98dddf1a1501260933479f85026c9af470f63b657f89fd82beca0cb2fc`, matching sidecar; FileVersion/ProductVersion 0.1.6. Not installed or launched. |
| Frozen source and prior evidence | PASS | All 30 post-fix source hashes match; all 13 first-checkpoint evidence files (brief, rulings, report and 10 logs) unchanged. |
| Scoped secret check | Completed | 98 files, 18 matches in 8 files; same classified synthetic/field/pre-existing contact-like matches, no new candidate in the three M3 files. No real profiles or ignored trees scanned. |
| Rust tests / full `verify.ps1` | Not rerun | Frontend-only guidance/test delta; retain the first checkpoint's dated 400 passed / 9 ignored and restic evidence without claiming a new run. |
| Independent M3 review | PASS, scoped | M3 addressed; no new Critical/Important/Minor findings in the three-file delta. Source/file identity and existing RED/GREEN/build logs inspected; no reviewer test rerun and no package/release conclusion. |

The refreshed package retained one release linker-output warning plus its summary; no test/build failure or frontend warning was reported. M3 adds conditional manual integration/file-only advice while preserving generic cause and no automatic fallback. Native, real-account, second-device and online model acceptance remain NOT_RUN. Independent final documentation/evidence review passed with no Critical/Important findings; its minor evidence-file naming correction is reflected above. This does not change release or live acceptance to PASS.

The generated local report is [verification/verify-latest.md](verification/verify-latest.md) when present (ignored local evidence). [Isolated restic acceptance](verification/local-restic-acceptance.md) covers the storage engine with temporary synthetic data, not native Codex access. No PARTIAL or NOT_RUN row may be reported as PASS.
