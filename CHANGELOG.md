# Changelog

## [Unreleased]

## [0.1.8] - 2026-09-25

- Add explicit project-root scanning guidance and a native folder picker so `F:\Projects` can be scanned as a complete direct-child project collection instead of relying on bounded whole-drive discovery.
- Show current scan results separately from retained/manual folders, expose a select-all-current-scan action, and surface when whole-drive discovery may be incomplete.
- Preserve recursive all-file counting and full local backup inclusion semantics, including hidden files, dependencies, build outputs, `.env`, private keys and tokens; this release only improves discovery clarity and selection control.
- Align application metadata, visible labels, bundled license headers, bilingual documentation and unsigned Windows x64 packaging at 0.1.8.

## [0.1.7] - 2026-09-23

- Separate automatic/security exclusions, rebuildable dependency/runtime notices, integrity warnings and genuinely missing files. Keep missing project roots visible with remedies instead of treating every skipped cache or dependency link as lost source.
- Validate copied Codex JSONL without the former per-line 8 MiB false-warning limit; retain malformed files and warn about the affected conversation.
- Add explicit file-only and migrate-and-connect ReHome import modes. The latter verifies restored files, App Server recognition of all planned conversations, and one user-selected ephemeral-fork probe, without messaging the original thread.
- Require closed-Codex confirmation and separate online consent for context transmission and possible model usage. Reject unavailable capabilities or unconfirmed safety settings, including enabled integrations; never silently downgrade verification.
- Keep migration jobs running across navigation and expose progress, transaction history, causes and remedies. Roll back ordinary failures after confirmed helper termination; preserve recovery material and stop database writes when termination cannot be confirmed.
- Document that recognition is not pixel-level desktop visibility, one probe is not full-history/tool validation, and local rollback cannot undo online processing or usage. Real-account, second-device and native UI acceptance remain NOT_RUN.
- Align metadata, visible labels, bilingual guides and unsigned Windows x64 packaging at 0.1.7. Older releases remain unchanged.
- Include offline program and third-party license/notice texts in the installer, and reject missing or stale notice inventories before packaging. This packaging correction does not change application behavior.
- Treat the pinned `github.com/cloudsoda/sddl` revision conservatively under LGPL-3.0: include the exact rclone/SDDL source bundle, relinking instructions and build manifest in the installer. This release-preparation change does not change application behavior or dependency versions.
- Fix the Windows CI restore preflight regression test so it checks the real application data directory instead of mistaking the `LOCALAPPDATA` parent directory for an application write.

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
