# M8 Import Publication Boundary Design and Review (2026-10-23)

## Result

**Candidate documented; independent security review returned NOT READY.** The
Master Spec requires the daemon to remain
the supported canonical writer and Syncthing to remain transport. It does not
specify OS sandboxing of a same-user Syncthing child. This note does not add
that stronger requirement.

## Source findings

- The managed Syncthing profile currently rejects active folders and peers;
  there is no Syncthing-to-canonical importer or incoming-event publication
  path (`services/cryptexd/src/managed_profile/native_xml.rs`).
- Canonical packs live under `canonical/events/<device-id>/` and the storage
  layer exposes immutable `.seg` files plus appendable `.open` files
  (`crates/storage/src/layout.rs`, `crates/storage/src/segment_io.rs`).
- Daemon readers/writers coordinate with inventory and per-device file locks
  (`services/cryptexd/src/canonical_writer.rs`).
- The Windows source-path/IPC audit found the same no-importer baseline
  (`evidence/m8/m8-windows-importer-and-ipc-source-audit-20261022.md`).

## Candidate publication flow

1. Configure a dedicated Syncthing folder rooted at a managed
   `transport/incoming` directory, outside `canonical/events`. The daemon
   accepts only the exact managed folder ID/path and refuses transport startup
   if the configuration points at canonical storage. Syncthing file arrival is
   never semantic acceptance.
2. Keep daemon-controlled `canonical/import-staging` and
   `canonical/import-journal` beside `canonical/events`, outside both the event
   tree and `transport/incoming`, on the destination volume. Verify path,
   ownership, and non-link directory handles before use.
3. Snapshot bounded, sealed segments using no-follow handles and bounded copies;
   metadata-then-path-copy is insufficient. After opening, require link count
   one and compare `(device,inode)` or volume file ID against entries under
   `canonical/events`; reject any alias. Verify size, identity, and modification
   state again after copying. Write each copy to a unique staging temporary,
   check and sync via handle, then atomically no-replace move to the staged
   final name; only complete files may enter a prepared manifest. Reject
   malformed names, incomplete frames, invalid signatures, identity/Brain
   mismatches, invalid sequence/hash chains, and trust/cutoff failures. Pin
   numeric max-files, max-file-bytes, max-batch-bytes, and concurrent prepared
   transaction limits before implementation. Staging bytes remain untrusted.
4. Persist a transaction manifest with a closed, versioned schema: fresh
   transaction ID, affected DeviceIds, canonical relative destinations, segment
   indexes, byte lengths, and SHA-256 digests. Sync staged files and the
   prepared manifest before publication. Recovery recomputes every staged and
   already-published digest rather than trusting manifest values.
5. Acquire the inventory lock, then every affected device writer lock in sorted
   DeviceId order. Require segment indexes contiguous with the verified sealed
   stream tip; reject `.open` files, gaps, divergent duplicate indexes, and
   unreconciled `.open`/`.seg` pairs. Revalidate inventory, chains, trust,
   cutoffs, and destinations under lock. Every canonical mutator (local append,
   restriction append, segment sealing, and import) must use one reviewed
   shared lock API in this order; importer locks alone do not serialize current
   appenders. Publish additive immutable
   `.seg` files with same-volume no-replace moves. Byte-identical destinations
   are idempotent; divergence is a typed fail-closed conflict.
6. Sync each published file and parent where the platform supports a verified
   durable operation. Use a closed-schema committed marker with an atomic,
   platform-specific durable-write procedure before releasing canonical locks.
   Do not claim Windows parent-directory durability while `sync_directory` is a
   no-op; implement and verify an equivalent strategy first. Test crashes after
   every move and around marker replacement. Derived indexes remain rebuildable.
7. Acquire the single-instance lock, reconcile the import journal under
   canonical locks, and only then start transport, inspect incoming files,
   prepare or publish another import, start IPC, projection/materialization,
   retrieval, or canonical enumeration. Complete a prepared transaction only
   when every staged/published digest and full validation succeeds. Otherwise
   hold affected streams and report a typed recovery conflict. No verified
   canonical reader may run before recovery; process locks do not provide
   multi-file crash atomicity.
8. Test config/path refusal, bounded parsing, duplicate-identical and divergent
   destinations, local append/import interleavings, every publication crash
   point, and startup recovery on both Windows and macOS.

The import journal uses unique transaction IDs, immutable prepared manifests,
and separate no-replace committed markers. Its single-flight mutex reserves
transaction ownership only: release it before transport or canonical-lock waits
and never acquire inventory/device locks while holding it. After canonical
locks are acquired, write the committed marker without reacquiring the import
mutex, then release canonical locks. This is separate from the pending-journal
mutex rule.

## Gate / next action

Three independent Cursor reviews returned **NOT READY**. They confirmed
critical code gates: existing appenders lack the shared inventory lock; startup
has no import-journal recovery gate; and owner-approval IPC lacks connection-
bound identity and one-time sessions. High implementation gates include
staging, segment continuity, Windows durability, v3 security-state integration,
and macOS peer binding. Further review gaps were incorporated into the latest
candidate: post-open link/file-identity checks, fixed staging/journal roots,
atomic staging finalization, explicit startup ordering, and import-mutex scope.
The third review still found the same code gates and requested stronger
post-open alias wording; this has since been added above. A fresh review is
required before owner acceptance.

This is a candidate design, not an accepted protocol or implementation. Keep
trust activation disabled. Obtain independent security review of the staging,
lock, conflict, and crash-recovery contract; resolve findings and obtain owner
acceptance before implementing trust-changing publication. Then implement the
complete Windows slice (importer, journal, startup gate, and adversarial tests),
followed by the consolidated macOS slice.

This is static source/design and independent-review follow-up only. No code,
runtime state, tests, Actions, trust state, or user data changed. Full findings
and source locations are in Cursor chat “Independent security review
findings”; prompt: `evidence/m8/m8-independent-security-review-prompt-20261023.md`.
