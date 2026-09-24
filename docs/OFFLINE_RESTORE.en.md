# Offline restore guide

This guide does not require a GPT login, a website, or the original app cache. You need the ENHE installation directory (or a standard restic executable) on the target Windows x64 device, the local restic repository, and the recovery password.

Offline recovery here means file recovery, excluding online model verification. This documentation covers 0.1.8 (2026-09-25); build/installation evidence in [STATUS](STATUS.md) does not replace recovery acceptance. Real-account, second-device and native UI checks are `NOT_RUN`.

## Restore with the app

1. Move the complete local repository to a local disk on the new device; do not place it inside the restore target.
2. Install and open ENHE Codex Backup without configuring cloud.
3. In Backups & Migration enter the repository and recovery password, then refresh snapshots.
4. Choose an independent restore target and restore. Inspect `backup-<id>\projects` and `backup-<id>\codex` for projects, Codex data, and handoff material.

The app rejects repository/target overlap, existing-target overwrite, traversal, invalid manifests, and symlinks in the restored payload. A failed restore may leave a `.partial` directory owned by that operation; after confirming the app has exited, it can be removed. Ordinary deletion is not secure erasure.

Review “Security exclusions / Handled automatically / Data warning / Files missing” in the manifest results. Excluded Codex credentials require sign-in again. Rebuildable dependency links may require the project's package manager to reinstall dependencies; offline operation after restore is not guaranteed. Malformed JSONL is retained but may affect the named conversation. A red missing project path means that source project never entered the snapshot. `warning` does not mean zero impact, and `partial` is not a complete copy. New classification does not rewrite old manifests.

Full restic project backup includes readable regular files, Git/worktree metadata, dependencies and sensitive project files; Codex data has separate safety exclusions. A ReHome `.rehome` package includes selected content with its own security exclusions, including `.git`, and is not an encrypted restic repository. The two formats are not interchangeable.

## Standard restic inspection

Replace the paths in PowerShell. The password file should be created by the current user and access-restricted; never put the password directly in command arguments or logs.

```powershell
$restic = "C:\Program Files\ENHE Codex Backup\resources\restic.exe"
$repo = "D:\ENHE\backups"
$passwordFile = "D:\ENHE\recovery-password.txt"

& $restic --repo $repo --password-file $passwordFile snapshots
& $restic --repo $repo --password-file $passwordFile check
```

## Restore a snapshot

Restore to a new temporary directory first and inspect it:

```powershell
$snapshotId = "replace-with-the-full-id-from-snapshots"
$target = "D:\ENHE\offline-restore"

& $restic --repo $repo --password-file $passwordFile restore $snapshotId --target $target
Get-ChildItem -LiteralPath (Join-Path $target "enhe-payload") -Force
```

ENHE snapshots use `enhe-payload` as their payload root. Standard restic restore does not make conversations appear in Codex automatically and does not execute restored content; inspect files manually, then reopen projects through the normal Codex flow. Original-account access, a second device, and thread continuation are not claimed as verified by this guide.

## ReHome file recovery versus online connection

“Restore files only” in ReHome import performs local restoration and checks without proving Codex recognition or continuation. For online verification, separately choose “Migrate and connect to Codex”, complete required authentication, connect to the network, close Codex and its helpers, and explicitly consent to usage. This mode is unavailable when no conversations map to the plan.

Online mode checks recognition of every planned target conversation ID, but sends one neutral message only on a temporary fork of the selected conversation. The fork uses that conversation's context with the model service and may incur usage. It does not append a verification message to an original conversation or prove every conversation's full history, tools or desktop visibility. Local rollback cannot undo server processing or usage.

An ordinary verification failure after confirmed helper termination triggers an automatic local rollback attempt. If termination is unconfirmed, database checkpoint and rollback writes stop and materials are retained for manual recovery. These transaction backups are not disposable restic `.partial` directories. Follow the error's cause, remedy and transaction ID into History, preserve current files and automatic backups, and fully close related processes before deciding on recovery. Never overwrite newer data. See [Migration failures and History](USER_GUIDE.en.md#migration-failures-and-history).

## Wrong password or damaged repository

Keep a read-only copy of the original before attempting recovery actions. For a wrong password, missing pack/index/config, or a failed check, record a redacted error and restore from another independent backup. Do not delete the original repository or run scripts/hooks from an untrusted source.
