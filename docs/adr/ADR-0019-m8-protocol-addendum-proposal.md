# ADR-0019 M8 protocol and security-event addendum — proposal

**Status:** PROPOSED for scoped security review; not accepted and not implemented  
**Date:** 2026-10-22  
**Scope:** Concrete candidate for the remaining ADR-0019 §8 gates. The accepted
product permissions in ADR-0019 and its restriction-authority addendum remain
the authority. This document does not authorize a writer, resolver, transport
activation, schema rollout, or trust change.

## 1. Decisions this proposal must preserve

- Master Spec and accepted ADRs remain authoritative.
- `cryptexd` alone writes canonical state; accepted records are immutable.
- Existing event schema v1 and v2 bytes/readers are unchanged. Unknown security
  versions or security-policy conflicts fail closed for guarded Brain content.
- Transport, device names, matching ownership, health, and event-supplied keys
  never create trust. No transitive approval exists.
- A peer is locally trusted only after this installation's owner has directly
  approved its exact Brain/DeviceId/full event-key binding. Remote approval
  records do not add peers to this installation's trust set.
- A directly trusted installation may issue the owner-confirmed cross-device
  restriction selected in `ADR-0019-m8-restriction-authority-addendum.md`.
- Recovery-root signing stays offline and outside routine M8 operations.

## 2. Candidate bootstrap exchange

The exchange is a bounded set of signed public messages that can be transferred
by copy/paste, QR, or another user-controlled channel. The carrier is not an
authority and no secret is sent. Pairing over Syncthing alone is insufficient.

### 2.1 Common encoding and limits

Each message is UTF-8 JSON parsed with duplicate-key rejection, closed field
sets, strict UUID/hex validation, and RFC 8785 JCS. Proposed limits: 64 KiB per
message; at most 16 messages and 1 MiB total per transaction; 32-byte
cryptographically random nonces; 128-bit-or-better transaction IDs. Public
keys are exactly 32 bytes, signatures 64 bytes, and hashes 32 bytes. Hex is
lowercase and fixed width. No timestamps affect validity.

Candidate signed messages have exactly these common fields:
`protocol_version` (integer 1), `message_type`, `transaction_id` (32 random
bytes as 64 lowercase hex characters), `brain_id`, `sender_device_id`,
`sender_signing_key_id`, `sender_public_key` (64 lowercase hex characters),
`sender_transport_device_id` (the exact native ID string reported by the
configured adapter and accepted by its existing native-ID format validator),
`peer_device_id`,
`peer_signing_key_id`, `peer_public_key`, `peer_transport_device_id`,
`previous_hash`
(64 lowercase hex characters or all zeroes for the offer), `payload`, and
`signature` (128 lowercase hex characters). The offer sets all peer identity
and peer transport fields to null; later messages bind both full identities
and both transport associations. For the currently pinned Syncthing adapter,
the accepted representation is eight groups of seven uppercase base32
characters separated by seven hyphens (63 ASCII characters total), matching
the adapter's existing format check. That shape check does not prove ownership
or identity. Cryptex stores the exact adapter-returned value and never treats
it as application identity or trust. Closed payloads:

For each signed message, `sender_signing_key_id` must equal the first 16 bytes
of SHA-256 over the decoded 32-byte `sender_public_key`, encoded as 32
lowercase hex characters, exactly as in ADR-0016. `initiator_identity` and
`responder_identity` in the agreement object are each exactly
`{device_id, signing_key_id, public_key}` using those wire values; the
transport-association object contains exactly the two named transport ID
strings. No display name or mutable profile field enters either object.

| Type | Payload |
|---|---|
| `offer` | `initiator_nonce` (64 lowercase hex characters) |
| `proof` | `offer_hash`, `responder_nonce` (64 lowercase hex characters) |
| `approval` | `agreement_hash`, `role` (`initiator` or `responder`) |

