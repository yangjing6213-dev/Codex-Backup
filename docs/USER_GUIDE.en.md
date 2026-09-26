# ENHE Codex Backup User Guide

This guide covers 0.1.10 (2026-09-27). Local build or installation does not prove GitHub publication or real-account continuation. See [STATUS](STATUS.md) for the evidence boundary.

## First run

Open Overview after launch. Seeing cloud marked “off” is the expected default; it does not block local discovery, backup, history, or restore.

At startup the app loads saved configuration before scanning the configured Codex location. If no Codex path is configured, it tries the current Windows user's `.codex`. If that path is missing, the attempted path is shown and Choose Codex data location opens a retry flow. Project discovery has configurable scan roots and an automatic scan toggle. When enabled, it discovers candidates within those roots in the background; with no roots configured, it uses local fixed drives. Rescan after changing the scope and review the latest candidates and permission notices. Discovery does not mean a project is backed up: review project selections and save them. Manual project addition remains available when automatic scanning is off.

Protected Windows folders are safely skipped and shown as an aggregate inaccessible-folder count instead of flooding the page with “access denied” lines. If needed, check Request administrator permission for restricted folders in Projects, click Rescan as administrator, and respond to UAC.

1. Open Projects, select the projects that belong in a complete backup; add non-Git folders through “Add a local project folder” when needed, then save.
2. Open Data, confirm the Codex data location, and save. Use the folder button beside the path to open the Windows picker; manual text entry remains available. Paths may contain Chinese characters, spaces, removable drives, or long names.
3. Open Backups & Migration, choose a backup folder for the encrypted repository, enter a recovery password, and choose Start local backup. The password encrypts the restic repository; losing it makes the repository unrecoverable.
4. To schedule backups, enable the current-user scheduled task and select Remember password. The password is protected with DPAPI for the current Windows user; without it, the worker reports that no password is available.
5. If you are new to the workflow, open How it works and follow the visual flow from automatic scan through restore or offline migration.

You can navigate between pages while a backup runs; the backup and its result remain available. Keep ENHE Codex Backup open: do not quit the app or shut down the computer. Turning cloud off during a local backup does not affect the local operation. The app manages only the restic child processes it starts.

A backup repository is the storage directory containing encrypted snapshots. It is separate from source projects and the restore destination. Keep the whole repository and its recovery password safe, separately. Remember password works only for the current Windows user; it does not replace the password needed on another computer.

## Inspect and restore

In Backups & Migration, confirm the repository location, enter its recovery password, and choose Refresh local backups. Choose a new empty Restore target folder separate from the repository and original data, then choose Restore on a snapshot. The app restores into private staging, checks `enhe-payload` and its manifest, and commits to `<restore target>/backup-<logical_backup_id>/`. Use the actual restored path returned by the app.

Inside that directory, `manifest.json` records sources, counts, exclusions, and missing content; `codex/` contains Codex files; and `projects/<project ID>/` contains project files, using IDs rather than original project names. Where needed, `git-metadata/<project ID>/worktree/` and `common/` preserve Git metadata. The app repairs recognized worktree pointers, so retain this entire layout when checking or moving restored data instead of moving a worktree folder alone.

If `backup-<logical_backup_id>` already exists, the app reports a conflict and does not overwrite it; choose another empty destination. Restored files are not executed as scripts, Git hooks, or MCP configuration; inspect them before opening a project.

Successful file recovery does not prove that original conversations are visible or can continue in Codex. First inspect the manifest, projects, and session data in the isolated restore directory. Applying them to a real Codex profile is a separate operation: back up the existing profile and independently verify compatibility, paths, indexes, and conversation continuation. Do not automatically overwrite the active `.codex` directory. File integrity or project registration alone is not proof that a conversation can resume.

## Complete backup scope

