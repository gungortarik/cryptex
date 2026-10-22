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
cryptographically random nonces; 256-bit (32-byte) transaction IDs. Public
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

### 2.4 Draft closure of scoped review findings M8-R1 through M8-R5

The following rules are proposed as normative protocol constraints. They close
the format ambiguities identified in the 2026-10-22 scoped review, but remain
subject to independent security review and owner acceptance.

**M8-R1 — immutable approval request.** Before showing a confirmation, the
daemon freezes the complete candidate and computes:

```text
approval_request_digest = BLAKE3-256(
  UTF8("cryptex-owner-approval-request-v1") || 0x00 ||
  JCS({brain_id, transaction_id, role, operation, agreement_hash,
       previous_message_hash,
       local_identity, peer_identity, transport_association,
       restriction_target, cutoff_seq, cutoff_content_hash})
)
```

`operation` is exactly `peer_admission`, `device_restriction`, or
`self_retirement`; `role` is `initiator`/`responder` for admission and `issuer`
for restriction/retirement. Each operation has a closed field shape:
`agreement_hash` and `previous_message_hash` are non-null only for admission;
they are null for restriction and self-retirement. For admission,
`previous_message_hash` is the hash of the exact preceding message in the
four-message order: `proof_hash` for the initiator's approval, or the
initiator-approval message hash for the responder's approval. This binds the
request to the exact signed prefix available before the local approval exists;
the complete `transcript_hash` is computed only after both approvals are
signed. `peer_identity` is null for self-retirement and is the exact peer or
target binding for admission/restriction. `transport_association` is null for
local self-retirement and otherwise contains the exact applicable adapter IDs.
Restriction fields are null except for a restriction, where target, cutoff,
and boundary hash are all required. `local_identity` and non-null `peer_identity` contain the
canonical DeviceId, 16-byte signing-key ID, and full public-key SHA-256
fingerprint. The daemon returns the frozen display snapshot and digest together
over the authenticated Desktop IPC. The UI renders that snapshot without
re-reading mutable pairing or transport state. Confirmation supplies that
digest plus a daemon-issued, single-use approval-session capability bound to
the exact authenticated IPC connection and owner-confirmation session. The
platform IPC adapter must establish the client process identity with an
OS-authenticated peer credential and bind it to that connection; PID alone or
same-user identity alone is insufficient. The capability is unguessable,
connection-scoped, and invalidated on disconnect or daemon restart. Each
platform's concrete peer-credential mechanism and its resistance to process
replacement/reuse are acceptance gates. The daemon compares the digest to its
pending request, verifies the operation and owner session, then consumes the
token. Validation and consumption are one atomic state transition scoped to
that authenticated session: concurrent confirmation, cancellation, and replay
requests serialize, and at most one can claim a still-pending token. If
cancellation wins, it invalidates the token. Confirmation consumes the token
before signature generation or canonical append. Any later error requires a
new displayed confirmation and token; a consumed token is never restored.
For admission it signs only the local approval message; the canonical
anchor remains gated on both approvals and the complete transcript. A
restriction or self-retirement may append its canonical event only after its
exact request is confirmed. After confirmation, the daemon reacquires the
required canonical locks and recomputes the approval snapshot, including the
target prefix and cutoff hash for restrictions. If any digest input changed,
it refuses this confirmation as stale, consumes the token, and requires a new
displayed snapshot and owner confirmation. A changed, cancelled, expired,
replayed, or restart-orphaned request is refused. Approval sessions expire
after five minutes of daemon monotonic time and are invalidated on IPC-session
close or daemon restart; a
durable pending transaction may be resumed only by displaying the same frozen
candidate and obtaining a fresh confirmation session. No owner confirmation is
inferred from a signed remote claim.