Each signature is Ed25519 over
`UTF8("cryptex-peer-bootstrap-sig-v1") || 0x00 || JCS(message_without_signature)`.
Every message hash uses
`BLAKE3-256(UTF8("cryptex-peer-bootstrap-message-v1") || 0x00 || JCS(signed_message))`;
`offer_hash` and `proof_hash` are those hashes for their respective messages.
`agreement_hash` is BLAKE3-256 over
`UTF8("cryptex-peer-bootstrap-agreement-v1") || 0x00 || JCS({offer_hash,
proof_hash, brain_id, initiator_identity, responder_identity,
transport_association: {initiator_transport_device_id,
responder_transport_device_id}})`. Both approval
messages sign that same agreement hash. `transcript_hash` is BLAKE3-256 over
`UTF8("cryptex-peer-bootstrap-transcript-v1") || 0x00 || JCS([offer, proof,
initiator_approval, responder_approval])`. These are the four complete signed
message objects, in that fixed array order; the digest commits to every field
and signature. `agreement_hash` is represented as 64 lowercase hex characters
in messages and approval records. Messages are ordered `offer`, `proof`,
initiator `approval`, responder `approval`; `previous_hash` is the preceding
message hash in that order. Each side retains the exact transcript and can
append its own local approval record only
after it has both owner-approved signatures. These exact labels/structures
remain proposals until independent review and golden vectors pass.

The offer's sender is the initiator and its `previous_hash` is all zeroes. The
proof's sender is the responder, its peer fields exactly echo the offer's
initiator identity/transport association, and its `previous_hash` is
`offer_hash`. Approval role must match the message signer and order: the first
approval is from the initiator with `previous_hash = proof_hash`; the second is
from the responder with `previous_hash` equal to the first approval's message
hash. All four messages carry the same transaction ID and Brain ID. A verifier
rejects any identity, transport association, role, order, or link mismatch.

### 2.2 State sequence

1. The initiating daemon makes a fresh offer bound to its current Brain and
   local identity. A joining daemon must be in explicit `join-existing-brain`
   setup with a newly created local DeviceId/key and no conflicting Brain
   identity. An already initialized different Brain is never silently replaced
   or merged.
2. The receiving daemon returns a proof bound to the offer hash and a fresh
   receiver nonce, signed by its existing local event key. Both interfaces
   display Brain ID, Cryptex DeviceIds, full event-key fingerprints, exact
   transport DeviceIds, and the exact operation. Users compare the full values
   over a separate trusted channel. Transport IDs are association labels only.
3. The initiator confirms the immutable candidate snapshot and signs its
   approval. The responder verifies it, displays the same immutable snapshot,
   and signs only after its own owner confirmation. Any change to Brain,
   Cryptex DeviceId, key, transport DeviceId, transaction, or
   preceding-message hash invalidates the flow. Changing a transport association
   later invalidates this approval's transport association; transport cannot
   use the changed association until a separately specified, owner-approved
   association flow is accepted and completed.
4. Each side verifies both signed approvals and writes the exact transcript in
   its own immutable local approval record. Local trust becomes active only
   after that record is durable. If delivery or append is interrupted, the
   signed transcript can be retried idempotently; each device reports its own
   approval state and never claims the other device's durable state. Transport
   remains separately controlled and cannot create trust.
5. Cancellation, timeout, process restart, or mismatched/replayed transcript
   creates no trust. Retries use the same transaction ID and exact transcript;
   a committed transaction is idempotent, while changed bytes require a new
   transaction and fresh nonces. A transaction without durable local pending
   state is refused.

The full key fingerprint is lowercase hex of SHA-256 over the 32-byte event
public key (64 characters); the existing 16-byte signing-key ID is not the
comparison value. Short fingerprints are never the confirmation value. Names
are display-only. This defines a new signed bootstrap protocol, so it requires
independent protocol/security review. It introduces no new cryptographic
primitive, key-agreement scheme, or encryption protocol.

### 2.3 Durable approval anchor and reconstruction

An approval anchor is a signed immutable security record in the local
installation's own device event stream, bound to the full transcript digest,
local DeviceId/key/transport DeviceId, peer Brain/DeviceId/key/transport
DeviceId, and transaction ID. The local derived roster is rebuilt only from
the local stream's valid records rooted in
the same ADR-0018 OS-held key. An imported peer's approval record is evidence
of that peer's action but never grants trust at this installation.

Missing/corrupt local history, missing/mismatched OS key, or a different Brain
means hold/refuse; do not infer an anchor from M1 pairing JSON, Syncthing, a
peer stream, or a regenerated key. Reinstall without proof of the old key is a
fresh installation and requires explicit enrollment. Joining an existing Brain
must preserve its immutable Brain identity bytes; first-Brain creation and
existing-Brain join are mutually exclusive setup operations.

## 3. Candidate security-event format

