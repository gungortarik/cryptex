# M8 Slice 2 peer admission — requirements and design boundary

**Date:** 2026-10-19  
**Status:** Preparation; no trust mutation or protocol selected by this record.

## Purpose

Define the next M8 trust boundary before pairing can change application trust.
Slice 1's `declared_trusted` count is diagnostic only. Neither a pairing JSON
record, a Syncthing device ID, a received event key, nor two machines sharing an
owner may authorize a peer. ADR-0018 deliberately keeps production retrieval
trust local-only until M8 supplies a reviewed authorization mechanism.

## Requirements already fixed by accepted documents

1. The daemon must bind explicit user-authorized peer trust to the intended
   Brain, stable Cryptex DeviceId and verified event-signing public key/key ID.
   Possession of transport configuration or an unauthenticated claimed key is
   insufficient. Private material remains in OS secure storage on its device.
2. A pending pairing cannot authorize content or security-policy writes.
   Privileged confirmation must use the authenticated Desktop boundary with
   narrow capabilities; AI, Markdown and imported memory cannot install trust.
3. Incoming records still require signature/schema/integrity-chain validation,
   intended Brain/device binding, and explicit trust/cutoff policy. Transport
   completion and health replies do not prove canonical acceptance.
4. Revoke/retire/compromise behavior uses device sequence cutoff semantics;
   wall-clock ordering must never decide security. Preserve full `u64`
   sequence values and the existing immutable event rules.
5. Existing v1 bytes, strict validation and golden vectors remain unchanged.
   Existing v2 reader/production-writer activation gates remain in force.
6. Remote admission must join canonical inventory/per-device locking and the
   shared `RetrievalSafetyState` guard. Unknown or unverifiable state must not
   expose partial content or silently mint replacement identities/keys.
7. Restart, replay, interruption, foreign-Brain/identity/key mismatch, concurrent
   trust changes and offline reconciliation require targeted failure coverage.
   Product guarantees must hold on both Windows and Mac.

Sources: Master Spec §§6–10; `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`;
`docs/07_SYNC_AND_CONFLICT_PROTOCOL.md` §§7–9;
`docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md` §§3, 9–13;
ADR-0015, ADR-0016, ADR-0017 and ADR-0018.

## Decisions to make concrete before adding a trust-changing command

| Decision | Why existing source is insufficient |
|---|---|
| Initial peer authentication and explicit confirmation ceremony | M1 defines pairing records only; transport identity is not Cryptex trust |
| Durable authorization / public-key binding representation and its authority | Mutable `PairingRecord.state` is not cryptographic authorization; neither v1 memory-note payload nor strict v2 knowledge-assertion payload defines trust-control data |
| Distribution/bootstrap and revocation reconciliation | A trusted-key resolver cannot trust a received key merely because its own received record says trusted; offline/cyclic authority and conflicting revocations need explicit rules |
| Cutoff boundaries and retained pre-cutoff history | Security ordering is fixed as device-sequence-based; exact transition representation and edge-case behavior require the M8 design |
| Reader-first compatibility and migration | A new security representation cannot be silently folded into frozen v1 or enable the separately gated v2 writer |

Next step is a narrowly reviewed implementation decision for these fields and
transitions, not widening `LocalDeviceKeyResolver` or adding a `pair` command
that writes `PairingState::Trusted`. If a proposal changes accepted architecture,
use the ADR/specification process before implementation. The M8 scheduling
exception authorizes the milestone; it does not settle these unspecified trust
design choices.

## Verification map for the reviewed design

- Explicit authorization and key/DeviceId/Brain binding; wrong/missing/stale
  confirmation, replay, replacement keys, untrusted sources, and transport-only
  pairing all fail closed.
- Pending attempts never become usable peers after restart or partial writes.
- Accepted authorization remains durable and idempotent across retries;
  revocation transitions cannot be undone by replayed/stale transport material.
- Before/at/after-cutoff events, `u64` edges and clock skew follow the reviewed
  sequence policy without changing existing event bytes.
- Newly observed untrusted/unknown canonical streams retain the existing
  Brain-wide content hold until explicit safe resolution.
- Actual Windows/Mac peer integration is distinct from fixture-only checks.

Until that design is resolved, Slice 1 remains diagnostic and every production
remote-trust or incoming-replication path stays unavailable.
