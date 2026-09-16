# ENHE Codex Backup

[中文](README.md) · [English user guide](docs/USER_GUIDE.en.md)

## 1. What is this repository?

ENHE Codex Backup is an independent Windows x64 local-first backup, restore, and offline migration utility. It preserves Codex data, project files, Git state, conversations, and handoff material without requiring cloud configuration or a GPT login. It is not an official OpenAI or ReHome product.

## 2. Who is it for?

### 1. Especially suitable for

- Codex and AI coding workflows that need inspectable local backups.
- Git, worktrees, uncommitted changes, and offline device migration.
- Windows users who need bilingual UI and light, dark, or system appearance.

### 2. Not suitable for

- Real-time cloud sync, team collaboration, or multi-cloud orchestration.
- Unattended overwrites of existing projects or restore targets.
- Official account recovery or a guarantee of live conversation continuation.

## 3. What does it produce?

- Creates encrypted, deduplicated local snapshots with standard restic and restores them into an independent directory without overwriting existing data by default.
- Keeps Git data, uncommitted changes, untracked files, related worktree data, Codex conversations/indexes, and development handoff material for selected projects.
- Includes the applicable ReHome `.rehome` offline migration flow as a separate, explicit entry point from complete local backup.
- Provides Simplified Chinese/English, light/dark/system appearance, and a current-user Windows scheduled task.
- Keeps cloud off by default. Settings include a real OneDrive/rclone configuration wizard and an explicit snapshot upload action; cloud-off mode starts neither rclone nor uploads.

## 4. What value does it provide?

It puts data backup and device migration into one local-first workflow, reducing dependence on cloud accounts and networks while preserving Git/worktree context and uncommitted work.

## 5. Example result

```text
Codex data location: C:\Users\<user>\.codex
Local backup repository: %LOCALAPPDATA%\ENHE\Codex Backup\backups
Discovered project: demo (.git, package.json)
Conversations: counted from Codex sessions / archived_sessions
Cloud: off; no remote call
```

Candidates are shown for user confirmation and are not automatically added to the backup scope. The folder button beside a path opens the Windows native folder picker; canceling leaves the old value unchanged.

## 6. Installation

1. Open the [GitHub Releases installer download page](https://github.com/yangjing6213-dev/Codex-Backup/releases), or directly download the [Windows x64 installer](https://github.com/yangjing6213-dev/Codex-Backup/releases/latest/download/ENHE.Codex.Backup_0.1.1_x64-setup.exe) and [SHA-256 checksum](https://github.com/yangjing6213-dev/Codex-Backup/releases/latest/download/ENHE.Codex.Backup_0.1.1_x64-setup.exe.sha256). If the page has no Release yet, use the local build steps below to create the installer.
2. Compare the installer SHA-256 with the sidecar in PowerShell.
3. Run the installer for the Windows current user.
4. Confirm the local data location on first launch; cloud may remain off.

Local build:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\bootstrap.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\verify.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\package.ps1
```

## 7. How to use it

The primary navigation is fixed to Overview, Projects, Backups & Migration, and Settings. For a first run:

1. Select the projects that belong in a complete backup; regular non-Git folders can be added manually.
2. Enter a local repository directory and recovery password in Backups & Migration, then run a local backup.
3. Restore through Restore target folder; an existing target is reported as a conflict and is not overwritten.

See the [English user guide](docs/USER_GUIDE.en.md) and [offline restore guide](docs/OFFLINE_RESTORE.en.md).

## 8. Project workflow

```text
Load config -> discover Codex -> fill safe local defaults ->
scan fixed-drive project candidates -> user confirms scope ->
restic snapshot -> verify/list -> restore to an independent directory
```

ReHome export/import is a separate migration entry point. Cloud does not participate in the local workflow; remote operations start only after an explicit cloud test or upload action.

## 9. Project directory structure

```text
desktop/                    Tauri + React desktop application
desktop/src/                bilingual UI, settings, backup, and migration flows
desktop/src-tauri/src/      Rust commands, discovery, restic, restore, and migration core
desktop/src-tauri/resources/ bundled restic/rclone runtimes
docs/                       specifications, acceptance, compatibility, guides, evidence
scripts/                    bootstrap, verification, local acceptance, and packaging
tests/                      isolated tests and documentation contract tests
```

## 10. Implementation notes

Credentials, cookies, private keys, `.env` files (except `.env.example`), dependency/cache directories, and runtime lock files are excluded from complete backups; `.git` is preserved and is not inherited from the migration tool's exclusion defaults. Restored payloads are treated as untrusted files: scripts, hooks, and MCP configuration are not executed. Remembered passwords use DPAPI scoped to the current Windows user; otherwise no password is stored.

Cloud connection, real OneDrive authorization, real-data upload, overwriting real Codex data, and system installation require explicit user action. Development acceptance uses synthetic data and temporary directories only.

## Verification and status

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\bootstrap.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\verify.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\package.ps1
```

The scripts pin and verify the Windows amd64 release archives for restic 0.19.1 and rclone 1.75.1. The current evidence includes an unsigned current-user NSIS package that was installed and started in the authorized test profile; it is not code-signed or auto-updated. See [STATUS](docs/STATUS.md) and [verification](docs/verification/).

See [COMPATIBILITY](docs/COMPATIBILITY.md), [UPSTREAM](docs/UPSTREAM.md), and [ACCEPTANCE](docs/ACCEPTANCE.md) for the compatibility and verification boundaries.

## 11. Version

Current version: `0.1.1`. This version focuses on Windows x64 local backup, restore, offline migration, automatic discovery, and bilingual settings. It also standardizes the home-page product title to “Codex Data Backup & Migration” and provides a new installer download. Real OneDrive, second-device, live conversation continuation, and clean-profile verification remain outside the completed evidence boundary; see [STATUS](docs/STATUS.md) and [ACCEPTANCE](docs/ACCEPTANCE.md).

## 12. Related projects

ReHome: this project includes its applicable offline migration capability, while complete local backup additionally preserves Git, worktrees, and handoff material.

## 13. About the author

### 1. Enhe - Product Designer - One-person Company Practitioner - AI Builder

Building a one-person company with AI.

- GitHub: [yangjing6213-dev](https://github.com/yangjing6213-dev)
- X/Twitter: [Amenenhe_ai](https://x.com/Amenenhe_ai)
- Website: [www.enhe-tech.com.cn](https://www.enhe-tech.com.cn/)
- WeChat: Hu-Amen
- Email: amen.enhe@gmail.com

[ENHE AI | AI tools, AI news, account services, and skills courses](https://www.enhe-tech.com.cn/)

## 14. Keep exploring

This project is one tool in the personal generation system I built with AI. If you are using AI for content, knowledge bases, workflows, or productization, visit [www.enhe-tech.com.cn](https://www.enhe-tech.com.cn/) for more material.

## License

This project retains the upstream repository's MIT license. Bundled components and versions are recorded in [THIRD_PARTY](docs/THIRD_PARTY.md).
