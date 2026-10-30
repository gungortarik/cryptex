# M8 Windows cutoff-aware content view foundation — 2026-10-29

## Implemented

`VerifiedSecurityHistory::content_records_through_cutoffs` selects authenticated
v1/v2 records only when the device has an effective cutoff and the record
sequence is at or below it. It excludes v3 security-policy records. The API is
explicitly a filtering helper: callers must derive cutoffs from the local trust
reducer and keep the lock-scoped verified history alive through content reads.

## Verification

- `cargo test -p cryptex-events --lib`: **80/80 passed**.
- `cargo test -p cryptex-events --doc`: **1/1 passed** (unforgeable verified
  bootstrap transcript compile-fail check).
- `cargo clippy -p cryptex-events --lib --tests --release -- -D warnings`:
  **PASS**.
- `cargo fmt --all -- --check`: **PASS**.
- `git diff --check`: **PASS**.

## Not established

The helper does not derive trust/cutoffs, produce a knowledge projection, or
feed `RetrievalSafetyState`. It does not authorize retrieval or import. The
Windows native package launch remains blocked by Code Integrity event 3077;
macOS parity, independent review, and M8 completion remain open. No Actions,
commit, push, or activation.