**M8-R2 — parser bounds before JCS.** For bootstrap messages only, enforce the
64-KiB raw UTF-8 byte limit before parsing, then parse with a streaming/token
path that rejects invalid UTF-8/JSON, duplicate decoded object keys, nesting
deeper than 4 containers, more than 24 members in any object, more than 16
items in any array, or any string token whose decoded UTF-8 representation is
longer than 16 KiB. The string bound applies to both object keys and values and
is enforced incrementally after JSON escape decoding. Reject unpaired
surrogate escapes and any invalid Unicode scalar sequence. Count the root
object as container depth 1; each nested object or array adds one level. Object
keys must be ASCII and belong to the exact closed schema. The only JSON number token
permitted is the exact integer token `1` for `protocol_version`; fractions,
exponents, negative zero, other numeric values, and out-of-range values are
rejected. The parser must enforce these bounds while consuming input, before
building an unbounded object or invoking JCS. After these checks, perform exact
field/type/identifier validation, then JCS, then signature/hash verification.
These bootstrap limits do not alter v1/v2 event parsing or event-record limits.

**M8-R3 — durable pending state and idempotency.** A per-installation,
daemon-owned pending journal stores only public protocol data: transaction ID,
Brain ID, operation kind, the exact frozen owner-confirmation snapshot and its
digest, the exact signed transcript messages received/generated so far,
transcript hash (null until complete), local DeviceId/key ID/full-key
fingerprint, peer/target binding where applicable, transport association,
role, state, whether a local approval signature has been generated and whether
it has been released externally, a stable event ID, and every append attempt's
reserved sequence, predecessor hash, and candidate content hash if append
outcome is uncertain. It never stores a private key or grants trust. At most 16
pending transactions and 1 MiB of journal payload are retained;
overflow refuses a new transaction before mutation. Allowed initiator
transitions are `offer_created -> proof_received -> local_approval_signed ->
transcript_ready`; allowed responder transitions are `offer_received ->
proof_created -> peer_approval_received -> local_approval_signed ->
transcript_ready`. Either role then transitions to `append_unknown` only on
uncertain durable append and to `anchored` only after canonical verification.
The first cryptographically valid proof durably pins the transaction to its
exact peer DeviceId, signing-key ID/public-key fingerprint, and transport
association. An exact byte-identical replay is idempotent. Any later proof
using the same transaction ID with a changed peer identity, key, or transport
binding is a typed transaction conflict; it cannot replace the pinned proof or
silently alter the state machine. Recovery requires a new transaction ID and
fresh nonces.
Owner cancellation or explicit owner expiry before a local approval signature
is released is terminal from any nonterminal state. After a signed approval has
been released externally, local cancellation prevents local anchor/continuation
but cannot revoke that already-sent signature or claim global cancellation;
the UI must report that limitation and the peer may already have its own local
approval. There is no automatic wall-clock expiry; the five-minute
approval-session expiry is the only timer and is measured by the daemon's
monotonic clock. Every journal transition is atomically persisted before its
corresponding response or external message is released. Candidate journal
writes use a unique temporary file in the journal directory, exclusive
creation, complete write, and file sync before replacement. On macOS,
replacement uses same-volume atomic rename followed by reopening/syncing the
destination and syncing its parent directory;
on Windows, replacement uses the existing `MoveFileExW` replace/write-through
pattern and reopens/syncs the destination; parent-directory sync is not claimed
on Windows. These are the currently evidenced platform steps, not a guarantee
beyond the documented OS/filesystem behavior. Unsupported platform durability
fails closed. Journal reads enforce a closed schema, size bounds, duplicate-key
rejection, and recompute stored digests; the journal remains untrusted cache
data and never establishes trust. Before releasing a previously generated
local approval message, verify its signature, transaction binding, and exact
match to the frozen snapshot being displayed. A durable `anchored` state is
only a cache: canonical local `security.peer_approved` history remains the
authority. The journal is reconciled against canonical history on startup;
nonterminal state must be re-presented as the same frozen candidate and receive
a fresh local confirmation session. Missing, malformed, or identity-mismatched
journal state causes refusal and a fresh transaction with new ID/nonces, never
trust. Before any new sequence reservation, and before every anchor append or
retry, hold the canonical inventory and required device locks in the
established order, verify the complete relevant local stream, and search both
the transaction ID and stable event ID. Exactly one record with the same
transaction ID, event ID, operation digest, transcript hash, and local
identity/key is idempotent success. A duplicate ID, or any existing event ID
with a different operation digest or transaction binding, is a typed conflict
and Brain-wide hold. Never append a second body for an existing event ID. For
an uncertain append, reread under those locks first: an exact matching record
means success; a mismatching record is a conflict. A retry may reserve a new
sequence only after the fully verified stream proves both IDs absent. It may
then reuse the transaction/event IDs with the same operation and payload but a
new sequence and current predecessor hash; every failed reservation remains a
permanent gap. Conflicting or unverifiable state is held. Journal loss never
authorizes blind append. A new transaction requires fresh nonces and a new
transaction ID.

