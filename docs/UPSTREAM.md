# Upstream and third-party boundary

## ReHome core

The migration UI and Rust migration core originate from the pinned ReHome source commit:

`cc5daae501b22b86e1b4c2dafbd1b4af61992b48`

Reference: [ReHome README at the pinned commit](https://github.com/CalebYcj/codex-rehome/blob/cc5daae501b22b86e1b4c2dafbd1b4af61992b48/README.md).

ENHE keeps the applicable `.rehome` migration format and transaction flow, then adds a separate complete local-backup path. Complete backup intentionally preserves `.git`, uncommitted changes, worktree metadata, Codex files, and handoff material instead of copying ReHome's default exclusions.

## Product changes in this workspace

- Product identity, Tauri metadata, and UI navigation are ENHE Codex Backup.
- The upstream updater plugin and update controls are removed; there is no telemetry or automatic update path.
- restic is used for encrypted local snapshots and independent restore.
- rclone/OneDrive is an opt-in adapter. Cloud is disabled in the default config and is never invoked by local backup.
- Current-user DPAPI and Task Scheduler integration are added for the optional worker.

The upstream license and notices remain in the repository. Do not represent ENHE as an official OpenAI or ReHome distribution.
