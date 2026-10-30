# ADR-0019 M8 protocol and security-event addendum — proposal

**Status:** PROPOSED overall; owner accepted §§2–4 as the Windows
implementation basis on 2026-10-29, including the operation-specific owner
approval digest fields below. This does not accept the full addendum or enable
trust, transport, retrieval, or publication.
**Date:** 2026-10-22  
**Scope:** Concrete candidate for the remaining ADR-0019 §8 gates. The accepted
product permissions in ADR-0019 and its restriction-authority addendum remain
the authority. This document does not authorize production writer/resolver
activation, transport activation, schema rollout, or a trust change.

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
       restriction_target, restriction_reason, cutoff_seq,
       cutoff_content_hash, last_authorized_seq,
       last_authorized_content_hash})
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
reason, and boundary hash are all required. `restriction_reason` is exactly
`retired` or `compromised`, and is non-null only for a device restriction.
Self-retirement fields are null except for self-retirement, where
`last_authorized_seq` and `last_authorized_content_hash` are both required and
bind the immediately preceding verified record in the local device chain;
they are null for admission and restriction. A zero sequence is permitted only
for the empty-prefix sentinel and must use the all-zero 32-byte hash. These
operation fields are frozen before display,
included in the canonical digest, stored in the durable journal, and rechecked
against locked canonical history before append. `local_identity` and non-null `peer_identity` contain the
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

**Proposed OS peer-credential pins (not yet accepted or natively verified):**
On Windows, for every named-pipe connection, obtain the peer PID from the
connected pipe handle using the kernel peer-credential API
(`GetNamedPipeClientProcessId`); compare it to the Desktop PID captured when
`cryptexd` was launched, and require the retained process handle for that exact
Desktop process to remain live. Resolve and validate the packaged Desktop image
through that pinned process identity at daemon startup. A missing PID,
mismatched PID, exited process, or unavailable process handle rejects the
connection; do not trust a PID in the request. The retained process handle
makes PID reuse fail closed.

This paragraph specifies only OS peer binding. It does not implement or close
the separate M8-R1 approval-session protocol: displayed digest binding,
one-time session consumption, cancellation serialization, and consume-before-
append remain independent requirements.

On macOS, for every connected Unix-domain socket, obtain the peer's opaque audit
token with `getsockopt(SOL_LOCAL, LOCAL_PEERTOKEN)`. At daemon launch, Desktop
obtains its own kernel audit token with `task_info(TASK_AUDIT_TOKEN)` and passes
it to the child over a one-shot inherited pipe (never argv, environment, or a
runtime file). The pipe is only a carrier, not authentication: the parent alone
owns the write end and the child alone receives the read end. The parent closes
its read end before spawning; the child closes any write end before reading.
Set close-on-exec/non-inherit on both ends and explicitly clear it only for the
one intended child read end at spawn. No grandchild or unrelated process may
inherit either end. The child must read exactly the fixed token size, reject
short/extra/malformed input, close the read end, and use documented
kernel-backed checks to prove that the token
identifies the live `getppid()` process instance and that this process is the
expected packaged Desktop executable before pinning it. The pipe ends must be
non-inheritable except for the intended child read end; token bytes must never
be logged. For each connection, match the kernel peer token to the pinned
process instance using PID plus a kernel process-generation identity (such as
pidversion), obtained and validated through a supported API, or an equivalent
kernel validation that the token still names the same live process instance.
Do not read opaque token slots directly from user space, and do not compare
unspecified mutable audit-token fields as a lifetime-stable byte string.
Current XNU source uses PID plus the embedded PID version in its internal
`proc_find_audit_token`; this does not by itself prove that Cryptex can call an
equivalent supported user-space API. Missing identity fields, failed
kernel/code-signature validation, a short read, process exit, mismatch, or
unavailable APIs reject the connection; there is no `LOCAL_PEERPID`-only
fallback. A daemon restart requires a new parent-bound handoff. Primary XNU
source confirms the socket/task token constants, the token's opaque nature, and
an internal PID-version check; the supported user-space process-generation
validation and token-bound code-signature APIs remain to be identified and
verified. Native tests must cover unrelated peers, forged handoff bytes, same-PID
replacement/reuse, credential changes, process exit during a connection, and
Desktop restart. This remains a review candidate, not an accepted security
guarantee.
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
   issuer. Edges from issuers not directly approved locally are excluded from
   the authority graph. A known, directly anchored issuer that is now ineligible
   because its valid history was later restricted or self-retired remains
   cryptographically verifiable; its actions are filtered by its effective
   prefix and do not become invalid input merely because its current authority
   changed. An unknown/unresolvable device stream or failed chain/signature
   remains subject to the existing Brain-wide fail-closed rule. Merely finding
   a well-formed policy event from a known but currently ineligible issuer is
   not itself a hold condition.
