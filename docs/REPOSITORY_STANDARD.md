# Repository standard

## Scope

ENHE Codex Backup is a Windows 11 x64 Tauri 2 desktop application. The local
backup path is the product baseline; cloud is disabled by default and is not a
dependency of local backup, restore, or discovery.

## Layout and tools

- `desktop/`: React 19, TypeScript, Vite, and Tauri UI.
- `desktop/src-tauri/`: Rust commands and backup, discovery, restore, and migration core.
- `docs/`: specifications, guides, compatibility, acceptance, and evidence.
- `scripts/`: bootstrap, verification, local acceptance, and packaging.
- `tests/`: isolated test harnesses and documentation contracts.
- Package manager: pnpm, using `desktop/pnpm-lock.yaml`.
- Rust dependencies: `desktop/src-tauri/Cargo.lock`.

## Required checks

Run the frontend tests/build, `scripts/verify.ps1`, and the Windows x64 package
script when changing application behavior or release inputs. Keep ignored
environment reports local unless a release note explicitly includes a stable
evidence document.

## Security and release

Do not commit personal data or secrets. Complete backup intentionally retains
Git and worktree data, so source selection and restore targets require user
review. Use ordinary Git history operations only; never force-push. Public
release status must distinguish implementation from checks that are
`NOT_RUN` or `NOT_VERIFIED`.