Use a separately versioned strict event schema (candidate envelope version 3)
inside the existing immutable per-device event chain, with the existing
`device_seq` allocator shared by memory and security records. This gives
restrictions an unambiguous device sequence boundary without modifying v1/v2.
The candidate must be rejected if it cannot be integrated without changing
their accepted bytes or chain semantics.

The v3 signable envelope retains the established fields: `schema_version`,
`event_id`, `brain_id`, `device_id`, canonical decimal-string `device_seq`,
display-only `created_at`, `event_type`, `classification`, `signing_key_id`,
`payload`, and `prev_content_hash`. Security records use the fixed `internal`
classification. Closed production event types proposed:

| Event | Closed payload fields | Meaning |
|---|---|---|
| `security.peer_approved` | `transaction_id`, `transcript` | This authoring installation's direct owner approval. The transcript contains the signed offer, proof, and both approvals. Only the local author's stream creates local trust. |
| `security.device_restricted` | `target_device_id`, `target_signing_key_id`, `target_public_key`, `reason` (`retired` or `compromised`), `cutoff_seq`, `cutoff_content_hash` | Owner-confirmed restriction on one exact key and its verified contiguous event prefix. |
| `security.device_self_retired` | `last_authorized_seq`, `last_authorized_content_hash` | Terminal self-retirement by the event's own ADR-0018 enrolled identity; applies only to that identity. |

All `security.*` records are policy metadata, never user memory. They are
excluded from memory search, retrieval, entity/fact materialization, Obsidian
projection, and user-visible memory exports. Authenticated security status is
exposed only through separately authorized, typed status surfaces; record
bodies and public-key material are not returned as note content.

The restriction cutoff is inclusive and encoded as a strict decimal `u64`
string; `0` means no ordinary content events are retained. The hash binds the
cutoff to the target stream's exact event at that boundary (all-zero hash only
for cutoff zero). A restriction cannot be issued for a boundary beyond the
issuer's verified contiguous target prefix. The signed event itself is not
rewritten or deleted by applying the cutoff. A self-retirement event at
sequence `n` must name `last_authorized_seq = n - 1` and the exact content hash
at that sequence (zero sequence uses the all-zero hash). The self-retirement
control event is validated as policy metadata even though it is one sequence
after the last authorized ordinary content event. It cannot authorize another
device. Any later record in that identity's stream is ineligible for content
or policy authority; a fork, inconsistent boundary, or ambiguous retirement
causes a guarded-content hold.

Candidate security hash/signature domains are distinct from v1/v2:

```
content_hash = BLAKE3-256(UTF8("cryptex-security-content-v1") || 0x00 || JCS(signable))
signature    = Ed25519.Sign(local_seed,
                  UTF8("cryptex-security-sign-v1") || 0x00 || content_hash_raw)
```

This reuses the already selected primitive families and JCS, but the new labels,
version number, schema fields, framing, limits, and vectors are proposals only.
They require exact schema generation and an independent security review before
acceptance. Unknown v3 fields/types, duplicate IDs, malformed keys, bad
signatures, chain gaps, or unsupported versions are never partially applied.

## 4. Candidate issuer and causal reduction

All incoming security records are first verified for framing, strict schema,
Brain, immutable chain continuity, full-key binding, hash, signature, sequence,
and transaction references. No file arrival order or clock value enters the
result.

For each installation, only its own directly approved peers are eligible to
issue cross-device restrictions. The local device may issue a self-retirement
from its ADR-0018 enrolled identity without appearing in its peer roster. A
cross-device restriction is eligible only when its issuer was directly
approved locally and the restriction event sequence is within that issuer's
effective accepted prefix. A device cannot authorize a third device by
approving it. Each local owner action is signed in the local stream; a signed
claim that a human approved something is not independent proof against a
compromised key.

Candidate deterministic reduction:

1. Build a directed graph with edge `I -> T` for a restriction from issuer
   `I` to target `T`. Restrictions targeting `I` determine `I`'s effective
   cutoff, so source components are resolved before the devices they restrict.
   Edges from issuers not directly approved locally are excluded from the
   authority graph; their presence in a canonical device stream remains an
   invalid/untrusted-input condition handled by the Brain-wide fail-closed rule.
2. Resolve acyclic components in deterministic topological order; sort nodes
   with equal graph position by the canonical DeviceId byte sequence. A device
   without eligible incoming restrictions has cutoff `u64::MAX`; otherwise its
   effective cutoff is the minimum eligible incoming cutoff. An issuer action
   above its own effective cutoff is ineligible.
