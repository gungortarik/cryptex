# M8 Windows internal persistent import integration — 2026-11-08

## Scope and authority

The owner requested permanent-import integration after the native relay plus
VM full-policy validation probe passed. This slice implements the Windows
transaction orchestration permitted by ADR-0019 protocol addendum §5.1.1.
The Master Spec, accepted sync boundaries and single canonical writer remain
unchanged. Normal publication/transport activation is still gated by the
existing native durability and independent security review requirements.

The new function is crate-private, Windows-only and has no production CLI,
IPC, watcher or managed-transport caller. Its tests exercise real canonical
publication through the daemon, not only validation-and-discard. No existing
owner runtime, secure-store item, paired profile or downloaded native session
is modified by these tests.

## Transaction behavior

`services/cryptexd/src/incoming_publication.rs` owns an existing safe runtime's
instance lock through the complete operation. It reconciles prior transactions
before any canonical enumeration, stages explicit bounded incoming paths,
loads the existing OS-held local anchor (FakeSecureStore in tests), opens M3/M4
read-only and evaluates the existing complete locked retrieval/trust snapshot.
It does not enroll identities, insert roster rows or accept caller-supplied keys.

Replay classification uses that successful snapshot, bound to the same runtime
and all candidate device locks. Exact existing canonical bytes are compared to
the private staged copy through the existing no-follow, regular-file,
single-link, length and SHA-256 checks. Only redundant staged copies are removed.
The resource budget remains reserved for the original complete input batch.
An identical replay publishes nothing; a mixed batch verifies/publishes only
its novel contiguous tail; divergence and active/sealed conflicts reject.

Novel records pass existing staged signature/schema/Brain/identity/continuity
checks and complete post-import trust/cutoff/content reduction. The resulting
proof retains inventory/device locks across prepared-manifest persistence and
the existing no-replace publisher/committed marker. Existing canonical bytes
are never replaced. Incoming transport files remain intact for retry.

After a final commit, the operation drops the old snapshot, retains runtime
instance ownership, runs receipt-bound transaction cleanup, then rebuilds
disposable M3/M4 from independently verified local-approval canonical history.
The existing content gate still holds unsupported semantics. This slice adds
no event format, journal schema, cryptographic primitive or review bypass.

Typed outcomes distinguish:

- `Rejected`: current batch failed admission before preparation; private staging
  cleanup succeeded. Startup recovery of a prior transaction may already have
  run; this is not a statement that the entire runtime was never changed.
- `RecoveryRequired`: startup recovery, preparation, publication or cleanup
  could not safely finish. Preserve journal/staging evidence; do not claim
  rollback or delete partial canonical destinations.
- `CommittedNeedsRebuild`: canonical commit/cleanup succeeded but verified
  derived refresh failed. The canonical event exists and must not be written
  again as a new event. Stale retrieval state continues to hold.
- Success: published segment/event counts plus exact-replay segment count,
  after cleanup and derived refresh; replay-only success has zero new events.

No failure during/after manifest preparation invokes unprepared discard:
the final manifest may already have reached disk. Process interruption releases
locks; the existing startup recovery rebinds policy for partial/full unmarked
imports and preserves final-marker decisions. It does not silently refresh
derived watermarks after a crash; admission remains held until explicit rebuild.

## Verification

Executed on the Windows host:

```text
cargo test --locked -p cryptexd --lib incoming_publication::tests:: -- --test-threads=1
  8 passed (includes one subprocess worker entry)
cargo test --locked -p cryptexd --lib import_staging::tests:: -- --test-threads=1
  63 passed
cargo test --locked -p cryptexd --lib incoming_validation::tests:: -- --test-threads=1
  9 passed
cargo test --locked -p cryptexd --lib trusted_import_probe::tests:: -- --test-threads=1
  3 passed
cargo clippy --locked -p cryptexd --all-targets -- -D warnings
  passed
cargo fmt --all -- --check
  passed
```

New cases use genuinely signed mutual-approval fixtures and the real filesystem:

1. Publish a peer segment containing three signed records, reopen the context,
   verify guarded retrieval readiness, replay with zero additional events and
   byte-identical canonical/source/incoming data.
2. Mix an existing segment with a new tail in reverse input order: only one new
   segment/event is published and earlier canonical bytes remain unchanged.
3. Reject an unapproved signer, corrupted bytes, stale M4 watermark, divergent
   existing destination, or a valid-first/corrupt-second batch without any
   current-batch canonical publication.
4. Refuse an already-owned runtime before staging/publication. Reuse a matching
   caller-held instance without reacquisition; reject a foreign runtime's guard
   before staging and retain the caller's ownership through the return.
5. Inject a secure-store failure only after canonical visibility: return
   `CommittedNeedsRebuild`, preserve exact committed bytes, leave the stale
   retrieval hold in force and clean transaction metadata.
6. Exit the whole worker process at six boundaries: prepared temporary sync,
   prepared final sync, first/second destination move, committed temporary sync,
   committed final sync. Reconcile twice; abort pre-move cases or finish exact
   partial/full publication; retain incoming bytes; hold stale derived state
   until a verified rebuild; replay without duplicate canonical writes.

Subprocess test keys are passed privately over stdin, never process arguments
or evidence logs. No Windows/VM native-keychain, macOS, power-loss or independent
security-review result is implied by FakeSecureStore filesystem integration tests.

## Review and continuation

Review `incoming_publication.rs`, `incoming_publication_tests.rs`, the replay
pruning method in `import_staging.rs`, and the runtime/lock-bound safety access
in `knowledge_materialization.rs`. Challenge especially proof lifetimes,
replay/admission ordering, prepared-error evidence preservation, partial-move
recovery, and commit-versus-derived-error reporting.

The normal managed adapter does not yet call the service. Before exposing a
publication command or shipping a VM publication package, resolve the existing
activation review/platform gates and integrate only through daemon ownership.
The standalone wrapper takes instance ownership itself. The daemon-owned entry
point accepts and checks an existing guard without reacquisition. An eventual
long-lived controller must preserve startup recovery order and turn a
recovery-required result into a runtime-wide admission hold before readers
continue. That controller integration is still absent. Do not label this dormant service as production replication or
M8 completion. Do not redo the completed pairing/transport/validation probes.