Starting with 0.1.3, full local project backup includes every readable regular file: hidden files, `.git`, uncommitted/untracked/ignored files, `node_modules`, build outputs, caches, and sensitive files such as `.env`, private keys and tokens. There is no extension filter. Recognized worktree metadata is retained. More files increase initial backup time and temporary disk use; keep the backup repository outside your projects.

Sensitive project files go into the encrypted restic repository. Use a strong independent recovery password, keep it separately, and never publish restored files or upload them to GitHub. Cloud remains off by default and nothing is uploaded automatically. **The Codex data location keeps its separate safety exclusions**: known login credentials, cookies, private keys and `.env` are excluded; dependencies, designated caches and runtime temporary files are reported as notices. Its active JSONL/SQLite capture rules are unchanged. Symbolic links and filesystem redirects are not followed. Recognized rebuildable dependency links and test-artifact links under `.local-audit` become notices; other links and unreadable or unsupported entries remain missing content.

When the project scan directory is `F:\Projects`, each direct child folder is listed by its actual folder name. Nested `.next` and test fixtures contribute files instead of becoming separate projects. Leaving the scope empty uses cancellable whole-disk discovery with depth/time limits. Rescanning replaces current discoveries; saved manual and historical paths are listed separately.

File counts are measured recursively in the background, including hidden files, Git, dependencies, build outputs and secrets. The UI shows “Counting files…” until a real count is available, and marks permission errors, redirects or cancellation as partial with skipped-entry counts. Only genuinely empty folders show zero. Counts measure regular files enumerable at scan time, not proof that their contents remain readable. Final backup counts, extra Git metadata and active-file changes are recorded in `manifest.json`.

## Understand backup and restore results

“Backup completed” (`complete`) means no notices, data warnings or missing items were reported; explicit security exclusions can still exist. “Backup completed with items to review” (`warning`) means no missing content was reported, but automatic handling or data warnings need attention. “Backup partially completed” (`partial`) means content is missing and the snapshot is not a complete copy. Restore carries the manifest's classification forward; it cannot recover content absent from the snapshot.

Open “View result details” and act on the impact:

| Category | Meaning and next action |
| --- | --- |
| Security exclusions | Codex sign-in credentials and other protected content were excluded; sign in again normally after restoring. |
| Handled automatically | Rebuildable dependency links, caches, runtime temporary files or test artifacts were omitted. Dependencies may need reinstalling with the project's package manager; offline operation is not guaranteed. Applications recreate runtime files; regenerate test artifacts as needed. This label does not mean zero impact or that dependencies were already reinstalled. |
| Data warning | Malformed or truncated JSONL was retained, but the named conversation may be incomplete. Keep the original file and inspect that conversation; this does not mean all project files are lost. |
| Files missing | A red missing project path means that source project is absent from this snapshot. Correct or reselect the path, reconnect its disk if necessary, and back up again. Deselect only a project you no longer need. Follow the named paths for permission failures, copy failures and ordinary links too. |

Repeated security exclusions and automatic handling are aggregated by count; data warnings and missing files retain affected paths and remedies. The new classification does not rewrite old `manifest.json` files. Older manifests may show an unclassified issue and retain their original partial status; upgrading must not conceal genuinely missing data.

## Move a complete backup to another computer

Wait for backup to finish, then copy the entire encrypted repository directory. On the new computer, point the local repository setting to that copy, enter the original recovery password, refresh snapshots, and restore to a new empty directory. Do not copy just one snapshot file or repository subdirectory: recovery requires the complete repository and password. The original computer's saved DPAPI password is not a substitute. This recovers files; applying a real Codex profile and continuing conversations still require the separate checks above.

## ReHome migration

Export ReHome migration package and Import ReHome migration package handle only selected projects, conversations, Skills, plugins, and generated images, with ReHome's own security exclusions, including `.git`. Selecting everything still does not create a complete restic backup; selecting a conversation alone does not include project files. An exported `.rehome` file is not an encrypted restic repository and does not replace a complete backup or its password protection.