**M8-R4 — v3 cross-version chain and key resolution.** A v3 security record
uses the same per-device sequence allocator as v1/v2 and the exact existing
cross-version `prev_content_hash` link: its predecessor hash is the prior
record's verified `content_hash`, regardless of predecessor schema version;
its successor links to the v3 record's verified content hash. Sequence gaps
remain allowed only as already defined by the shared allocator; each record
must still have a strictly greater sequence and correct previous hash. The v3
hash/signature use the v3 domains below, and no v1/v2 bytes, validators, or
domains change. A new reader resolves the local stream only with the ADR-0018
enrolled local key and a remote stream only with an exact peer key binding
rooted in this installation's directly approved local anchor. Imported peer
approval events may be validated as that peer's history but never add a key to
this local resolver. Missing or conflicting binding holds the Brain. An older
reader that encounters `schema_version: 3` classifies the envelope as
unverifiable; it must not advance that device chain or return guarded content
from the affected Brain. A new reader accepts v1 and v2 only through their
existing validators and applies the strict v3 validator. Golden vectors must
cover v1->v2->v3->v1 with valid links, a wrong predecessor hash, a sequence gap
with a valid chain, and an older-reader v3 hold.
After strict verification, recognized `security.*` v3 records are policy
metadata: the new reader must feed them to the deterministic trust/cutoff
reducer and update `RetrievalSafetyState` from that result. They must not be
classified as generic unsupported memory semantics. Unknown v3 types,
unverifiable security records, or an unavailable/inconsistent reducer remain
fail-closed holds. A v3 writer or trust activation is forbidden until this
reader/retrieval integration and its tests exist.

**M8-R5 — canonical identity equality.** `brain_id` in every bootstrap
message, and `brain_id` plus `event_id` in every candidate v3 canonical
security event, use exactly lowercase hyphenated UUID text (8-4-4-4-12);
uppercase hex, braces, compact forms, and alternate spellings are rejected at
this protocol boundary. At the M8 boundary, parse the existing local Brain ID
with the existing UUID validator, derive its canonical lowercase wire form
once, and compare bootstrap/v3 identifiers by their 16 UUID bytes. This handles
legacy local metadata whose accepted spelling is uppercase without changing
the strict incoming wire rule or rewriting v1/v2 event bytes. If the local ID
cannot be parsed unambiguously, refuse before starting the transaction. This
rule does not change existing v1/v2 acceptance.
`DeviceId` is the existing ASCII `[A-Za-z0-9_-]{1,64}`
identity and compares byte-for-byte, case-sensitively; it is never
case-folded. Key IDs compare as exactly 16 decoded bytes and public-key
fingerprints as exactly 32 decoded bytes; their wire form is fixed-width
lowercase hex. Public keys compare as their exact 32 bytes. Transport IDs
compare as exact adapter-returned ASCII strings after the adapter's existing
format validation. Case-variant identity and key-ID values fail exact binding
checks; they are not silently normalized.

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
| `security.device_restricted` | `transaction_id`, `target_device_id`, `target_signing_key_id`, `target_public_key`, `reason` (`retired` or `compromised`), `cutoff_seq`, `cutoff_content_hash` | Owner-confirmed restriction on one exact key and its verified contiguous event prefix. |
| `security.device_self_retired` | `transaction_id`, `last_authorized_seq`, `last_authorized_content_hash` | Terminal self-retirement by the event's own ADR-0018 enrolled identity; applies only to that identity. |

