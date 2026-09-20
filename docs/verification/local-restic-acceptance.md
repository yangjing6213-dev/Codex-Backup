# Isolated local restic acceptance

Generated: 2026-09-20T16:08:36.3761698+08:00

Synthetic fixture: Git repository with uncommitted and untracked files, linked worktree, active/partial JSONL, SQLite WAL/SHM sidecars, archived JSONL, and handoff notes.

| Check | Status |
| --- | --- |

| encrypted repository initialized | PASS |
| snapshot was created | PASS |
| Git metadata restored | PASS |
| uncommitted file restored | PASS |
| worktree file restored | PASS |
| active JSONL restored | PASS |
| SQLite WAL sidecar restored | PASS |
| handoff notes restored | PASS |

This is a real restic CLI test against a temporary encrypted repository. It validates the storage engine and restore layout only; it does not claim Rust/Tauri application integration, OneDrive E2E, account migration, or second-device continuation.