2. Resolve issuer-prefix dependencies on the restriction digraph with a single
   deterministic procedure (no arrival/time order). Let the candidate graph be
   the directed multigraph of step-1 edges `I -> T`. Proceed as follows:

   a. Compute the strongly connected components (SCCs) of the current edge set.
      Form the condensation DAG of those SCCs. Process SCCs in deterministic
      topological order of the condensation; break ties by sorting each SCC's
      member `DeviceId` values lexicographically by ASCII bytes and comparing
      the resulting sorted tuples.
   b. Before processing an SCC, every restriction whose issuer lies outside that
      SCC may contribute to an external cutoff on a member only when that
      issuer's effective authority cutoff is already fully resolved (its SCC was
      processed earlier in condensation order) and the issuer event sequence is
      at or below that issuer's effective cutoff. Ineligible or not-yet-resolved
      outside issuers contribute nothing. Never use an unresolved or ineligible
      outside edge to filter or break a cycle.
   c. Independently of SCC membership, each device's valid self-retirement
      boundary is a per-identity terminal cap on that device's ordinary content
      and policy actions. Apply every such cap when computing that device's
      provisional filter threshold. Self-retirement is not modeled as a graph
      edge and is not described as an "outside" cutoff.
   d. For each device in the SCC, the filter threshold used in this step is the
      minimum of `u64::MAX`, all eligible already-resolved external restriction
      cutoffs targeting that device, and its self-retirement cap if present.
      Remove every outgoing edge from that device whose issuer event sequence is
      strictly above its filter threshold. Those edges are known ineligible and
      cannot create a cycle hold.
   e. After removals, recompute the SCCs and condensation DAG of the affected
      subgraph. If any cycle remains anywhere in that recomputed subgraph, the
      authority is mutually affected and unresolved; apply the Brain-wide
      safe-conflict hold in step 4. Do not resolve an acyclic piece separately
      when another piece still contains a cycle. Only when the entire recomputed
      subgraph is acyclic may step 3 resolve it. Nested SCCs are handled only by
      condensation order: a downstream cycle must not be filtered until every
      upstream issuer it depends on is resolved or the Brain is already held.

   This procedure is still a candidate and requires independent re-review with
   the golden vectors in §6.
3. Resolve the fully acyclic recomputed subgraph in deterministic topological
   order; sort nodes with equal graph position by the canonical DeviceId byte
   sequence. For every device `D`, compute its effective authority cutoff over
   the whole verified candidate graph as the minimum of `u64::MAX`, its valid
   self-retirement boundary if present, and every incoming restriction
   `I@seq -> D` whose issuer `I` has a fully resolved effective cutoff and for
   which `seq <= effective_cutoff(I)`. This rule includes eligible incoming
   restrictions outside the SCC currently being processed; an edge does not
   stop contributing merely because an earlier cycle filter removed other
   edges from that SCC. Resolve eligible incoming restrictions in topological
   order so each issuer cutoff is known before evaluating its outgoing edges.
   A self-retirement record is a terminal control exception validated against
   the stream's prior verified record/hash; its `last_authorized_seq` caps all
   ordinary content and policy actions by that device. An issuer action above
   its effective cutoff is ineligible and cannot tighten any target. If a
   required issuer cutoff is not fully resolved, step 2 or step 4 applies; it
   must not be treated as absent and must not be replaced by an SCC-local
   minimum.
4. Any unresolved dependency cycle after step 2 (including a self-edge;
   self-retirement is not modeled as a graph edge), competing fork, missing
   policy prefix, or result that depends on unavailable security history places
   the affected Brain in a guarded-content hold. It does not choose an event by
   arrival order, timestamp, device preference, or naive minimum over
   unauthenticated claims. Self-retirement is represented as an explicit
   terminal local action and does not grant that device any later authority.
   A competing fork, for this candidate, means any of: two verified records in
   one device stream that share a `device_seq` or `event_id` with different
   content bytes; two divergent verified prefixes for the same device identity
   that cannot be reduced to one contiguous chain for cutoff binding; or a
   restriction/self-retirement whose `cutoff_content_hash` /
   `last_authorized_content_hash` does not equal the verified record at the
   named sequence. Each case is a typed hold, not a tie-break.
5. Recompute from the complete validated union whenever new history arrives.
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

This is deliberately conservative. A remaining unresolved dependency cycle
after the eligibility filtering in step 2 is a mutually affected authority
conflict under accepted ADR-0019. The reducer must
return a typed Brain-wide guarded-content hold, consistent with Master Spec
§9; it must return no partial guarded content and must not select a restriction
by arrival order, timestamp, device preference, or unauthenticated minimum.
Recompute the hold from the complete verified canonical union so replicas
converge on the same result. A compromised authorized issuer can therefore
cause denial of service; this is the explicit safe-conflict outcome permitted
by ADR-0019 §8, not an automatic cycle-resolution algorithm.

