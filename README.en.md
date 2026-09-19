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

Candidates are shown for user confirmation and are not automatically added to the backup scope. The folder button beside a path opens the Windows native folder picker; canceling leaves the old value unchanged. Inaccessible folders are safely skipped and summarized.

Projects includes a Request administrator permission for restricted folders option. Windows UAC appears only after the user checks it and clicks Rescan as administrator; the normal scan is not interrupted.

## 6. Installation

1. Open the [GitHub Releases installer download page](https://github.com/yangjing6213-dev/Codex-Backup/releases), or directly download the currently published [Windows x64 installer (0.1.3)](https://github.com/yangjing6213-dev/Codex-Backup/releases/latest/download/ENHE.Codex.Backup_0.1.3_x64-setup.exe) and [SHA-256 checksum](https://github.com/yangjing6213-dev/Codex-Backup/releases/latest/download/ENHE.Codex.Backup_0.1.3_x64-setup.exe.sha256). The 0.1.4 source is available; until a separately authorized 0.1.4 Release is published, use the local build steps below to create that installer.
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

The primary navigation includes Overview, Projects, Data, Backups & Migration, Settings, and How it works. For a first run:

1. Configure a scan folder in Projects, click Rescan, verify the direct child folder names and background file counts, then select the projects that belong in the backup.
2. Confirm the Codex data location on Data and the local repository directory in Settings.
3. If needed, check Request administrator permission for restricted folders in Projects and click Rescan as administrator. If UAC is canceled, the normal results remain available.
4. Enter a recovery password in Backups & Migration, then run a local backup.
5. Restore through Restore target folder; an existing target is reported as a conflict and is not overwritten.

The How it works page presents the same flow as an accessible visual guide.

See the [English user guide](docs/USER_GUIDE.en.md) and [offline restore guide](docs/OFFLINE_RESTORE.en.md).

## 8. Project workflow

```text
Load config -> discover Codex -> fill safe local defaults ->
scan fixed-drive project candidates -> summarize inaccessible folders -> user confirms scope ->
restic snapshot -> verify/list -> restore to an independent directory
```

ReHome export/import is a separate migration entry point. Cloud does not participate in the local workflow; remote operations start only after an explicit cloud test or upload action.

## 9. Project directory structure

```text
desktop/                    Tauri + React desktop application
desktop/src/                bilingual UI, settings, backup, and migration flows
desktop/src/App.tsx         main navigation, scan permission summary, and operation guide
desktop/src-tauri/src/      Rust commands, discovery, restic, restore, and migration core
desktop/src-tauri/resources/ bundled restic/rclone runtimes
docs/                       specifications, acceptance, compatibility, guides, evidence
scripts/                    bootstrap, verification, local acceptance, and packaging
tests/                      isolated tests and documentation contract tests
```

## 10. Implementation notes

Starting with 0.1.3, complete local project backups include every readable regular file, including hidden files, dependencies, build outputs, `.env`, private keys, and tokens. The Codex data location keeps separate safety exclusions for known login credentials and other high-risk data. Sensitive project files are stored only inside the encrypted restic repository: use a strong independent password and never publish restored content or commit it to GitHub. `.git` and related worktree data are preserved and are not inherited from the migration tool's exclusion defaults. Restored payloads are treated as untrusted files: scripts, hooks, and MCP configuration are not executed. Remembered passwords use DPAPI scoped to the current Windows user; otherwise no password is stored.

Cloud connection, real OneDrive authorization, real-data upload, overwriting real Codex data, and system installation require explicit user action. Development acceptance uses synthetic data and temporary directories only.

Windows may use the internal `\\?\` long-path form; configuration and UI normalize it to an ordinary drive or UNC path. Administrator scanning is an explicit UAC action, not an automatic elevation.

## Verification and status

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\bootstrap.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\verify.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\package.ps1
```

The scripts pin and verify the Windows amd64 release archives for restic 0.19.1 and rclone 1.75.1. The current evidence includes an unsigned current-user NSIS package that was installed and started in the authorized test profile; it is not code-signed or auto-updated. See [STATUS](docs/STATUS.md) and [verification](docs/verification/).

See [COMPATIBILITY](docs/COMPATIBILITY.md), [UPSTREAM](docs/UPSTREAM.md), and [ACCEPTANCE](docs/ACCEPTANCE.md) for the compatibility and verification boundaries.

## 11. Version

Current version: `0.1.4`. It retains the 0.1.3 complete-project backup, background counting, navigation-safe operations, and ENHE blue theme and icon, while fixing an upgrade issue that could leave an existing Windows desktop shortcut displaying the stale icon. The installer refreshes a shortcut only when one already exists, preserving a user's decision not to create one. The currently published GitHub download remains 0.1.3; publishing a 0.1.4 Release and installer assets requires separate authorization. Real OneDrive, second-device, live conversation continuation, and clean-profile verification remain outside the completed evidence boundary; see [STATUS](docs/STATUS.md) and [ACCEPTANCE](docs/ACCEPTANCE.md).

## 12. Related projects

ReHome: this project includes its applicable offline migration capability, while complete local backup additionally preserves Git, worktrees, and handoff material.

## 13. About the author

### 1. Enhe - Product Designer - One-person Company Practitioner - AI Builder

![About Enhe](docs/assets/about-author.png)

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
