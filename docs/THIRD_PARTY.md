# Third-party components

| Component | Version/source | Use | License/notice |
| --- | --- | --- | --- |
| ReHome core | Git commit `cc5daae501b22b86e1b4c2dafbd1b4af61992b48` | `.rehome` migration and transaction core | Upstream MIT; see root `LICENSE`. |
| restic | 0.19.1 Windows amd64 | Encrypted, deduplicated local repository | BSD-2-Clause; [upstream license](https://github.com/restic/restic/blob/v0.19.1/LICENSE). Archive hash is recorded in `docs/verification/third-party-sha256.txt`. The executable is downloaded by `scripts/bootstrap.ps1` and is not committed. |
| rclone | 1.75.1 Windows amd64 | Explicit OneDrive configuration and remote restic backend | rclone's upstream COPYING is MIT; the linked pinned `github.com/cloudsoda/sddl` revision is distributed under the conservative LGPL-3.0 path because its exact LICENSE and upstream repository classification say LGPL-3.0. Its README's MIT sentence remains recorded as a documentation discrepancy. The installer carries exact corresponding source, relinking instructions and build metadata under `resources/licenses/`. |
| Tauri / React / Rust crates | Locked by `desktop/src-tauri/Cargo.lock` and `desktop/pnpm-lock.yaml` | Desktop shell and UI | Each package's license metadata remains authoritative. |

No third-party credential, token, or user data is included in this repository. Release packaging must preserve the corresponding upstream notices when redistributing binaries.

## Offline notices in the Windows installer

The installer includes [the notice directory](../desktop/src-tauri/resources/licenses/THIRD-PARTY-NOTICES.txt) under the installed application's `resources/licenses` directory:

- `LICENSE.txt`: the original program/ReHome MIT text and attribution.
- `THIRD-PARTY-NOTICES.txt`: reading guide, NSIS installer terms/source location, and Microsoft.Web.WebView2 SDK 1.0.3650.58 LICENSE/NOTICE for the linked static Loader. The Loader matches the official SDK package byte-for-byte; this is separate from the externally installed WebView2 runtime.
- `JS-THIRD-PARTY-NOTICES.txt`, `RUST-THIRD-PARTY-NOTICES.txt`, and `GO-THIRD-PARTY-NOTICES.txt`: versioned dependency inventories and original license/notice texts, including nested notices where supplied.
- `manifest.json`: reviewed notice SHA-256 values and dependency/runtime input fingerprints.

These are local readable files, not merely links to online license pages. Original upstream attributions remain intact. Some dependency inventory entries are a conservative build-time/platform superset; the individual inventory states its scope.

`tests/bundled_licenses_test.ps1` runs in both `scripts/verify.ps1` and `scripts/package.ps1`. It rejects missing notices, changed dependency inputs, altered notice files, or notices absent from Tauri resources. Packaging also checks the generated NSIS file-inclusion instructions. Text input fingerprints normalize CRLF to LF for portable Git checkouts; notice payload fingerprints are raw bytes, preserved by the directory's Git attributes.

When dependency lockfiles or either pinned runtime binary changes, review and refresh the notice texts and fingerprints before packaging. Do not update fingerprints alone to silence a failed license check. This inventory is technical redistribution evidence, not a blanket legal certification or a replacement for applicable upstream terms.

## 0.1.8 LGPL source and relinking material

The pinned rclone binary includes `github.com/cloudsoda/sddl v0.0.0-20250224235906-926454e91efc`. At that exact revision, its [LICENSE](https://github.com/cloudsoda/sddl/blob/926454e91efc/LICENSE) contains LGPL-3.0, while its [README](https://github.com/cloudsoda/sddl/blob/926454e91efc/README.md#license) identifies MIT. The upstream repository is classified as LGPL-3.0, so this package selects the LGPL-3.0 distribution path and retains the README sentence as a disclosed documentation discrepancy.

The installer includes `RCLONE-LGPL-SOURCE-BUNDLE.zip`, which contains the
exact rclone application source at the linked v1.75.1 commit and the exact
SDDL source at the linked revision. `RCLONE-LGPL-SOURCE-AND-RELINKING.txt`
and `RCLONE-BUILD-MANIFEST.json` record the source hashes, observed build
inputs and a relinking command. This is the conservative LGPL distribution
arrangement; it does not rely on an MIT-only interpretation or promise
byte-for-byte reproducibility.

The 0.1.8 rebuild does not upgrade, remove or change rclone, and does not
change application behavior.