All `security.*` records are policy metadata, never user memory. They are
excluded from memory search, retrieval, entity/fact materialization, Obsidian
projection, and user-visible memory exports. Authenticated security status is
exposed only through separately authorized, typed status surfaces; record
bodies and public-key material are not returned as note content.

The restriction cutoff is inclusive and encoded as a strict decimal `u64`
string; `0` means no ordinary content events are retained. The hash binds the
cutoff to an exact event in the target stream's verified chain prefix; all-zero
hash is allowed only with cutoff zero. A cutoff must name an existing event
sequence in that prefix, not an unoccupied number reserved by an interrupted
append, and cannot exceed the verified prefix tip. “Contiguous prefix” means
the complete verified event chain in stream order; numerical sequence gaps
remain possible under the shared allocator. The signed event itself is not
rewritten or deleted by applying the cutoff. A self-retirement event at
sequence `n` must name the immediately preceding verified chain record's
sequence and exact content hash, which may be less than `n - 1` after skipped
reservations. If there is no preceding record, use cutoff zero and the
all-zero hash. The self-retirement control event is validated as policy
metadata as the sole terminal control exception; it does not authorize
ordinary content at or after its own sequence and cannot authorize another
device. Any later record in that identity's stream is ineligible for content
or policy authority; a fork, inconsistent boundary, or ambiguous retirement
causes a guarded-content hold.

Every security-event `transaction_id` is a fresh 32-byte random value encoded
as 64 lowercase hex characters. It is unique within the local author's stream
across all `security.*` event types. Before append/retry, the daemon searches
that stream under its writer lock: the same transaction ID with the same event
type and exact operation digest is idempotent; reuse for a different operation
or payload is a typed conflict and Brain-wide hold. Define the operation digest
as:

```text
security_operation_digest = BLAKE3-256(
  UTF8("cryptex-security-operation-v1") || 0x00 ||
  JCS({schema_version, brain_id, device_id, event_type, classification,
       signing_key_id, payload})
)
```

`payload` includes `transaction_id`. Exclude `event_id`, `device_seq`,
`created_at`, `prev_content_hash`, `content_hash`, and `signature`: a retry
must preserve the operation while using the shared allocator's newly reserved
sequence and current predecessor. Exactly one matching canonical record is
required for idempotent success. More than one record with the transaction ID
is a typed conflict and Brain-wide hold, even if their operation digests match.
The stable transaction ID plus this digest lets recovery find restriction and
self-retirement operations as well as pairing anchors after an uncertain
append.

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
signatures, missing/broken predecessor links, or unsupported versions are
never partially applied. Strictly increasing sequence numbers may contain gaps
when a reservation was durably consumed before an append failed, as specified
in ADR-0015/0016; a valid hash link is still required across such a gap.

The v1 -> v2 -> v3 vector must include each version's own hash/signature
domains. Every link carries the verified predecessor `content_hash`; it does
not recompute a predecessor using the current record's schema. The v3 canonical
JCS object includes the exact v3 envelope fields and is signed only after
strict v3 validation and per-device key resolution.

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

1. First validate every candidate restriction's exact schema, event signature,
   chain, Brain, issuer identity, target binding, and boundary hash. Build a
   directed candidate graph with edge `I -> T` for each structurally and
   cryptographically valid restriction authored by a locally directly approved
   issuer. Detect directed cycles in this candidate graph before applying
   issuer-prefix eligibility; any such cycle places the Brain on hold, even if
   a cycle edge might later be outside its issuer's effective prefix. This is
   deliberately conservative and requires independent liveness/DoS review.
   Restrictions targeting `I` determine `I`'s effective cutoff, so source
   components are resolved before the devices they restrict. Edges from
  issuers not directly approved locally are excluded from the authority graph.
  A known, directly anchored issuer that is now ineligible because its valid
  history was later restricted or self-retired remains cryptographically
  verifiable; its actions are filtered by its effective prefix and do not
  become invalid input merely because its current authority changed. An
  unknown/unresolvable device stream or failed chain/signature remains subject
  to the existing Brain-wide fail-closed rule. Merely finding a well-formed
  policy event from a known but currently ineligible issuer is not itself a
  hold condition.