No M8 operation may clear or override this hold while the verified union still
contains the unresolved cycle. Master Spec §14 and `docs/08` §7–8 reserve the
offline recovery-root capability for first-device bootstrap after all ordinary
trusted devices are lost; it is not a routine M8 conflict override. M8 defines
no cycle-specific recovery operation, so the hold remains until a future,
independently reviewed policy can derive a safe outcome from canonical history.
The independent security review must confirm this fail-closed scope and
availability consequence before protocol acceptance. If that review finds the
hold or lack of M8 recovery inconsistent with the accepted contract, keep the
protocol NOT ACCEPTED and return for an explicit policy decision.

**Owner implementation decision (2026-10-29):** The owner accepts the
bootstrap/onboarding protocol in §2 and the security-event and trust/cutoff
reducer model in §§3–4 as the Windows implementation basis. This is not
acceptance of ADR-0019 as a whole and does not waive independent protocol or
security review. The parser, transcript verifier, reducer, and approval flow
may be implemented and tested, but trust, transport, retrieval, and publication
activation remain disabled until the required independent review and native
platform gates pass.

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

The M8 target holds canonical inventory/device locks only for validation and
durable append; it never holds them across UI confirmation, clipboard/QR work,
or transport waits. The Windows implementation slice now routes local
append/seal/recovery operations through a shared lock-set API that holds the
inventory lock through the device operation and acquires multi-device locks in
sorted DeviceId byte order. It does not yet add an importer publication barrier
or startup journal recovery. Section 5.1 remains a required M8 contract; the
remaining writer/importer paths and platform durability still require
verification. Transport and guarded-content visibility are enabled only after
durable bilateral approval and a successful full rebuild. Incoming files
remain untrusted until validation.

### 5.1 Required target lock order and post-confirmation revalidation

The required M8 transaction contract is:

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

The Windows implementation now follows this lock order for local
append/seal/recovery operations and provides a sorted multi-device lock-set
primitive. Import publication, startup reconciliation, and cross-platform
durability are still absent. The lock implementation is not a trust-activation
gate by itself; the complete contract and all required failure tests remain
mandatory.

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

#### 5.1.1 Owner-approved Syncthing-to-canonical publication flow

The owner approved this flow for the M8 Windows implementation slice on
2026-10-23. This approval applies to the implementation direction in this
subsection; it does not accept ADR-0019 as a whole, close the remaining
independent security review, or enable trust. The flow follows the Master Spec's
single supported canonical writer and the accepted sync protocol's separation
of transport from Cryptex semantics. It does not add an OS sandbox requirement
for the same-user Syncthing child.

1. Syncthing's only managed folder is a dedicated `transport/incoming` tree
   outside `canonical/events`. Within it, sealed candidates use the canonical
   relative naming shape `<device-id-hex>/<20-digit-segment-index>.seg`; the
   daemon requires the path's device stem to equal
   `DeviceId::as_filename_stem()` and its segment index to match the candidate
   metadata before staging. The daemon validates the managed folder
   ID and exact normalized path before starting transport and refuses startup
   if a folder points at canonical storage. Transport completion is only a
   hint to inspect incoming files.
2. Import staging and its journal live in daemon-controlled runtime storage at
   fixed siblings of `canonical/events` (for example,
   `canonical/import-staging` and `canonical/import-journal`), outside both
   `canonical/events` and `transport/incoming`, and on the same volume as
   canonical destinations. The daemon verifies the expected paths, volume,
   ownership, and non-link directory handles before use; it refuses startup if
   the layout cannot meet these conditions.
3. The daemon copies bounded, sealed `.seg` candidates into that staging root.
   It opens each source without following symlinks/reparse points, then checks
   identity through the opened handle: reject link count other than one and any
   `(device,inode)` / volume-file-ID that aliases an entry under
   `canonical/events`. Copy only through the opened handle with a fixed size
   limit; metadata checks followed by a path-based copy are insufficient.
   Freeze the observed size and verify identity, size, and modification state
   again after copying; reject a source that changed during the read. It also
   rejects unsafe segment names, incomplete frames, and any candidate that
   fails event schema, signature, device identity, Brain identity, sequence,
   predecessor-hash, trust, or cutoff validation. Validation applies to the
   staged snapshot, not a mutable transport path. Each copy is written to a
   unique staging temporary file, checked and synced through its handle, then
   atomically moved with no-replace semantics to its staged final name. Only
   complete staged files may be referenced by a prepared manifest. Importer
   admission limits are at most 16 sealed segment files per batch, 1 GiB per
   file, 4 GiB total per batch, and one prepared transaction at a time. The
   per-file cap accommodates 1,000 ADR-0016 maximum-size event records plus
   frame headers; these are importer resource limits, not event-format limits.
   The daemon reserves each declared length against the batch budget before
   copying, checks actual bytes while streaming, and discards the complete
   staged batch if a source grows, exceeds any cap, or otherwise fails
   validation. Exceeding any limit fails closed without partial publication.
   A caller-supplied key resolver or cryptographic verification primitive
   alone is insufficient. The reducer result and `RetrievalSafetyState` are
   bound to verified tips under canonical locks immediately before publication
   (step 5) and again before any recovery finish move (step 7).
