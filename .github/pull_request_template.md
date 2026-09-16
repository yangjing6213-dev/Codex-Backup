## Summary

<!-- What changed and why? -->

## Verification

- [ ] Frontend tests/build run, or the reason is documented.
- [ ] Rust formatting/tests run when Rust changed.
- [ ] Documentation and compatibility notes updated when behavior changed.
- [ ] Real `.codex`, cloud accounts, credentials, and personal migration packages were not used.

## Safety

- [ ] Cloud remains off by default and local backup remains independent.
- [ ] Restore conflicts and existing data are not silently overwritten.
- [ ] No secrets, personal data, or generated build output is included.