2. Resolve acyclic components in deterministic topological order; sort nodes
   with equal graph position by the canonical DeviceId byte sequence. For each
   device `D`, its effective authority cutoff is the minimum of all eligible
   incoming restriction cutoffs, its valid self-retirement boundary if present,
   and `u64::MAX` (the identity value when neither restriction exists). A
   self-retirement record is a terminal control exception validated against the
   stream's prior verified record/hash; its `last_authorized_seq` is also a cap
   on all ordinary content and policy actions by that device. For each
   cross-device restriction `I -> T`, the issuer event sequence must be at or
   below `I`'s effective cutoff to contribute `cutoff_seq(T)`. The DAG order
   resolves upstream issuer caps before their outgoing restrictions. An issuer
   action above its effective cutoff is ineligible and cannot tighten any
   target.
3. Any candidate-graph cycle detected in step 1 (including a self-edge;
   self-retirement is not modeled as a graph edge), competing fork, missing
   policy prefix, or result that depends on unavailable security history places
   the affected Brain in a guarded-content hold. It does not choose an event by
   arrival order, timestamp, device preference, or naive minimum over
   unauthenticated claims. Self-retirement is represented as an explicit
   terminal local action and does not grant that device any later authority.
4. Recompute from the complete validated union whenever new history arrives.
   Until the same result is derivable, no guarded content is returned. Previously
   exposed plaintext cannot be recalled.

Owner-confirmed self-retirement is terminal for that identity: it records the
last verified chain record before its control event (not the arithmetic value
`n - 1`, because a reserved sequence may have been skipped) and cannot grant
that identity authority over another device. The exception is limited to
validating that actual retirement record; it does not retain ordinary content
at or after the control event's sequence. A cross-device restriction
that participates in a dependency cycle causes a hold. An unknown device
stream, invalid chain, or failed signature follows the existing Brain-wide
fail-closed rule and never enters the graph.

This is deliberately conservative. A valid signed cycle can cause a Brain-wide
guarded-content hold even when one or more edges would be ineligible after
prefix resolution; a compromised authorized issuer can therefore cause a
denial of service. This candidate does not define a cycle-specific recovery or
break-glass operation. The owner must explicitly accept this availability
consequence and its recovery path, or select a separately reviewed cycle rule,
before protocol acceptance. No production reducer or trust activation is
authorized while this decision is open.

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

Pending transaction transitions are persisted atomically in the local
daemon-owned journal described in §2.4. This journal is never trust authority.
After restart, reconcile it against the local immutable stream under the same
inventory/device lock order before any retry. If an append may have completed,
look up the transaction ID and compare transcript hash, local key binding,
event ID, and event content hash before deciding success or retry. A changed
transcript under a reused transaction ID is a typed conflict; do not append it
or silently allocate a replacement ID.

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

### 5.1 Candidate lock order and post-confirmation revalidation

The current daemon writer/read path establishes this candidate order:

1. Acquire `canonical/events/inventory.lock` whenever a stable full inventory
   or a new device directory is required.
2. Acquire every required `canonical/events/<device>/writer.lock` in ascending
   byte order of the case-sensitive ASCII `DeviceId`. Keep the inventory lock
   held while the device set is enumerated and device locks are acquired.
3. For a local append only, acquire the sequence-store process gate and that
   author's sequence-counter file lock, durably reserve the sequence, then
   release both sequence-store locks. Append/sync the immutable frame while
   retaining the required inventory/device locks. A crash after reservation
   can leave a permanent sequence gap. Never acquire an inventory or device
   lock while holding a sequence-store lock.
4. Release the remaining locks in reverse order. If a derived SQLite update is
   required, begin its transaction only after canonical device locks, following
   the existing `device -> SQLite` rule. Policy authority is never committed
   in SQLite.

