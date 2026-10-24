# M8 Windows lock-set implementation slice 1

**Date:** 2026-10-23
**Scope:** Shared canonical inventory/device writer locking
**Source baseline:** `2a5c847c121792779c053c7be8f5e04e723a87e1`
**Platform:** Windows x86_64-pc-windows-msvc

## Change

`services/cryptexd/src/canonical_writer.rs` now uses a shared canonical
write-lock set for local append, seal, and recovery operations:

- Inventory lock is acquired before any device writer lock and retained for
  the duration of the canonical operation.
- Multi-device sets sort case-sensitive ASCII `DeviceId` values by bytes and
  reject duplicates before touching the filesystem.
- Acquired device locks are released in reverse order before inventory release.
- Existing-device writers now participate in inventory serialization, as do
  first-time device creation and full-inventory readers.

Tests cover existing-device serialization, inventory retention through the
operation, sorted multi-device acquisition, duplicate rejection without path
creation, and verified-reader exclusion while a writer has an incomplete tail.

## Verification

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test -p cryptexd --lib` | 393 passed, 0 failed, 4 ignored |
| `cargo clippy -p cryptexd --all-targets -- -D warnings` | PASS |
| `cargo test -p cryptexd` binary target | Not executed: Windows Application Control blocked `cryptexd-*.exe` with OS error 4551 after the library tests passed |

## Boundary

This is a Windows-host code/test result for the shared Rust lock layer. It does
not implement the Syncthing incoming importer, staged-copy validation,
transaction journal, startup recovery gate, Windows directory durability,
approval IPC, or `security.*` retrieval-state integration. It is not macOS
verification, an independent security re-review, or M8 completion. Trust and
transport activation remain disabled.

No GitHub Actions, commit, or push was run.