4. Before publication, the daemon durably records a transaction manifest with
   transaction ID, affected DeviceIds, relative segment destinations, segment
   indexes, lengths, and SHA-256 digests in a closed, versioned schema. Stage
   files and the prepared manifest are synced before any canonical destination
   is made visible. Recovery must recompute and compare every staged and
   already-published file digest; manifest values alone are not evidence of
   file integrity.
5. Under the inventory lock and all affected device writer locks in sorted
   DeviceId order, the daemon revalidates the current inventory, trust/cutoff
   state, stream chains, and destinations. Segment indexes must be contiguous
   with the verified sealed stream tip; `.open` files, gaps, divergent duplicate
   indexes, and unreconciled `.open`/`.seg` pairs are rejected or held for
   recovery. Publication is additive and uses same-volume no-replace moves.
   Existing byte-identical segments are idempotent; a divergent destination
   fails closed. Existing canonical segments and events are never replaced in
   place. Before trust activation, every canonical mutator—including local
   append, restriction append, segment seal, and import—must use one reviewed
   shared lock API: acquire inventory first, then affected device writer locks
   sorted by DeviceId. The Windows implementation slice migrates current local
   append/seal/recovery entry points to this API and adds lock-order and
   interleaving tests. No importer currently publishes canonical segments;
   until its publication and startup-recovery gates are implemented and tested,
   trust activation stays disabled. Importer-only locks do not serialize
   current appenders.
6. The daemon syncs each destination and required parent directory, then
   durably marks the transaction committed before releasing canonical locks.
   The closed v1 marker is canonical UTF-8 JSON, at most 4 KiB, with exactly
   `protocol_version` (integer `1`), `transaction_id` (canonical lowercase
   hyphenated UUID), and `prepared_manifest_sha256` (64 lowercase hex digits).
   Its final name is `<transaction-id>.committed.json`; its only recognized
   temporary name is `.<transaction-id>.committed.tmp`, both directly in
   `canonical/import-journal`. Reject duplicate/unknown fields, non-canonical
   JSON, size overflow, or a filename/embedded-ID mismatch. Write the unique
   marker temporary in the marker directory and sync the file before placement.
   On Windows, use same-volume `MoveFileExW` with `MOVEFILE_WRITE_THROUGH` and
   without `MOVEFILE_REPLACE_EXISTING` or `MOVEFILE_COPY_ALLOWED`; require the
   no-replace move to succeed, then read back and validate the final marker
   before releasing canonical locks. Do not treat a successful
   `FlushFileBuffers` call on a directory handle as a documented guarantee:
   Microsoft's directory-handle API list does not include that function. The
   existing M8 host probe demonstrates that the call succeeds on one Windows
   filesystem only. Windows committed status remains unavailable until native
   interruption coverage exercises every marker boundary on supported target
   filesystems and the resulting limits are documented. Define and test crashes
   after each move, after the final move, during marker placement, and after
   marker persistence. Derived indexes update after publication and remain
   rebuildable.
