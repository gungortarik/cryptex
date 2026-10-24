# M8 Windows staging and startup recovery slice 2

**Date:** 2026-10-24
**Branch:** `codex/m8-device-trust`
**Scope:** Bounded snapshot staging and pre-start recovery gate; no canonical publication.

## Implementation

- `services/cryptexd/src/import_staging.rs` copies `.seg` candidates from the
  fixed `transport/incoming` runtime subtree into a unique
  `canonical/import-staging/<transaction-id>` directory.
- Windows opens the local drive root, then walks every directory component
  relative to its already-open parent handle with `NtCreateFile` and
  `FILE_OPEN_REPARSE_POINT`; each opened component is checked for directory,
  reparse-point, and link-count attributes. The directory handles omit
  `FILE_SHARE_DELETE` while held. The incoming leaf is also opened relative to
  its parent handle, without following a reparse point, and denies write/delete
  sharing while copied. File attributes, link count, volume serial, and file ID
  are read through the opened handle. Unix uses `O_NOFOLLOW`, device/inode and
  link count.
- Source paths reject traversal, links and reparse points. Staging and
  `canonical/events` must be ordinary directories on the same volume.
- Copy is bounded by the M8 admission limits, SHA-256 is computed while
  streaming, staged bytes are read back and compared, the temp file is synced,
  and final placement uses no-replace semantics. Windows placement uses
  `MoveFileExW(MOVEFILE_WRITE_THROUGH)`; no Windows parent-directory flush claim
  is made.
- A cross-process single-flight lock enforces one staging transaction at a
  time. Failed staging rolls back its unprepared transaction; callers can
  explicitly discard a staging session.
- Daemon `hold`/`login-hold` now runs the startup recovery gate after acquiring
  the single-instance lock and before durable identity setup, managed transport,
  IPC, or readers. It deletes only recognized unprepared staging-only trees.
  Any prepared/unknown journal entry, link, malformed tree, or busy lock returns
  a typed `ImportRecoveryConflict` and prevents startup.

## Verification

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test -p cryptexd --lib import_staging::tests` | 7 passed, 0 failed |
| `cargo test -p cryptexd --lib` | 405 passed, 0 failed, 4 ignored |
| `cargo clippy -p cryptexd --all-targets -- -D warnings` | PASS |
| `cargo check -p cryptexd --bin cryptexd` | PASS |
| `cargo check -p cryptexd --target x86_64-pc-windows-msvc --lib` | PASS |
| `git diff --check` | PASS |

Windows-only tests confirm that the component walk rejects a junction and that
held component handles block an ancestor rename on this Windows filesystem.
These focused tests do not replace independent security review or full race and
fault-injection coverage.

## Remaining boundaries

The Windows adapter verifies current-user ownership and walks the local drive
directory chain one component at a time relative to held handles. It requests
each component itself so reparse points are inspected and rejected rather than
followed. Handle sharing excludes deletion while the chain is held. These
checks cover the imported source and the active staging paths; they do not
claim isolation from arbitrary same-user malware, which the approved M8 flow
does not require. macOS/Unix still use static path checks; no corresponding
native containment adapter or race verification is claimed.

Staging does not yet validate segment frames/signatures/trust/chain continuity,
persist prepared manifests or committed markers, reconcile a prepared
transaction, or publish into `canonical/events`. Unknown journal state is
intentionally a fail-closed startup conflict. Full reparse/race/fault
injection, packaged runtime checks, macOS parity, independent security review,
and M8 completion remain open. Trust and managed transport remain disabled.

No Actions, commit, or push.
