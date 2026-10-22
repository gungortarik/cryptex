# M8 local approval anchors and restriction authority — review proposal

Date: 2026-10-22. **PROPOSED / NOT ACCEPTED. No implementation authority.**
This narrows the review questions in ADR-0019 section 8. It does not reserve
event names/schema versions, define cryptographic bytes or enable remote trust.
The Master Spec and accepted ADRs remain authoritative.

Subsequent owner decision (2026-10-22): the cross-device restriction permission
choice below is now selected in
`docs/adr/ADR-0019-m8-restriction-authority-addendum.md`. Historical unresolved
wording below describes the proposal before that choice. This document's
remaining anchor/schema/causal details are still proposed, not accepted.

## 1. Proposed local approval anchor

An existing installation's root remains its explicitly enrolled local identity,
whose public metadata is bound to its OS-held signing key under ADR-0018.
A transported roster or self-signed remote event cannot replace that root.

For each peer, propose a daemon-authored immutable local approval record in
the separately reviewed security format, binding ALL of:

- Brain UUID;
- approving installation DeviceId, full event public key and derived key ID;
- exact peer DeviceId, full event public key and derived key ID;
- both intended native transport identities/associations;
- pairing transaction identity and the exact approved transcript commitment;
- reference to the verified peer consent for that same transcript;
- issuer sequence/chain and reviewed policy context.

The record represents approval by this installation only. It does not confer
transitive authority on introduced devices. B approving C cannot add C to A's
resolver. A must explicitly approve C before accepting C's content. Peer names,
short key IDs and transport connection state cannot stand in for full bindings.

The existing `LocalReviewReceipt` is only an in-memory review acknowledgement.
It is not this durable anchor, proof of key possession or peer consent.

## 2. Rebuild, reinstall and existing-Brain join

| Situation | Proposed required action |
|---|---|
| Same installation/key; derived roster lost | Rebuild only from verified security history rooted in the same local enrolled key and explicit local approvals |
| Approval history missing/corrupt/incompatible | Hold; do not infer pins from Syncthing/M1 JSON or regenerate approvals |
| OS key missing or mismatches metadata | Refuse; do not silently create replacement identity/key under the old DeviceId |
| New installation joining an existing Brain | Explicit pending enrollment with a fresh installation/key; bilateral owner approval before remote trust/transport activation |
| Reinstall where original key cannot be proven | New installation enrollment; no automatic inheritance of the old device's local approvals |
| All ordinary trusted devices lost | M11 recovery-root process; no M8 substitute and no routinely stored recovery signing secret |

New-installation enrollment must not accidentally run first-Brain creation or
merge two independently created Brains. Exact temporary enrollment state,
durable bootstrap anchor and rollback boundaries are still protocol review
requirements. No existing identity setup implementation is declared sufficient
for cross-device join by this proposal.

## 3. Proposed authority rule for retire/revoke/compromise

Recommend the following explicit starting point for review:

1. A local owner operation is handled by the authenticated Desktop/daemon path;
   AI/MCP/imported memory cannot issue it.
2. A received restriction is eligible for authority analysis only if its issuer
   is directly approved at this installation, the issuer's exact signing key
   matches, and its causal policy context establishes authority at issuance.
   Merely being transported or having a mathematically valid signature is
   insufficient.
3. The target must be an exact Brain/DeviceId/full-key binding. Restrictions
   cannot operate by mutable display name or apply to another/new key.
4. Valid restrictions only tighten that binding's retained contiguous prefix.
   They do not delete history, implement Purge, revive an old grant or authorize
   a replacement key. Cutoff 0 retains no events; the boundary is inclusive.
5. If deciding the issuer's authority depends on disputed/forked/missing policy
   or on whether another mutually affected restriction is valid, hold guarded
   content. No timestamp, arrival order or preferred device breaks the tie.

This does **not** finish issuer authorization: the precise authority granted to
a directly approved peer (self-retirement only versus authority to restrict a
different peer) is not selected by existing ADR-0019. That permission must be
explicitly selected in the complete addendum, not inferred from “trusted”.
Until selected, neither alternative may be enabled. This proposal deliberately
does not present membership alone as a complete authority proof.

## 4. Concrete conflict cases the reducer must settle

| Case | Required outcome / review issue |
|---|---|
| Two independently authorized restrictions on B: 100 and 80 | Retain at most 80; still validate every retained event |
| Unsigned/unapproved restriction on B: 0 | Cannot enter a naive minimum; unsupported/unresolved security input may require a hold |
| Old admission arrives after a valid restriction on same key | Cannot widen accepted prefix or restore authority |
| A restriction event is above A's own accepted cutoff | Cannot assume it remains an authorized issuer action |
| A and B restrict each other so each decision would exclude the other | Explicit policy conflict hold; no arrival-order choice |
| Restriction affects a grant on which another issuer relies | Resolve the causal authority dependency or hold; no implicit transitive grant |
| Cutoff exceeds verified contiguous prefix | Cannot claim missing history verified; request explicit retained boundary |
| Clock jumps or peers have different clocks | Authority and cutoff result unchanged |

The examples are acceptance requirements, not executable reducer rules.
The complete specification must define the policy context and an order-independent
algorithm with bounds; these examples do not supply a consensus algorithm.

## 5. Wire/persistence prerequisites still required

- Exact bounded public bootstrap exchange, role binding, fresh challenge/consent
  lifecycle, full owner comparison and replay rejection. Do not invent a short
  authentication code or an encrypted channel.
- Exact strict security bytes, existing Ed25519/hash/canonicalization usage and
  domains, signature/hash coverage, security stream integration and golden
  vectors. No silent extension of frozen v1/v2.
- Reader-first compatibility and older-reader failure on the exact new bytes.
- Transaction identity/idempotency and crash table for every durable boundary;
  approval persists before transport/content visibility.
- Complete lock order integrated with current identity/inventory/device guards;
  no canonical lock held across owner dialogs or network waits.
- Scoped review of the COMPLETE proposal before accepting/activating it.

No new crypto protocol, canonical writer, remote resolver entry or trust mutation
is implemented. M8 remains open; no Actions, commit, push, merge or tests here.
