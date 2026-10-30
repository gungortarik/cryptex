# M8 Windows prepared-recovery policy verifier — 2026-10-29

## Scope

Implement the validation half of the ADR-0019 no-marker partial-publication
recovery gate. The work does not make startup resume publication or clean up a
prepared transaction.

## Implemented

- Added an import-recovery scan that excludes only exact manifest-bound
  canonical tail segments whose length and SHA-256 match. Excluded files are
  opened through a checked directory-handle chain, rejected unless they are
  ordinary single-link files, and checked for identity stability while read.
- The scan preserves the pre-import canonical stream and M3/M4 projection
  comparison while the manifest's full segment set is being checked.
- Added a prepared-batch verifier that selects exactly one matching staged or
  canonical file for every manifest segment, checks staged file-ID aliases,
  and verifies the frame digest, event signature/schema, Brain and device
  binding, event sequence, predecessor hash, local trust, effective cutoff,
  and segment-index continuity against the current lock-bearing snapshot.
- The resulting private `ImportPolicyVerifiedBatch` retains the inventory and
  device locks. It is validation evidence only: no canonical move, committed
  marker write, cleanup, or transport integration is enabled.
- Any batch containing an authenticated v3 trust-control record is rejected
  until the post-import security-history reducer can recompute trust/cutoffs;
  the current snapshot alone cannot authorize such a transition.
- Added `ProjectionDb::open_existing_read_only`: it requires an existing
  ordinary DB file, opens SQLite read-only, runs `quick_check`, validates the
  persisted schema without write-probing constraints or creating FTS temp
  tables, and does not create, migrate, or repair projection state.
- The retrieval safety gate now uses the same read-only schema validator. The
  prior writable FTS capability probe attempted a temporary write and caused
  valid read-only startup recovery checks to be held as stale materialization.
- Windows startup now invokes the verifier only for a no-marker prepared
  transaction with one or more visible canonical destinations. It lazily loads
  the existing Brain identity and enrolled local key, opens the existing
  projection read-only, and rechecks under its own lock-bearing policy
  snapshot. Missing state fails closed. Even after a successful verification,
  startup returns a typed hold and preserves the manifest and canonical bytes;
  no publication, marker completion, or cleanup is authorized.

## Verification

- `cargo test -p cryptexd import_staging::tests::prepared_recovery_batch_verifies_existing_destination_against_preimport_policy -- --exact`: **PASS**.
- `cargo test -p cryptexd --lib --bins`: **513 library passed, 12 binary passed, 4 ignored**.
- Full `cargo test -p cryptexd --bins --tests`: library/binary tests passed, but all 9 lifecycle integration tests were blocked when Windows Application Control refused to execute the generated daemon fixture (OS error 4551). This is an environment execution block, not a test assertion failure.
- `cargo test -p cryptex-projection`: **not re-run successfully**; Windows Application Control refused to execute its test binary (OS error 4551). Prior passing result predates the current read-only gate adjustment.
- `cargo clippy -p cryptexd --all-targets --release -- -D warnings`: **PASS**.
- `cargo clippy -p cryptex-projection --all-targets --release -- -D warnings`: **PASS**.
- `cargo fmt --all --check`: **PASS**.
- `git diff --check`: **PASS** (existing handoff is CRLF; Git reports its normal line-ending warning).

The recovery regression uses only a synthetic local identity, event stream,
and persistent disposable projection reopened read-only. It confirms the
read-only policy path verifies a matching existing destination, then continues
to hold the transaction and preserve its manifest and canonical bytes. These are host-level process tests,
not power-loss, supported-filesystem durability, native packaged, transport,
or macOS parity proof. The verifier checks cutoff against the pre-import trust
snapshot and explicitly rejects v3 security effects introduced by the batch
until combined-history reduction exists.

## Remaining gates

- Reduce imported v3 security effects against the post-import trust snapshot
  before any replay path can proceed; the current startup gate deliberately
  rejects such batches.
- Add no-replace recovery publication only after marker durability gates and
  interruption coverage are satisfied. Startup currently validates and holds.
- Complete Windows native durability and packaged review, transport-to-staging
  wiring, macOS parity, and independent security review.

No Actions, commit, or push. M8 is not complete.
