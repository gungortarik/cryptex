# M8 Windows unmarked destination classification — 2026-10-29

## Scope

Prepared imports without a committed marker may have no published files, a
matching destination subset, all matching destinations, or conflicting/missing
files after interruption. This slice improves classification only. It does
not resume publication or authorize cleanup when any canonical destination is
visible.

## Implemented

- Recovery accepts only manifest-listed staged files or their exact canonical
  destinations; duplicate staged+destination copies fail closed.
- Present files are checked against manifest identity, length, and SHA-256.
- With no visible destination and intact staging, the existing prepared-abort
  path remains repeatable.
- Matching subset and all-destination states return distinct typed holds and
  preserve the prepared manifest, staging transaction, and canonical bytes.
- Documentation now distinguishes this diagnostic classification from the
  future policy-bound replay path.

## Verification

- `cargo test -p cryptexd import_staging::tests:: -- --test-threads=1`: **52
  passed, 0 failed**.
- `cargo clippy -p cryptexd --all-targets --release -- -D warnings`: **PASS**.
- `cargo fmt --all -- --check`: **PASS**.
- `git diff --check`: **PASS** (Git warns that the existing handoff file uses
  CRLF and may be normalized on a later write).

The regression first proves duplicate staged+destination copies hold, then moves
one of two prepared segments to canonical storage and proves subset
classification. It also proves a missing segment holds, moves the second and
proves all-destination classification, then restores the synthetic staging
arrangement and proves pre-publication abort. These are process-level tests,
not power-loss or Windows filesystem durability proof.

## Still blocked

ADR-0019 requires no-marker replay to rerun the full local trust/cutoff and
`RetrievalSafetyState` validation against the pre-import canonical snapshot
under locks. Startup currently runs import reconciliation before normal
projection/retrieval setup. A safe replay gate must account for transaction
segments already moved into canonical storage without treating the partial
stream as a valid current snapshot, and must confirm the existing projection
matches the corresponding pre-import state. Until that gate exists, partial or
complete unmarked destinations remain held. Production transport-to-publish
wiring, Windows marker durability proof, macOS parity, native UI review, and
independent security review remain open. M8 is not complete.

No Actions, commit, or push.