7. Startup must acquire the single-instance lock and reconcile every prepared
   transaction under canonical locks before starting transport, inspecting
   incoming files, preparing or publishing another import, starting IPC,
   projection/materialization, retrieval, or any canonical enumeration. The
   target commit/recovery decision is deterministic and classifies the marker
   before destination visibility. A valid committed marker permits cleanup only
   when every destination matches; if any destination is absent or mismatched,
   hold. An unrecognized, malformed, or conflicting marker/temporary is a
   typed hold with evidence preserved. A recognized marker temporary may be
   atomically completed only after its transaction and manifest digest are
   revalidated and every destination is verified against the manifest;
   otherwise hold.
   With no committed marker or recognized marker temporary, no destination
   visible means validate the complete staged set and abort the prepared
   transaction while leaving the transport source retryable; with every
   destination visible and matching the prepared manifest, durably
   create/verify the committed
   marker and finish cleanup; with a strict subset visible, finish the remaining
   no-replace moves only when every visible destination matches and every
   remaining staged file matches the manifest. While holding the same canonical
   locks, subset recovery must rerun step 5's full inventory, trust/cutoff and
   `RetrievalSafetyState` binding, verified-tip continuity, destination/alias,
   and `.open` conflict checks immediately before any finish move. A failed or
   changed revalidation becomes a typed hold; it must not continue from the
   stale prepared snapshot. Then write/verify the marker; any missing, changed,
   conflicting, malformed, or unverifiable state is a typed fail-closed hold
   with evidence preserved. With no marker and all destinations present, the
   exact prepared-manifest digest and every destination must be revalidated
   before safely creating the marker; otherwise hold. Windows committed status
   is unavailable until
   marker-parent durability has a verified platform strategy and native
   interruption coverage; the staging/journal parent-flush helper alone does
   not prove marker durability. The importer holds the
   canonical locks throughout the finish/marker sequence, and readers remain
   gated until recovery completes. These are target rules; they are not
   implemented by the current pre-publication abort-only slice. No
   `for_each_verified_event*` or `scan_verified_canonical*` reader may run
   before reconciliation completes. This is required because a process crash
   releases locks but can occur between individual file moves.
   The recovery decision table is normative for this proposal. “Visible” means
   the expected canonical destination exists; “matches” requires a no-follow
   identity/metadata check plus exact length and SHA-256 equality with the
   prepared manifest. Any unlisted canonical entry, alias, reparse/link, or
   unexpected journal entry is a typed hold. Every row that creates or
   completes a committed marker must, while holding inventory and all affected
   device locks, rerun the complete step 5 validation against the current
   canonical snapshot, including trust/cutoff and `RetrievalSafetyState`, and
   recheck every destination immediately before any missing canonical move or
   marker creation/completion. A valid final marker is already the durable
   commit decision: its recovery row verifies the closed marker, exact
   prepared-manifest digest, and every destination before cleanup, but does
   not re-admit the transaction under a potentially newer trust/cutoff policy.
   The normal reducer still applies current policy to canonical content before
   retrieval.

   The normal committed window contains the final marker together with its
   exact prepared manifest and staging transaction until cleanup. Recovery
   must verify that companion transaction, its closed manifest digest, and the
   complete destination set before removing any transaction-local state. A
   valid marker with a missing, mismatched, or unverifiable prepared/staging
   companion is a typed hold; it is not permission to guess cleanup state.

   | Marker state | Destination state | Required action |
   |---|---|---|
   | Unknown, malformed, conflicting, or mismatched final marker | Any | Typed hold; preserve journal and staging evidence. |
   | Valid final marker bound to the exact prepared-manifest digest | All destinations, prepared manifest, and staging files match | Cleanup only this transaction after closed-schema, manifest-digest, identity, and file-hash checks; never alter canonical files. |
   | Valid final marker | Prepared manifest or staging companion is absent, mismatched, or unverifiable | Typed hold; preserve remaining evidence and canonical files. |
   | Valid final marker | Any destination absent or mismatched | Typed hold; do not infer or repair a committed transaction. |
   | Recognized marker temporary with valid closed schema and exact manifest digest | Every destination matches | Complete only the no-replace temp-to-final marker placement, sync/read back, then cleanup after locked validation. |
   | Recognized marker temporary | Any destination absent or mismatched | Typed hold; a marker temp cannot authorize missing canonical moves. |
   | No marker/temp; prepared transaction is valid | No destination visible; every staged file matches | Abort pre-publication transaction; preserve transport source for retry. No canonical mutation. |
   | No marker/temp; prepared transaction is valid | Strict subset visible; visible files match and all remaining staged files match | Rerun full locked step 5 validation; finish only remaining no-replace moves; durably create/read back final marker; otherwise typed hold. |
   | No marker/temp; prepared transaction is valid | Every destination visible and matching | Rerun full locked step 5 validation; durably create/read back final marker; cleanup only after marker validation. |
   | No marker/temp | Any visible destination mismatches, or staged file is missing/changed | Typed hold; preserve evidence and do not delete or overwrite canonical entries. |
   | Unknown marker/journal entry, multiple transactions, or unrecognized temp | Any | Typed hold; preserve evidence. |

   A valid prepared manifest alone is not proof that a destination is trusted or
   that an import committed. Canonical readers, writers, projection, transport,
   and IPC remain gated until startup recovery returns a terminal safe result.
   The owner approved this table for the Windows implementation slice on
   2026-10-23; it is normative for that slice, not a blanket acceptance of this
   ADR or authorization to activate trust. The current tree has an internal
   lock-bearing no-replace publication primitive and startup recovery for
   valid final and temporary committed markers when every destination matches.
   Partial-publication replay and production transport-to-publication wiring
   remain absent. Windows marker-parent durability and native interruption
   gates remain open, so the publisher is not activated.
 8. Tests must cover config/path refusal, links/reparse points, size/count
   bounds, duplicate-identical and divergent destinations, local append/import
   interleavings, every publication crash point, and startup reconciliation on
   both Windows and macOS. The Windows slice now has a bounded bootstrap
   parser, authenticated owner-approval IPC with one-time connection-bound
   sessions, and v3 `security.*` reducer integration with
   `RetrievalSafetyState`; these implementations still require independent
   security review and native-platform verification. Concrete peer-credential
   binding/revalidation remains a platform gate, including macOS live-process
   identity and PID-reuse coverage. Unknown security-event versions must fail
   closed.

