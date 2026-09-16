# Acceptance mapping

The complete acceptance baseline is the supplied `ENHE-Codex-Backup-完整开发执行指令.md`. The table records what is observable in this workspace and what remains unverified.

| Case | Evidence in this workspace | Status |
| --- | --- | --- |
| A01 local offline | Local path is the default; cloud-off state and no-remote adapter tests pass; Rust/Tauri release compilation and isolated restic acceptance pass; installed-app flow is only startup-verified | PARTIAL |
| A02 Git/worktree | Real restic CLI fixture restores Git metadata, uncommitted/untracked files and a worktree file; app staging captures common/worktree metadata and repairs copied pointers; submodule/LFS and installed-app coverage remain open | PARTIAL |
| A03 conversations/WAL | Real fixture restores active/partial JSONL and SQLite WAL/SHM files; app staging uses SQLite online backup for recognized databases and quarantines their sidecars; real Codex session visibility is not verified | PARTIAL |
| A04 history/retention | Retention code and snapshot listing exist; app integration and deletion/source-missing matrix are NOT_RUN | NOT_RUN |
| A05 paths/resources | Path validation and exclusion tests exist; long-path, disk-full, and large-resource matrix is NOT_RUN | PARTIAL |
| A06 transaction/rollback | Upstream migration transaction core is embedded and Rust tests pass; interrupted installed-app runs are NOT_RUN | PARTIAL |
| A07 package/security | Path/schema/symlink checks and credential exclusion tests exist; malicious archive/hooks/MCP matrix is NOT_RUN | PARTIAL |
| A08 scheduler/concurrency | Current-user Task Scheduler arguments, worker lock and DPAPI implementation exist and Rust tests pass; real task run is NOT_RUN | PARTIAL |
| A09 cloud switch | Default config is disabled; local UI path has no cloud call; adapter and Rust integration tests pass; installed-app isolation is NOT_RUN | PARTIAL |
| A10 cloud failure/live recovery | rclone protocol implementation exists; no OneDrive account authorization or remote restore was performed | NOT_RUN |
| A11 bilingual UI | TypeScript, build, DOM flow tests, and local browser preview of navigation/settings/language switch pass; native folder-picker click-through and desktop screenshot/WebView visual check are NOT_RUN | PARTIAL |
| A12 installer/device | New Windows x64 NSIS installer was generated with a matching SHA-256 file, installed in the current user profile, and started with a responsive application window; clean-profile and second device/account tests are NOT_RUN | PARTIAL |

The detailed latest run is in `docs/verification/verify-latest.md` when `scripts/verify.ps1` has been run. No PARTIAL or NOT_RUN row may be reported as PASS.
