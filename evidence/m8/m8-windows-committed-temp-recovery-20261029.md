# M8 Windows committed-marker temporary recovery — 2026-10-29

## Scope

Implemented the ADR-0019 §5.1.1 recovery row for a recognized committed-marker
temporary. This completes an already-published transaction only; it does not
publish canonical files or re-admit trust.

## Recovery behavior

- Revalidates the closed marker and exact prepared-manifest digest, then holds
  the canonical inventory and affected device locks while checking the staging
  subset and hashing every destination.
- If every destination matches, moves the marker temporary to its final name
  with no-replace semantics, syncs the journal parent, and reads back the
  final marker before cleaning that transaction's staging, manifest, and
  marker.
- If any destination is absent or mismatched, the companion is invalid, a
  final marker already exists, or journal state is ambiguous, recovery holds
  and preserves the available evidence. It never repairs or alters canonical
  files.

## Windows verification

- `cargo test -p cryptexd import_staging::tests:: -- --test-threads=1` —
  **52 passed, 0 failed**.
- Added cross-process interruption coverage after the temp-to-final move and
  after final-marker parent sync/read-back. In both cases, a subsequent startup
  completes cleanup while preserving canonical destination bytes.
- Final-marker cleanup, temp-marker refusal with absent destinations, partial
  destination holds, marker/manifest replacement checks, and preexisting
  publication interruption cases pass in the same focused suite.
- `cargo fmt --all` completed. The full daemon suite and strict Clippy have
  not yet been rerun after this slice.

## Limits

These are process-interruption tests, not power-loss or Windows filesystem
durability proof. The Windows parent-directory sync strategy remains an open
platform gate. An interruption during multi-entry cleanup can leave an
incomplete companion; startup holds rather than inferring missing state.
Partial-publication replay, production transport-to-publication wiring,
macOS parity, independent security review, Actions, commit, push, and M8
completion are not claimed.