Implementation status (2026-10-29): the owner accepted §§2–4 as the Windows
trust/reducer implementation basis, subject to the independent review and
native-platform gates above. The Windows daemon now snapshots bounded
incoming candidates into `canonical/import-staging` and runs an import startup
gate after the instance lock but before identity setup, transport, IPC, or
readers. It removes only recognized unprepared staging-only transactions. A
prepared transaction is validated under canonical inventory/device locks and
aborted only when every destination is absent and the complete staged tree
matches; any visible destination or changed/missing staged state remains a
typed recovery conflict. Recognized committed markers are parsed and bound to
their prepared companion under canonical locks. A valid final marker is
cleaned up only when every canonical destination matches the manifest; cleanup
removes only that transaction's staging, manifest, and marker and never changes
canonical files. A recognized temporary marker is moved to its final name only
after all destinations match, using no-replace placement, parent sync, and
read-back before the same transaction cleanup. Partial-publication replay and
transport-to-publication wiring remain unavailable. Unknown or conflicting
journal states still fail closed.

The Windows bootstrap parser/transcript verifier is implemented as a bounded
candidate-validation primitive. A daemon-owned bootstrap journal foundation
now models the frozen owner snapshot, exact transcript, state, stable event ID,
and append-attempt metadata; it validates duplicate keys, schema, snapshot
digest, verified transcript identity/transport/role-specific predecessor
bindings, and serializes atomic updates. A Windows authenticated Desktop IPC
path now verifies a bounded candidate, freezes and returns the immutable
snapshot, and issues a connection-bound one-use session. The Desktop review
panel displays the full identities/fingerprints and requires explicit owner
confirm or cancel. On confirm, the daemon revalidates the signed prefix,
consumes the grant, loads only the existing matching local signing key, writes
the signature to the journal, and then returns the signed message. On cancel,
it validates the same transcript/digest and persists cancellation without
signing. Loopback tests cover authenticated routing, snapshot binding, signed
message verification, and journal-before-release ordering. This is still not
the complete M8-R3 flow. A lock-bearing internal `security.peer_approved`
append foundation now verifies the complete transcript, local owner-approval
journal state, reduced canonical trust snapshot, global event/transaction
uniqueness, stable operation digest, and canonical reread; exact replay is
idempotent. An `append_unknown` entry with no exact canonical record is held
instead of retried blindly. Daemon startup now reconciles `append_unknown` and
`anchored` journal entries against the lock-scoped canonical history before
transport or IPC starts; only an exact canonical match is accepted, and other
states hold startup while preserving evidence. Startup does not append. These
synthetic append/replay/reconciliation tests pass. The authenticated Desktop
`bootstrap.approval.complete` IPC and Tauri local-trust action now invoke the
lock-bearing append path. Completion accepts only the full verified transcript
matching the journaled, explicitly released owner approval and frozen
snapshot; the daemon scans canonical trust/inventory under locks before
appending. A complete absence scan permits an explicit owner retry with a new
sequence, while startup reconciliation never appends. Tests cover success,
exact replay, startup reconciliation, and rejection without journaled owner
approval. This creates only a local canonical trust anchor; it does not start
transport or sync. Transport exchange/peer validation, native packaged UX and
interruption/durability proof, macOS parity, and independent security review
remain open.
The staging API now requires transport paths to encode the exact DeviceId and
20-digit segment index and rejects duplicate `(DeviceId, index)` candidates in
one batch. Rollback keeps ancestor handles pinned while removing an unprepared
transaction. `ImportStagingSession::verify_candidate` now reopens the staged
snapshot through the held-directory chain, checks object identity and digest,
parses only complete frames, verifies supported v1/v2 record signatures using
a caller-supplied trusted-key resolver, and checks device/Brain identity,
sequence, and previous-hash continuity from a caller-supplied verified stream
tip. This is a validation primitive only: it is not wired into publication and
does not replace trust/cutoff or `RetrievalSafetyState` checks. A first pure
Windows trust/cutoff reducer now implements deterministic SCC filtering over
a complete pre-verified policy-history input and has synthetic coverage for the
seven required reducer vectors plus fail-closed history states. It does not
parse or authenticate events, bind cutoff boundary hashes, or authorize
publication. A strict, separate v3 security-event parser authenticates closed
event envelopes, payload shape, canonical JCS, content hash, signing-key
binding, and Ed25519 signature. A verified-history layer orders supplied v1,
v2, and v3 records by device/sequence, rejects conflicting event IDs and
unsupported semantics, and checks cross-version chains; its cancellable API
never returns partial history. The reducer-input builder validates nonzero
cutoff sequence/hash/key-ID against the exact target-chain record and enforces
self-retirement's immediately preceding record and terminal behavior. These
layers trust the caller to provide an exhaustive lock-scoped snapshot and
locally trusted key resolver; they do not verify bootstrap message signatures
or derive the directly-approved roster. The normal version dispatcher still
classifies v3 as unverifiable. Neither verified-history nor reducer input is
wired to `RetrievalSafetyState`, production canonical scanning, publication,
or transport. Trust, transport, retrieval, and publication activation remain
disabled until those integrations, independent review, and native gates pass.
Staging now
compares the incoming opened-file identity against files under every canonical
device directory and rejects an alias. This admission-time scan is not race-free
and must be repeated under canonical locks before publication. Prepared/committed
journal support is partial: a session can write an immutable versioned
canonical-JSON prepared manifest with exact relative paths, lengths, and
SHA-256 values using a no-replace move. Closed-schema validation enforces
canonical transaction/device IDs, sorted contiguous segment entries, and
configured batch bounds. Startup parses and verifies the manifest under
canonical inventory/device locks. With no committed marker, it aborts only
when all destinations are absent and all staged files match. A matching
destination subset or a fully visible matching destination set is now
classified from exact identities, lengths, and SHA-256 values, but remains a
typed hold because the full trust/cutoff and `RetrievalSafetyState` recovery
gate is not wired. Mismatched, duplicated, or missing staged/destination files
also hold and preserve evidence. A closed v1 committed-marker parser and typed
startup hold exist. A valid final marker is cleaned up only after its exact
prepared-manifest digest, staging companion, and every destination pass locked
checks. A valid temporary marker is promoted no-replace only when every
destination matches, then synced, read back, and cleaned up under the same
locked checks. The working tree has an internal no-replace canonical publisher
and committed-marker writer that consume the local-policy/M3-M4 lock-bearing
token. They are tested on synthetic fixtures but are not called by managed
transport or a production IPC path. No-marker destination/companion replay and
transport-to-publication wiring remain unavailable. The prepared
staging/journal path now uses the M8-scoped
Windows `sync_staged_parent` directory-handle flush for placement/removal; this
does not establish durability for canonical destination publication, the
separate pending journal, or power loss. The Windows process-interruption tests passed 2/2 at the pre-marker checkpoint.
After companion/destination classification and lock-bound revalidation, the
focused group grew to 40 `import_staging::tests` on Windows; the daemon library
suite passed 440 tests with 4 ignored. `cargo fmt --all -- --check`, strict
release Clippy, and `git diff --check` pass. New Windows-host process-kill
coverage exercises the publisher after one destination move, after committed
marker temp sync, and after final-marker placement/read-back; recovery holds and
preserves evidence at each. These are process-crash and host-filesystem results,
not physical power-loss or supported-filesystem durability proof. Current
tests cover matching-subset classification, valid final/temp marker companion
holds, marker/manifest replacement after lock acquisition, malformed final and
temporary markers with staging present, final+temporary coexistence, and
byte-preservation of marker/manifest/staging evidence. They also exercise a
synthetic complete publisher transaction and recognize that moved staged files
are represented by matching canonical destinations. They do not exercise
valid-final cleanup or publication replay. Companion digest/tree checks begin
before lock acquisition to derive the lock set; then the marker and manifest
are re-read and compared and the staged-file subset/destinations are rechecked
under canonical locks. These runs supersede the earlier OS error 4551 result.
The isolated `pilot-fixture` canonical writer also runs the existing import
recovery gate after acquiring the instance lock and before writing identity or
events; it now reconciles intact prepared transactions as described above.
`verify_batch` additionally orders
all candidates by bytewise DeviceId and segment index, requires exact
starting-tip coverage, rejects segment-index gaps, and verifies sequence/hash
continuity across each device's staged segments. The
`verify_batch_against_canonical` wrapper derives the starting tips from a
complete versioned canonical scan while the inventory/device locks remain held;
the importer-only staging slot must be released first, with the unprepared
transaction tree retaining the single-flight claim. These remain validation
primitives and do not authorize publication or implement trust/cutoff/
`RetrievalSafetyState` policy. The focused batch/recovery tests are included in
the current Windows `import_staging::tests` result (21/21).
The Windows staging path now validates current-user ownership and walks each
local-drive directory component relative to a held parent handle, opening the
component itself so reparse points are rejected. Its focused native tests cover
junction refusal and a blocked ancestor rename while handles remain open; they
are not full race/fault-injection or independent-review evidence. macOS still
has only static path checks. The general Windows `sync_directory` remains a
no-op; the scoped `sync_staged_parent` flush is used for staging/prepared
journal and the test-exercised publication primitive, but does not prove
pending-journal or canonical-publication parent durability. Process-kill
publication boundaries are covered, but physical power loss and the target
filesystem durability strategy remain unresolved. The bounded bootstrap
parser/transcript verifier, pending journal, authenticated Windows approval
session, and local-trust completion path now exist. The v3 security reader and
trust/cutoff reducer feed `RetrievalSafetyState` on Windows. These local trust
and retrieval paths do not activate peer transport. Remote replication remains
blocked on transport exchange and validation, canonical publication/recovery,
platform durability, macOS parity, and independent security review.

