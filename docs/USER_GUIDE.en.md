# ENHE Codex Backup User Guide

## First run

Open Overview after launch. Seeing cloud marked “off” is the expected default; it does not block local discovery, backup, history, or restore.

At startup the app loads saved configuration before scanning the configured Codex location. If no Codex path is configured, it tries the current Windows user's `.codex`. If that path is missing, the attempted path is shown and Choose Codex data location opens a retry flow. The app then discovers project candidates on local fixed drives in the background. This scan reads directory metadata only; candidates must be checked before they enter the backup scope.

1. Open Projects, select the projects that belong in a complete backup; add non-Git folders through “Add a local project folder” when needed, then save.
2. Open Settings and confirm the Codex data location and local backup repository. Use the folder button beside each path to open the Windows picker; manual text entry remains available. Paths may contain Chinese characters, spaces, removable drives, or long names.
3. Open Backups & Migration, enter a recovery password, and choose Start local backup. The password encrypts the restic repository; losing it makes the repository unrecoverable.
4. To schedule backups, enable the current-user scheduled task and select Remember password. The password is protected with DPAPI for the current Windows user; without it, the worker reports that no password is available.

Turning cloud off during a local backup does not affect the local operation. The app manages only the restic child processes it starts.

## Inspect and restore

Choose Refresh local backups to list snapshots in the encrypted repository. Enter an independent Restore target folder, then choose Restore on a snapshot. The app restores into private staging, validates `enhe-payload` and its manifest, and commits to a `backup-<id>` directory.

If the target already exists, the app reports a conflict and does not overwrite it. Restored files are not executed as scripts, Git hooks, or MCP configuration; inspect them before opening a project.

## Complete backup scope

Selected projects retain project files, `.git`, uncommitted changes, untracked files, and recognized worktree metadata. The Codex data location is copied as files, including active/archived JSONL, SQLite files and WAL/SHM sidecars, unclassified content, and handoff material. Missing or damaged content is recorded in the manifest instead of being reported as complete.

Credentials, cookies, private keys, `.env` files except `.env.example`, dependency directories, caches, and runtime lock files are excluded. Complete backup does not inherit ReHome's `.git` exclusion.

## ReHome migration

Export ReHome package and Import ReHome package are for applicable offline device migration. They use the target Codex registration and migration transaction, and do not replace a complete restic backup. Close Codex on the target before importing. Review the plan when conflicts appear; use the migration history rollback action after a failed or canceled transaction.

## Optional cloud

Cloud is off by default. To configure OneDrive:

1. In Settings, enter an rclone config file, remote name, OneDrive path, and cloud repository path.
2. Choose Start OneDrive setup and answer each question shown by the wizard. Canceling does not enable cloud or upload a snapshot.
3. After the config file exists, explicitly enable cloud backup and save settings.
4. A remote operation starts only when you choose Test OneDrive connection or Upload this snapshot.

Local backup does not depend on cloud. Authorization failures, offline access, throttling, and timeouts affect only the explicit cloud operation; local snapshots remain on the local repository.

## Language and appearance

Use the sidebar language button to switch between Simplified Chinese and English. Settings offers light, dark, and follow-system appearance. Switching language does not clear form fields; keyboard users can tab through navigation, forms, errors, and action buttons.

## Common errors

- “Recovery password required”: no password was supplied and no current-user DPAPI password is available.
- “Repository overlaps a source”: move the local repository or restore target outside the source data.
- “Target already exists”: choose a new restore target; the app will not overwrite the old one.
- “Cloud configuration incomplete”: finish OneDrive setup and save it before enabling cloud.
- “Scheduled task unavailable”: the current-user task could not be registered or read; manual local backup remains available.
- “Local project scan partially completed”: some directories were inaccessible, skipped, or canceled. Existing candidates remain usable, but partial results are not a claim of a complete disk scan.

See [Offline restore](OFFLINE_RESTORE.en.md) for standard restic commands and [STATUS](STATUS.md) for the current verification boundary.
