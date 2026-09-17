# Changelog

## [Unreleased]

No unreleased changes.

## [0.1.2] - 2026-09-17

- Normalized verbatim Windows paths at the configuration, picker, and display boundary while retaining safe internal canonical paths.
- Kept automatic fixed-disk discovery, aggregated inaccessible-directory errors, and added an explicit Windows administrator rescan request.
- Added a bilingual operation-guide flowchart for first-run scanning, project selection, local backup, restore, and offline migration.
- Added the `v0.1.2` version label above the in-app product name and to the desktop window title.
- Updated the Windows x64 installer and usage documentation for the new discovery and guide flows.

## [0.1.1] - 2026-09-17

- Renamed the bilingual home-page product title to “Codex 数据备份&迁移” / “Codex Data Backup & Migration”.
- Updated the application version metadata and published Windows x64 installer links for this release.

## [0.1.0] - 2026-09-17

- Added Windows x64 local-first backup, restore, and offline migration flows.
- Added Git/worktree-aware backup staging, Codex data discovery, and rollback-safe restore handling.
- Added bilingual UI, theme selection, native directory pickers, bounded local project discovery, and startup diagnostics.
- Added an opt-in OneDrive/rclone configuration path while keeping cloud off by default.
- Added synthetic tests, isolated restic acceptance, documentation, and an unsigned NSIS installer.

Live OneDrive, second-device, clean-profile, and original-session continuation checks remain outside the verified evidence boundary.
