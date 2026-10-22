# M8 protocol addendum — scoped design review

**Date:** 2026-10-22\
**Reviewed candidate:** `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md`\
**Disposition:** **NOT ACCEPTED / NOT READY FOR TRUST-ACTIVATING IMPLEMENTATION**\
**Scope:** Static consistency and security-design review against the Master
Spec, accepted ADR-0015 through ADR-0019 product decisions, current event reader,
and M8 lifecycle constraints. This is not an independent cryptographic review,
implementation review, or runtime verification.

## Summary

The candidate preserves the selected product direction: bilateral owner
approval, no transitive grants, local daemon authorship, immutable security
records, sequence cutoffs, and fail-closed holds. No direct conflict with the
accepted product permissions was found in those choices. The proposal correctly
keeps trust changes, schema activation, and transport protection disabled.

The candidate is not ready for acceptance. Several security-critical details
are intentionally described as gates but still need exact normative rules and
test vectors before the protocol can be implemented. No source code or protocol
state was changed in this review.

## Required closure findings

### M8-R1 — Bind each UI approval to one immutable transaction digest

**Severity:** High\
The proposal says a stale confirmation must not approve replacement data and
that each owner confirms an immutable candidate. It does not yet define the
authenticated Desktop-to-daemon request/receipt binding that enforces this.

Before acceptance, specify a single digest over the exact displayed Brain,
DeviceId, full event-key fingerprint, transport association, transaction ID,
roles, and operation. The daemon must retain the exact pending transcript,
accept confirmation only for that digest and local owner session, and refuse
changed, expired, cancelled, replayed, or restart-orphaned requests. The UI must
render the same immutable snapshot whose digest the daemon checks; it must not
re-read mutable pairing fields after confirmation.

### M8-R2 — Specify bounded parsing before canonicalization

**Severity:** High\
The proposal caps each message at 64 KiB and requires duplicate-key rejection
and closed field sets, but does not set explicit JSON nesting, object-member,
array-element, or string-byte limits for bootstrap messages. A small deeply
nested JSON input can still stress a parser before schema rejection.

Specify deterministic parser limits and their enforcement order: byte limit,
UTF-8/JSON validity, duplicate-key/depth/container limits, exact field/type
validation, then JCS and signature verification. Reject non-integer,
out-of-range, or otherwise non-canonical numeric forms for protocol integers.
Keep these limits separate from event-record limits and include boundary and
over-limit vectors.

### M8-R3 — Define durable pending-transaction and idempotency records

**Severity:** High\
The candidate requires durable pending state and says a committed transaction
is idempotent, but the exact persisted state transitions and lookup keys remain
open. This is necessary to distinguish a safe retry from a changed transcript
reusing the same transaction ID after process restart or an uncertain append.

Specify the durable pending record fields and allowed transitions for offer,
proof, each approval, cancellation, timeout, restart, and local anchor commit.
Idempotency must compare the complete transcript hash and the local identity/key
binding; a transaction ID with different transcript bytes must fail closed.
Define how unknown append outcome is reread under the event-store locks before
any retry. No new trust may be derived from volatile UI state.

### M8-R4 — Finish the v3 reader/chain contract before writing

**Severity:** High\
The current `crates/events/src/versioned_reader.rs` dispatches v1 and v2 and
classifies later envelope versions as unverifiable. Its chain verifier only
advances validated v1/v2 variants. The proposal correctly requires reader-first
rollout, but the v3 chain integration is not specified sufficiently to implement
without risking a second interpretation of existing bytes.

Before a writer exists, state exactly how v3 records share the existing
per-device sequence allocator and cross-version `prev_content_hash` chain,
which key resolver authenticates security records, and how a history containing
v3 policy records is wholly held by old readers. New readers must retain the
current v1 validator and v2 behavior unchanged, strictly validate v3, and fail
closed for unsupported security versions/types. Add byte-for-byte vectors for
v1→v2→v3 transitions and older-reader behavior.

### M8-R5 — Make identifier and key-binding equality canonical

**Severity:** Medium\
The candidate requires strict UUID validation and exact identity echoing, but
does not explicitly state the wire case/canonical spelling rule for UUID
fields. The shared validator accepts uppercase and lowercase UUID hex while
preserving the supplied spelling. Protocol comparisons must not accidentally
mix textual equality with UUID identity equality.

Choose one canonical UUID wire form for Brain and event identifiers (prefer
lowercase hyphenated text consistent with the other fixed-width lowercase
fields), reject alternate spellings at the protocol boundary, and define
DeviceId/key-binding equality as exact bytes or an explicitly normalized
representation. Add case-variant mismatch vectors. This is a protocol-format
clarification, not a reason to change existing v1/v2 bytes or validators.

## Reviewed positive constraints

- Offer/proof/approval messages bind the full transcript and both transport
  associations; transport identifiers are explicitly not trust roots.
- Signatures prove possession of the claimed key, while out-of-band owner
  comparison and explicit approval establish the intended peer; the proposal
  does not claim transport pairing alone authenticates a device.
- Cross-device restriction authority is limited to locally approved issuers;
  cycles, missing policy history, forks, and ambiguous results produce holds.
- The accepted owner permission to restrict another exact device/key remains
  explicit and does not create transitive admission.
- Security records are policy metadata and excluded from memory retrieval and
  projections; local trust is reconstructed from the local device's own
  authenticated anchor rather than imported pairing JSON.
- The proposed cutoff is sequence-based and preserves immutable history;
  plaintext already copied to an inaccessible device is not claimed to be
  erased.

## Still required before acceptance

1. Resolve M8-R1 through M8-R5 in the candidate and produce exact schema,
   generated bindings, and golden vectors.
2. Complete the separate issuer-authority/reducer review for mutual restrictions,
   cycles, multiple eligible cutoffs, key/device binding conflicts, delayed
   histories, and order-independent convergence.
3. Specify exact lock order, durable append/idempotency behavior, restore,
   downgrade, and every crash boundary in the authorization/transport/content
   sequence.
4. Obtain the required independent security review; this scoped review does
   not satisfy that requirement.
5. Only after explicit protocol acceptance, implement reader-first and run the
   required Windows batch followed by the consolidated macOS stage.

No Actions, trust-changing code, schema activation, writer, resolver, IPC
mutation, transport protection, or M7 write-back was enabled. M7 remains
deferred; managed-note I/O remains disabled pending the accepted physical
containment gate.