The owner dialog, QR/clipboard exchange, and transport waits occur with none of
these locks held. The confirmation token binds the frozen display digest, not a
lock held across human interaction. After owner confirmation, the daemon
reacquires the exact inventory/device set, including both the restriction
issuer and target device writer locks (or the single affected device for
self-retirement), in sorted DeviceId order under the inventory lock. It checks
for inventory changes and revalidates the target prefix, issuer authority, transcript/transaction
references, and all digest fields before append. A changed inventory or
boundary invalidates the one-use confirmation and requires a new displayed
snapshot. Any sync/import actor that can mutate canonical event files must join
this lock protocol before trust activation; detecting an out-of-protocol
mutation after the fact is not a race-free substitute. This order remains a
proposal until checked against every writer/importer and platform durability
path.

The importer contract is a release gate: a sync/import actor must acquire the
inventory lock and all affected device writer locks in this order before it
publishes, replaces, adds, or removes canonical event frames, and retain those
locks until the complete validated update is durably visible. Readers must not
observe a partial multi-file import. A Syncthing folder that can directly
mutate canonical event files without this lock participation is not safe for
trust activation; post-hoc inventory scans or filesystem watchers do not close
the race. The concrete sync-to-canonical publication mechanism and its
interleaving/failure tests remain unimplemented and must be reviewed before
trust can be enabled.

The pending-journal transaction mutex must not be held while waiting for
inventory/device/sequence locks or for owner/transport input. Claim a
single-flight transaction state under that mutex, release it, then acquire the
canonical locks; persist the resulting state after releasing canonical locks.
On crash between canonical append and journal update, startup reconciliation
uses the event ID, transcript hash, and recorded append-attempt hashes to
discover the durable result. Journal/canonical mismatches fail closed.

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
  reader fail-closed fixture, size/depth/duplicate/unknown-field tests, and
  security-operation-digest vectors covering retry with changed chain placement,
  changed operation payload, cross-type transaction-ID reuse, and duplicate
  canonical records. Bootstrap bounds require a dedicated streaming/tokenizer
  parser and reject vectors for duplicate decoded keys, non-integer `1.0`,
  depth 5, 17 array items, and a decoded string of 16 KiB + 1 bytes; existing
  event parsers do not satisfy the pre-allocation bootstrap bound.
- Tests must cover the same transaction ID receiving a conflicting second
  proof; same-user/different-process approval attempts, IPC disconnect and
  process replacement; target-prefix mutation while a restriction is being
  confirmed; formerly approved but now prefix-ineligible issuer history;
  v3 `security.*` policy records updating retrieval safety; and crash recovery
  that finds an event by transaction/event ID before reserving another sequence.
- Import tests must race a reader/restriction append against an incoming
  multi-file canonical update and prove lock participation prevents partial or
  mixed snapshots. Journal durability fault injection must match the actual
  platform guarantee: Unix parent-directory sync is required; Windows uses
  `MoveFileExW` replace/write-through and destination reopen/sync, without
  claiming parent-directory sync. Test canonical UUID output from uppercase
  legacy local Brain metadata while leaving all existing v1/v2 bytes unchanged.
- Before acceptance, record the owner's explicit decision on the cycle-induced
  Brain hold and a reviewed recovery path, or replace it with a separately
  reviewed deterministic rule. Do not treat the existing “liveness review” note
  as acceptance.
- Crash injection at every row in §5, idempotent retry tests, restore/rebuild and
  downgrade tests, journal write/flush/replace/readback fault injection and
  torn/corrupt journal recovery on both platforms, cutoff `0`, `1`, and
  `u64::MAX`, and before/at/after checks.
- Windows batch first, then one consolidated macOS run against the exact same
  accepted source. No trust-changing implementation until review findings are
  resolved and this addendum is explicitly accepted.

The scoped 2026-10-22 findings M8-R1 through M8-R5 have draft resolutions in
§2.4 and §3. They are not considered independently reviewed or accepted by
their inclusion here; the required independent security review must challenge
the selected limits, approval binding, journal transitions, v3 reader contract,
and canonical identity rules before acceptance.

## 7. Current status

This document is a concrete review candidate, not an accepted protocol. In
particular, the new schema/domain labels and causal graph rule are unreviewed.
No code, schema, writer, resolver, IPC mutation, pairing UI, or peer transport
activation is authorized. Existing Windows lifecycle receipts remain scoped to
their tested gates. M8 and the consolidated Mac stage remain open; M7 stays
deferred; no Actions are requested.
