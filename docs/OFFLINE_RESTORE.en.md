# Offline restore guide

This guide does not require a GPT login, a website, or the original app cache. You need the ENHE installation directory (or a standard restic executable) on the target Windows x64 device, the local restic repository, and the recovery password.

## Restore with the app

1. Move the complete local repository to a local disk on the new device; do not place it inside the restore target.
2. Install and open ENHE Codex Backup without configuring cloud.
3. In Backups & Migration enter the repository and recovery password, then refresh snapshots.
4. Choose an independent restore target and restore. Inspect `backup-<id>\projects` and `backup-<id>\codex` for projects, Codex data, and handoff material.

The app rejects repository/target overlap, existing-target overwrite, traversal, invalid manifests, and symlinks in the restored payload. A failed restore may leave a `.partial` directory owned by that operation; after confirming the app has exited, it can be removed. Ordinary deletion is not secure erasure.

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

## Wrong password or damaged repository

Keep a read-only copy of the original before attempting recovery actions. For a wrong password, missing pack/index/config, or a failed check, record a redacted error and restore from another independent backup. Do not delete the original repository or run scripts/hooks from an untrusted source.
