# Repository guidance

This repository contains the ENHE Codex Backup Windows desktop application.
Keep changes limited to this project; do not use real Codex profiles, cloud
accounts, credentials, or personal migration packages in tests.

## Canonical checks

- `pnpm --dir desktop test -- --run`
- `pnpm --dir desktop run build`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\package.ps1`

Rust tests use the project-local toolchain when available. Use synthetic data
and temporary directories. Cloud is opt-in and must remain off in local tests.
Never commit `.env` files, tokens, passwords, personal `.codex` data, build
outputs, or local audit reports. Do not force-push or rewrite shared history.
