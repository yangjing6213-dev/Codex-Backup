# Security

Do not publish a personal `.rehome` migration package. It can contain private conversation history, project files, local paths, and generated artifacts.

ENHE Codex Backup excludes authentication data, cookies, `.env` files, private keys, dependency directories, virtual environments, and runtime files by default. Complete local backups intentionally preserve selected `.git` and worktree data. If you discover an unintended inclusion path or security issue, report it privately to the repository owner. Do not attach a personal package to a public issue.
