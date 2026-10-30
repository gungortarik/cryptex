# M8 Windows bootstrap journal foundation — 2026-10-29

## Scope

Owner accepted ADR-0019 §§2–4 as the Windows implementation basis. This change
implements only the bounded pending-journal model/store foundation for M8-R3.
It does not enable trust, transport, canonical publication, or user-facing
approval.

## Implemented

- Closed-schema journal rows for the immutable owner-approval snapshot,
  digest, role/state, public signed transcript bytes, stable event ID, and
  append-attempt metadata.
- Recomputed snapshot digest and strict identifiers, duplicate transaction and
  stable-event rejection, transaction/byte/attempt caps, and explicit state
  transition validation.
- Complete bootstrap transcript verification bound to Brain/transaction,
  participant keys/fingerprints, role-specific transport IDs, agreement hash,
  and the exact preceding approval-message hash.
- Cross-process locked read/modify/write API; unique same-directory temporary
  file with exclusive create, complete write and sync, atomic replacement,
  destination reopen/sync, and Windows `MoveFileExW` replace/write-through.
- Duplicate JSON key rejection and fail-closed preservation of unexpected
  entries or leftover temporary files. Journal data remains untrusted cache
  and never grants trust.

## Verification on Windows

- `cargo test -p cryptex-events --lib bootstrap::tests`: **4/4 PASS**
- `cargo test -p cryptexd --lib bootstrap_journal::tests`: **9/9 PASS**
- `cargo test -p cryptex-events --lib`: **78/78 PASS**
- `cargo test -p cryptexd --lib`: **467 PASS / 0 FAIL / 4 ignored**
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings`: **PASS**
- `cargo fmt --all --check`: **PASS**
- `git diff --check`: **PASS**

The tests cover digest binding, state-transition rejection, ID uniqueness,
duplicate JSON keys, round-trip persistence, rejected update preservation,
concurrent serialized updates, and fail-closed preservation of an interrupted
temporary file. They are not process-kill or power-loss fault-injection tests.

## Still open

- Production IPC does not create or consume these journal transactions.
- No authenticated one-use owner approval session, confirmation/cancel race
  serialization, OS-key lookup for the journal flow, signature generation or
  release, or startup reconciliation against canonical history exists.
- No canonical event append/idempotency wiring or transport-to-bootstrap path
  exists. The journal cannot authorize trust or publication.
- Native interruption/durability evidence and macOS implementation/parity are
  still required. M8 is not complete.
