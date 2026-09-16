# Third-party components

| Component | Version/source | Use | License/notice |
| --- | --- | --- | --- |
| ReHome core | Git commit `cc5daae501b22b86e1b4c2dafbd1b4af61992b48` | `.rehome` migration and transaction core | Upstream MIT; see root `LICENSE`. |
| restic | 0.19.1 Windows amd64 | Encrypted, deduplicated local repository | BSD-2-Clause; [upstream license](https://github.com/restic/restic/blob/v0.19.1/LICENSE). Archive hash is recorded in `docs/verification/third-party-sha256.txt`. The executable is downloaded by `scripts/bootstrap.ps1` and is not committed. |
| rclone | 1.75.1 Windows amd64 | Explicit OneDrive configuration and remote restic backend | MIT; [upstream license](https://github.com/rclone/rclone/blob/v1.75.1/COPYING). Archive hash is recorded in `docs/verification/third-party-sha256.txt`. The executable is downloaded by `scripts/bootstrap.ps1` and is not committed. |
| Tauri / React / Rust crates | Locked by `desktop/src-tauri/Cargo.lock` and `desktop/pnpm-lock.yaml` | Desktop shell and UI | Each package's license metadata remains authoritative. |

No third-party credential, token, or user data is included in this repository. Release packaging must preserve the corresponding upstream notices when redistributing binaries.