3. A directed cycle (including any self-edge; self-retirement is not modeled
   as a graph edge), competing fork, missing policy prefix, or result that
   depends on unavailable security history places the affected Brain in a
   guarded-content hold. It does not choose an event by arrival order, timestamp,
   device preference, or naive minimum over unauthenticated claims. Self-
   retirement is represented as an explicit terminal local action and does not
   grant that device any later authority.
4. Recompute from the complete validated union whenever new history arrives.
   Until the same result is derivable, no guarded content is returned. Previously
   exposed plaintext cannot be recalled.

Owner-confirmed self-retirement is terminal for that identity: it records the
last locally authorized sequence immediately before its control event and
cannot grant that identity authority over another device. The control event's
single-sequence exception is limited to validating that retirement record; it
does not retain ordinary content at that sequence. A cross-device restriction
that participates in a dependency cycle causes a hold. An unknown device
stream, invalid chain, or failed signature follows the existing Brain-wide
fail-closed rule and never enters the graph.

This is deliberately conservative. The proposed graph rule and hold scope need
security review for completeness, liveness, cross-device convergence, and
compromised-issuer behavior. No production reducer is authorized by this text.

## 5. Reader-first rollout, transaction order, and recovery

| Boundary | Required durable/observable result after crash |
|---|---|
| Offer/proof only | Pending transaction only; no trust, transport, or content. |
| Owner confirms before record commit | No trust. Retry must show the same immutable transcript or require a fresh transaction. |
| Sequence reserved, event not durable | Sequence may be skipped; never reused. No trust. |
| Security event durability uncertain | Return typed unknown outcome; reread by transaction ID before retry. Never append a second approval blindly. |
| Approval event durable, derived roster stale | Rebuild roster from canonical history before transport/content enablement. |
| Both owner approvals exist but local record is not durable | No local trust. Resume the exact transaction; do not claim peer durability. |
| Local approval durable | Local trust may be active; peer's durable state remains unknown. Transport remains separately supervised and cannot confer trust. |
| Restriction durable, projection incomplete | Apply conservative hold until cutoff/reducer projections are rebuilt. |
| Worker shutdown uncertain | Report unresolved; never claim clean shutdown or discard ownership state. |

Reader rollout precedes writer rollout. Older readers that encounter schema v3
must fail closed for the affected Brain, with a typed unsupported-security
version diagnostic; they must not ignore the record and expose content. New
readers must read v1/v2 unchanged and v3 strictly. Activation requires schema
and binding generation from Rust types, golden vectors, exact size limits,
cross-platform byte-for-byte tests, backup/restore and downgrade behavior.

The daemon holds its canonical inventory/device locks only for validation and
durable append; it never holds them across UI confirmation, clipboard/QR work,
or transport waits. Lock order and a transaction journal must be reconciled
with current event-store locks before implementation. Transport and guarded
content visibility are enabled only after durable bilateral approval and a
successful full rebuild. Incoming files remain untrusted until validation.

## 6. Required review and verification before acceptance

- Compare this candidate field-by-field with Master Spec, ADR-0015/0016/0017/0018,
  ADR-0019, M8, event-pack framing, storage locks, and OS SecureStore behavior.
- Independent security review of bootstrap message signing, replay/stale UI
  handling, domain separation, fingerprint derivation, key loss, and compromised
  device behavior.
- Independent review of the reduction algorithm, cycle/mutual restriction
  cases, cutoff boundary hash, fork handling, order-independent convergence,
  and guarded hold scope.
- Exact schema/TypeScript generation, JCS and signature golden vectors, older
  reader fail-closed fixture, size/depth/duplicate/unknown-field tests.
- Crash injection at every row in §5, idempotent retry tests, restore/rebuild and
  downgrade tests, cutoff `0`, `1`, and `u64::MAX`, and before/at/after checks.
- Windows batch first, then one consolidated macOS run against the exact same
  accepted source. No trust-changing implementation until review findings are
  resolved and this addendum is explicitly accepted.

## 7. Current status

This document is a concrete review candidate, not an accepted protocol. In
particular, the new schema/domain labels and causal graph rule are unreviewed.
No code, schema, writer, resolver, IPC mutation, pairing UI, or peer transport
activation is authorized. Existing Windows lifecycle receipts remain scoped to
their tested gates. M8 and the consolidated Mac stage remain open; M7 stays
deferred; no Actions are requested.