To import, choose the `.rehome` file and review its contents and the displayed Codex data location. Choose a project destination when the package contains projects. Preview the exact paths to be written: projects go into `<project destination>/<packaged project name>/`, while sessions and indexes go into the plan's target Codex location. This differs from the local snapshot restore layout above. Save work and fully exit the target Codex before confirming import. Review conflicts before choosing to keep local files or use package files; the app manages safety backups before replacement.

After previewing, choose a restore mode:

- **Restore files only** restores files, sessions and indexes with local checks. It does not start online access verification or prove Codex recognition or continuation. If no conversations map to the plan, only this mode is available; the result explicitly says conversations were not verified.
- **Migrate and connect to Codex (recommended)** requires a network connection and sign-in or authentication as required by the configured model service. Save work and fully close the target Codex and its helpers. Select one planned conversation and explicitly consent to online verification and usage before starting. Keep ENHE Codex Backup open until the job finishes.

Connection progresses through “Files restored”, “Conversations recognized by Codex” and “Temporary branch can continue messaging”. Recognition covers every planned target conversation ID. Continuation sends one fixed neutral verification message only on a separate temporary fork of the selected conversation and requires a matching nonempty reply and completion result. It does not append the message to an original conversation, test each other conversation, or prove full history, tool use or every project workflow. Recognition is an App Server result, not pixel-level proof of visibility in the desktop UI.

The temporary fork uses the selected conversation's context with the configured model service, not just an isolated neutral prompt, and may incur model usage. Local rollback cannot undo processing of transmitted content or usage. Turning off restic cloud backup does not block online connection verification that you explicitly consent to.

Codex App Server must support the required capabilities and safety restrictions. Verification stops if temporary forks, read-only restrictions or other required capabilities cannot be confirmed; incompatible configuration such as enabled MCP integrations can also block it. Follow the error guidance to check configuration or update Codex through official channels, then preview again. There is no automatic fallback to messaging an original conversation or restoring files only. CLI `0.155.0-alpha.16` protocol schemas were checked offline during development; this is neither a minimum supported version nor advice to install an alpha. See the [official Codex App Server reference](https://learn.chatgpt.com/docs/app-server) for protocol background.

## Migration failures and History

Errors include a cause and remedy. For an unavailable service, check the Codex installation and App Server capabilities. For authentication failures, sign in normally, check the model service configuration, then fully exit Codex. For recognition or continuation failures, check consent, the selected conversation, service and network, then preview again. If MCP or other integrations are enabled, inspect and resolve issues manually in Codex, or explicitly choose “Restore files only”. A generic verification failure is not necessarily caused by integrations; the app does not automatically disable them or change restore mode. A temporarily unavailable job status does not mean migration failed or prove rollback; do not start the job again.

An ordinary verification failure after confirmed helper termination triggers an automatic local rollback attempt. Use the final “Local changes were rolled back” or failure status, not an earlier “Files restored” progress step, to assess the resulting files. Rollback can fail, including when files have changed since migration.

“Codex helper termination is unconfirmed” means a helper may still be writing. The app stops database checkpoint and rollback writes, retaining the transaction, current files and automatic backups for manual recovery instead of rolling back against a live writer. Fully close Codex and its helpers, open History using the transaction ID, and inspect status and retained copies before deciding whether an available rollback or continue-rollback action is appropriate. Preserve the material and do not force an overwrite of newer data; continuing rollback is not guaranteed to succeed. Another rollback cannot start while migration is running.

Real-account, second-device, original-conversation desktop visibility and online continuation checks for this development build are `NOT_RUN`. Passing synthetic protocol or browser checks does not replace them.

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
- “Local project scan partially completed”: some directories were inaccessible, skipped, or canceled. The page shows aggregate counts; existing candidates remain usable, but partial results are not a claim of a complete disk scan.

See [Offline restore](OFFLINE_RESTORE.en.md) for standard restic commands and [STATUS](STATUS.md) for the current verification boundary.
