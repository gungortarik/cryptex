# M8 Windows final-marker cleanup slice — 2026-10-29

## Scope

Implemented the ADR-0019 §5.1.1 recovery row for a valid **final** committed
marker. This is transaction cleanup after commit, not publication or replay.
The production transport-to-publication path remains disabled.

## Behavior

- Recovery retains the import journal lock and acquires the canonical inventory
  and affected device locks.
- It rebinds the closed final marker and prepared manifest, checks the exact
  manifest digest, verifies the remaining staging tree, and hashes every
  destination against the prepared manifest.
- Cleanup runs only when every destination is present and matches. It removes
  this transaction's staging tree, prepared manifest, then final marker. It
  does not alter canonical event files.
- A missing/mismatched destination, bad digest, incomplete companion, marker
  temporary, unknown journal entry, or partial-publication state remains a
  typed fail-closed hold. Marker temporary completion is not implemented.

## Windows verification

- `cargo test -p cryptexd import_staging::tests:: -- --test-threads=1` —
  **49 passed, 0 failed**. Includes final-marker cleanup, preservation for
  absent/subset/mismatched destinations, marker-temp holds, and cross-process
  interruption after final-marker placement.
- `cargo test -p cryptexd --quiet -- --test-threads=1` — **507 library tests,
  12 binary tests, 9 lifecycle tests, 3 supervisor tests, and 2 rollback tests
  passed; 4 library tests ignored**.
- `cargo clippy -p cryptexd --all-targets --release -- -D warnings` — PASS.
- `cargo fmt --all` — PASS.

## Limits

These are Windows process-crash tests, not power-loss or filesystem durability
proof. The existing Windows directory-parent sync strategy remains unverified
as a platform durability guarantee. A crash partway through multi-entry
cleanup can leave a final marker with an incomplete companion; startup then
holds rather than guessing. Recoverable cleanup interruption and its journal
representation remain open. No native package interaction, live peer
transport, production publication, macOS parity, independent security review,
Actions, commit, push, or M8 completion is claimed.
