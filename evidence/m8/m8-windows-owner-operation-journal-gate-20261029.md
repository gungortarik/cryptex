# M8 Windows owner-operation journal gate

> Follow-up on 2026-10-29: the owner accepted the §2 bootstrap model. The
> operation-payload binding below has since been implemented; the original
> gate description is retained as historical context. This follow-up does not
> enable canonical append or M8 trust activation.

## Implemented foundation

- Added issuer-only `owner_pending` and `owner_approved` journal states for
  non-admission security operations.
- Persisted an immutable, digest-checked snapshot before a caller can issue an
  approval capability; confirmation only advances when the supplied digest
  matches that stored snapshot.
- Refused duplicate transaction rebinding and malformed issuer/operation
  state.
- Kept canonical append closed for non-admission operations. They have no
  operation-specific frozen payload in the journal, so they cannot enter
  `append_unknown` or `anchored`.
- At the time of this report, device restriction was refused because the
  approval snapshot omitted its `retired`/`compromised` reason; self-retirement
  lacked the exact last-authorized sequence/hash. The follow-up below closes
  the snapshot/digest gap but does not enable append.

## Verification

- Focused owner-operation journal regression: **1/1 passed**.
- `cargo test -p cryptexd --lib`: **507 passed, 0 failed, 4 ignored** after
  the final fail-closed assertion adjustment.
- `cargo clippy -p cryptexd --lib -- -D warnings`: **PASS**.
- `cargo fmt --all --check` and `git diff --check`: **PASS**.

## Required owner decision

To implement the owner-confirmed operations without hidden choices, amend the
ADR approval request shape/digest to bind `restriction_reason` and the
self-retirement `last_authorized_seq` plus `last_authorized_content_hash`.
The journal must store that exact operation payload before sequence reservation
and use it for crash reconciliation. The approval snapshot/digest amendment is
now in the candidate contract and accepted Windows implementation basis. M8
remains open.

## Owner-approved snapshot-binding follow-up

- `ApprovalSnapshot` now has closed operation-specific fields for restriction
  reason and self-retirement boundary; shape validation rejects missing,
  extra, or malformed fields.
- The canonical approval digest covers those fields, and the bounded journal
  persists the exact snapshot before owner confirmation. Transaction retries
  remain digest-pinned.
- Regression coverage confirms a changed restriction reason or retirement
  boundary changes the digest, valid restriction snapshots persist, and the
  non-admission operation still cannot enter append/anchor states.
- Canonical history revalidation, security-event construction and append,
  crash reconciliation, native Windows interaction, and macOS parity remain
  open. No real trust operation was run.
- Final verification after the follow-up: `cargo test -p cryptexd --lib --bins`
  passed (**514 library tests, 4 ignored; 12 binary tests**), strict release
  Clippy passed, and format/diff checks passed. An initial full-test launch
  was blocked by Windows Application Control (OS 4551); a later full run
  succeeded without changing security settings.
