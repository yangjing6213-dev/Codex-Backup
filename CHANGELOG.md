# Changelog

## [Unreleased]

## [0.1.6] - 2026-09-20

- Reject a non-empty ordinary folder before restic runs unless it has a complete restic repository layout, leaving every existing file unchanged.
- Classify repository, recovery-password, and disk-space failures from sanitized restic diagnostics without exposing the temporary password-file path.
- Show bilingual backup and restore errors with a summary, cause, solution, and technical detail; correct the restore-failure wording.
- Approve the pinned `esbuild` install script in the pnpm workspace policy so canonical frontend checks and packaging can run non-interactively.
- Keep package metadata, the window title, visible version labels, documentation, and the unsigned Windows installer aligned at version 0.1.6.

## [0.1.5] - 2026-09-20

- Show current project discoveries by drive, network location, or other location while hiding unselected nested paths remembered only by historical Codex sessions.
- Keep startup scanning and recursive all-file counts while retaining manually added, selected, and temporarily unavailable project paths for explicit review.
- Add a responsive bilingual About the Author page with the supplied portrait and contact details, and restrict external-link access to the three published HTTPS destinations.
- Refresh existing Windows desktop shortcuts by inheriting the installed executable icon, with a native NSIS regression fixture.
- Keep package metadata, the window title, visible version labels, documentation, and the unsigned Windows installer aligned at version 0.1.5.

## [0.1.4] - 2026-09-20

- Refresh an existing Windows desktop shortcut during installation so it uses the bundled ENHE blue application icon instead of a stale icon.
- Preserve the user's no-shortcut choice by updating the desktop shortcut only when one already exists.
- Keep the application package, window title, visible version label, and documentation aligned at version 0.1.4.

## [0.1.3] - 2026-09-20

- Scan configured collection directories as direct child folders with their actual names, without promoting nested build/test directories into projects.
- Count all regular project files recursively in background workers; show incomplete counts and separate historical/manual paths from current results.
- Include project dependencies, build outputs, hidden files and explicitly authorized `.env`, private keys and tokens in encrypted local backups; retain separate Codex credential exclusions and legacy ReHome export rules.
- Replace the green theme with an accessible blue palette for light/dark/system modes, and use an ENHE-inspired blue application/window/installer icon.
- Correct the selected Codex folder argument in native discovery, serialize pending file counts, prevent stale startup discovery from overwriting a newer scan, and preserve existing settings when a rescan is requested before configuration loading finishes.
- Preserve the restore manifest for source mapping and show missing-file details without counting internal metadata as project files.

- Kept backup/restore work off the UI thread and preserved running tasks and results across page navigation.
- Added return navigation and themed layouts to ReHome export/import; local backup is the default operation view.
- Fixed deselection of unavailable saved projects, normalized Windows path aliases, and persisted explicit empty selections.
- Added startup scan settings and folder scope; rescanning replaces discoveries while preserving saved/manual choices.
- Excluded historical migration archives, Codex/plugin caches and the configured repository from project discovery only.
- Added repository explanations and bilingual restore/device-transfer walkthroughs; unknown file counts now use background enumeration with explicit pending/error states.
- Corrected refreshed backup file counts to exclude directories and the internal manifest while retaining unchanged files.
- Added guided backup actions to the overview for project settings, data settings, and starting a local backup.
- Added a dedicated Data page for Codex data backup configuration and clearer navigation between setup and backup tasks.
- Improved the missing Codex data guidance so users are directed to the Data page.

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
