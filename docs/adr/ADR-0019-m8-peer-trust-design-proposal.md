# ADR-0019 — M8 peer admission and security-event evolution

**Status:** ACCEPTED product direction only — complete protocol/security-event activation remains gated  
**Date:** 2026-10-20  
**Scope:** M8 device trust; no implementation activation

**Owner direction (2026-10-20):** After reviewing the concrete recommendation,
the owner answered "kurallara uygun ve dogru is elutfen yap". Proceed with the
recommended bilateral approval/no-transitive-grant direction subject to all
repository rules. This accepts the product direction, not an unspecified wire
protocol, a security-review result, or a new production writer. The §8 gates
remain mandatory. In-memory review/cutoff helpers and the bounded read-only
transport adapter may be implemented without enabling peer trust.

## 1. Context and authority

Master Spec §§6–9 and 13, documents 07/08 and the accepted M8 contract require
explicit device trust, immutable replication, sequence-based revocation and no
canonical master. ADR-0018 authorizes only the enrolled local key. M1's mutable
`PairingRecord` is display/setup data, not peer authorization. ADR-0016 freezes
v1 memory-note events; ADR-0017 defines strict v2 assertions whose writer remains
independently gated. Neither schema contains security-policy events.

The owner authorized M8 and Windows-first delivery. That does not select a trust
protocol or bypass schema gates. This proposal makes the product direction
reviewable; it does not claim the complete protocol is already designed.

## 2. Selected pairing direction

Pair only through authenticated Desktop operations handled by the daemon.
Both devices display the intended Brain, stable DeviceIds and full event-key
fingerprints. The user compares these through a separate trusted channel and
confirms on each device. Names, matching owner, transport connection, health
replies and file arrival cannot replace this check. Do not invent a short
authentication code or custom cryptographic handshake.

Bind approval to an immutable candidate snapshot. Changing Brain, DeviceId,
signing key or transport association invalidates approval. A stale dialog may
not approve replacement data. Cancellation or restart before durable approval
leaves the peer pending. Initial enrollment into an existing Brain is a separate
explicit setup flow, never implicit first-Brain creation or merging two Brains.
Its exact bootstrap transcript is a review gate in §8.

## 3. No transitive grants

Each installation explicitly pins its intended peer identity/key. A trusted
peer introducing a third device cannot authorize that device automatically.
For more than two machines, each installation approves a new peer before using
its content. No installation becomes canonical master.

Missing, replaced or conflicting pins fail closed. Do not widen ADR-0018's
local resolver until the M8 design is reviewed. Never authorize from transported
pairing JSON, filenames or event-provided keys. Private keys stay in OS secure
storage; they are never transferred during pairing.

## 4. Immutable security history and compatibility

Propose daemon-authored immutable canonical security events in a separately
versioned strict schema. Do not overload `memory.note`, add fields to frozen v1,
widen v2 assertions or make a mutable JSON roster canonical policy. A local
roster is derived and reconstructable from authenticated security history plus
explicit local bootstrap authorization.

The security reader and compatibility checks precede its writer. Exact schema,
version, domains, genesis/pin anchoring, storage integration and capability
negotiation require §8 review. Reuse audited Ed25519/BLAKE3/canonicalization
primitives only with a reviewed format; no custom cryptography. This proposal
does not enable the v2 writer or silently amend the Master Spec.

## 5. Sequence cutoffs and reconciliation

Propose an inclusive last-accepted sequence: `seq <= cutoff` is eligible for
all ordinary validation; `seq > cutoff` is ineligible. Eligibility alone is not
acceptance. Cutoff 0 accepts no events. Preserve full `u64` values through strict
decimal strings, not JavaScript numbers. Time remains display data only.

Retire means intentional cessation of future device writes; compromise means
the user distrusts its key and selects a safe retained-history boundary. Neither
deletes history or implements M10 Purge. Default to a verified contiguous prefix,
not the highest filename/sequence or unverified incoming tip. Both stop new
authorization by that identity under the reviewed causal rules.

Propose that independently valid restrictions for the same binding only tighten
the accepted prefix (minimum cutoff). Retries and stale grants cannot remove a
restriction. Re-enrollment needs a new identity/key and fresh approval.

This merge proposal is not a complete authorization proof. Who may issue a
restriction, issuer authority at its causal point, mutually revoked issuers,
delayed decisions and policy forks must be specified before restrictions can
apply automatically. Unresolved authority causes a content hold, never timestamp
last-write-wins. Do not implement a naive minimum over unauthenticated records.

## 6. Backend transport and safety

Retain pinned unmodified Syncthing 2.1.5, a private Cryptex profile and backend
process supervision/authenticated loopback REST per ADR-0004. No user-global
profile attachment or renderer API secrets/raw configuration. Streaming response
bounds, total deadlines, redirect refusal and process/profile ownership checks
are required product work, not established by M0 fixture success.

Transport enablement requires committed approval. Incoming material must pass
daemon-owned schema, signature, chain, Brain/key and cutoff validation under
inventory/device locks before guarded search/projections can use it. Remote
transport may not rewrite the local device's canonical stream.

Distinguish pending, transport-connected, validated and protected states.
Offline is normal. Key mismatch, unsupported policy, unknown streams, policy
conflicts and partial writes produce typed redacted diagnostics and applicable
Brain-wide holds, with no partial guarded content returned. Revocation cannot
erase already exported plaintext on a lost device. Normal operation must not
require the offline recovery secret; M11 disaster bootstrap remains separate.

## 7. Alternatives

| Alternative | Proposed disposition |
|---|---|
| Trust Syncthing or mutable pairing JSON | Reject: transport/display is not authorization |
| Automatically trust devices introduced by peers | Reject for initial M8: expands authority/compromise impact |
| Bilateral owner-approved identity/key pinning | Recommend: explicit intent, no master or cloud dependency |
| Silently extend v1/v2 with security fields | Reject: frozen/strict schema boundary |
| Make one laptop the trust master | Reject: violates peer/offline requirements |
| Timestamp-ordered revocation | Reject: prohibited by accepted documents |

## 8. Acceptance and implementation gates

Owner acceptance selects this product direction. It does not by itself review
a cryptographic protocol or activate a writer. Before trust-changing code,
complete and review the implementation addendum covering:

1. Exact bounded bootstrap transcript, replay/stale-confirmation rejection,
   key possession/authentication evidence and trust-anchor persistence/recovery.
2. Strict event bytes/domains, schemas and golden vectors; reader-first migration
   and fail-closed older-reader behavior.
3. Issuer-authority/causal rules, compromised peers, missing/forked policy and
   order-independent reduction or explicit safe conflict holds.
4. Atomicity/idempotency, lock order and restart across authorization, transport
   configuration and content visibility. Missing state cannot mint replacements.
5. Scoped security review against the Master Spec and accepted ADRs.

Then implement within the Windows batch followed by the one consolidated Mac
stage. Test wrong Brain/key/DeviceId, transport-only pairing, replay, cancelled
approval, restart at durability boundaries, cutoff 0/1/u64::MAX, before/at/after
cutoff, clock skew, revocation reordering/conflicts, partial transport, untrusted
streams and offline convergence. Retain current golden vectors and safety guards.
Actions require separate owner authorization.

## 9. Current effect

The product direction is accepted within the scope above. No peer trust, key
creation, schema change or writer activation results from this document.
The exact security protocol/schema is still gated by §8. M8 remains in progress;
M7 remains deferred and managed-note write-back disabled.
