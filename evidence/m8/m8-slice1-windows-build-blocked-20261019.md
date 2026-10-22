# M8 Slice 1 Windows candidate and verification status

**Date:** 2026-10-19  
**Base:** `74bc3b14127962579ae03838bafea9336612a033`  
**Branch:** `codex/m8-device-trust` (uncommitted candidate)

## Changes

- New read-only `replication_status` module and `replication status` daemon CLI.
- Existing local-device metadata validation is reusable within the daemon
  crate; its validation rules and serialization are unchanged.
- Added 12 focused library tests and 3 CLI tests for missing runtime/no writes,
  truthful unconfigured status, declaration counts, redacted corruption,
  unsupported/mismatched identity, bounded inspection, duplicate/misnamed
  records, case-distinct IDs, and explicit target/argument rejection.

## Local verification

`cargo fmt --all` applied formatting.

The attempted command:

```text
cargo test --locked -p cryptexd --lib replication_status::
```

used `CARGO_TARGET_DIR=%LOCALAPPDATA%/Cryptex-Dev/build/m8-device-trust-target`.
Windows Application Control rejected generated dependency build scripts for
`quote 1.0.47`, `zmij 1.0.23`, and `curve25519-dalek 4.1.3` with OS error 4551.
The test binary was not reached. No test, compile, Clippy, or runtime PASS is
claimed; this is a host build-policy blocker, not an observed test failure.
No security setting was changed or bypass attempted. Partial build artifacts
are outside the repository. No daemon or Syncthing process was launched.

## Required next verification on Mac

Use a new writable isolated M8 worktree at the exact base and apply the verified
source patch. Preserve the dirty original Mac checkout and designated read-only
M7 verification worktree. Run formatting check, focused library/CLI tests,
strict daemon Clippy and a daemon-only build; then smoke-test the status command
against a fresh nonexistent synthetic runtime path and confirm no directory is
created. Transfer instructions accompany the source package.

No app bundle/GUI build, personal runtime, key enrollment, real pairing, Actions,
commit, push, or merge is needed. M8 is not complete; M7 write-back stays off.