The import journal uses unique transaction IDs, immutable prepared manifests,
and separate no-replace committed markers; it does not mutate one shared
manifest in place. The import single-flight mutex may only reserve/release
in-memory transaction ownership. Release it before waiting for transport or
canonical locks; never hold it while acquiring inventory/device locks. After
canonical locks are acquired, write the durable committed marker without
reacquiring the import mutex, then release canonical locks. Startup holds the
single-instance lock while reconciling manifests/markers before any transport
or reader starts. This import-journal mutex is distinct from the pending-journal
mutex described below; neither mutex may be held across canonical-lock waits.

The flow remains subject to independent security review and platform durability
verification before remote transport/publication activation. It does not claim
protection against arbitrary same-user malware that directly edits local
application data outside Cryptex's supported APIs.

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
- Before owner acceptance, pin a concrete connection-bound OS peer-credential
  mechanism separately for Windows and macOS. Document the OS identity evidence
  the daemon verifies, how it remains bound to the live IPC connection, and how
  process replacement/reuse is rejected; PID or same-user checks alone do not
  satisfy this gate. The candidate pins above remain PROPOSED until both native
  platform test matrices and an independent review pass.
- Independent review of the reduction algorithm, cycle/mutual restriction
  cases, cutoff boundary hash, fork handling, order-independent convergence,
  and guarded hold scope. Required reducer vectors include at least:
  (1) eligible external breaker that correctly drops one edge of `{A,B}` and
  resolves without hold; (2) ineligible external breaker
  `C→D(cutoff 2), D@5→A(cutoff 9), A@10→B, B@10→A` must hold `{A,B}` and must
  not apply `B@10→A` alone; (3) nested SCC where `{C,D}` unresolved holds the
  Brain before `{A,B}` is filtered; (4) self-retirement cap inside an SCC that
  drops a post-retirement outgoing edge and leaves an acyclic remainder;
   (5) identical verified unions under shuffled file-arrival orders converge on
   the same cutoffs or the same hold; (6) for an eligible, fully resolved
   `X→A(cutoff 2)`,
   `A@10→B`, `B→A`, and `A@3→E(cutoff 0)`, resolve `A` with effective cutoff
   2, exclude `A@10→B`, and do not apply `A@3→E`; (7) if filtering one SCC
   splits its recomputed subgraph into an acyclic piece and a cyclic piece, hold
   the whole Brain instead of resolving the acyclic piece independently.
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
- Before acceptance, independently review the cycle-induced Brain-wide hold,
  its no-partial-content behavior, replica convergence, and the absence of an
  M8 override. The accepted Master Spec/ADR-0019 permit explicit safe conflict
  holds; do not invent a cycle-breaking or M11 recovery-root path. If review
  finds the hold/recovery boundary inconsistent with the accepted contract,
  keep the protocol NOT ACCEPTED and obtain an explicit policy decision.
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
particular, the new schema/domain labels and causal graph rule remain
unaccepted. The 2026-10-22 independent re-review recommended NOT READY; a
documentation follow-up pinned SCC/condensation-ordered eligibility filtering,
self-retirement as a per-identity cap, a competing-fork predicate, and reducer
vectors. That follow-up is not a new independent review and does not close the
importer lock-participation or per-platform IPC binding gates. The 2026-10-25
Cursor follow-up also returned NOT READY: macOS handoff authenticity and
supported user-space process-generation/code-signature validation remain
underspecified; Windows peer binding is not the R1 approval-session protocol;
publish/recovery must rebind policy on the locked snapshot, revalidate subset
finish, and cover marker ambiguity and Windows marker-parent durability. Apple
XNU source confirms the opaque token warning and internal PID-version check,
but these do not establish a supported user-space identity API or native
guarantee. No code, schema,
writer, resolver, IPC mutation, pairing UI, or peer transport activation is
authorized by this proposal as a whole. The owner's 2026-10-23 approval in
§5.1.1 separately authorizes implementation of that Windows publication-flow
slice only; it does not accept the proposed trust schema, authorize trust or
peer transport activation, or close the independent review, durability, or
macOS gates. Existing Windows lifecycle receipts remain scoped to their tested
gates. M8 and the consolidated Mac stage remain open; M7 stays deferred; no
Actions are requested.
