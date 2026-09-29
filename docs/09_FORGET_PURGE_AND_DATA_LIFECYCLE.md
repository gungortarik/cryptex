# 09 — Cryptex Forget, Purge and Data Lifecycle

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the accepted semantic boundaries between Forget and Purge and the limits of Cryptex deletion guarantees.

It preserves integrity, stale-device suppression, recovery honesty and attachment/reference safety without inventing implementation details that belong to M10.

## 2. Forget

Forget removes content from normal retrieval and AI context while remaining reversible.

Forget is a semantic state change, not guaranteed physical erasure.

Forgotten material may remain in canonical history according to the accepted model so it can be restored later.

The exact user workflow and storage representation belong to M10.

## 3. Purge

Purge is a deliberate content-removal workflow.

Purge wins semantically over stale replicas.

A stale peer must not be allowed to silently reintroduce purged content as active memory after reconnection.

The exact purge protocol is implemented in M10.

## 4. Integrity bridge

The audit resolves the purge/hash-chain conflict by allowing minimal opaque integrity/ledger data to remain after content removal where required.

Any retained bridge data must be content-free or otherwise limited to what is required for:

- integrity verification
- stale-device suppression
- purge-state continuity

It must not be used as a hidden copy of the purged content.

## 5. Physical purge limits

Physical purge is best-effort across reachable Cryptex-controlled replicas, snapshots and backups.

Cryptex cannot honestly guarantee deletion from:

- inaccessible lost devices
- devices that never reconnect
- provider-controlled historical versions outside Cryptex control
- storage systems Cryptex cannot reach or manage

The product must communicate these limits truthfully.

## 6. Syncthing version history

Where Cryptex controls Syncthing version history on reachable peers, purge workflows must eventually scrub relevant Cryptex-controlled version history.

M0 only proves sufficient managed-Syncthing/versioning visibility for feasibility.

M10 productizes the purge/version-history behavior.

## 7. Mass-deletion containment

The audit explicitly rejects a transactional guarantee that replication-driven mass deletion can always be prevented.

Any deletion guard is best-effort containment.

Cryptex must not present such a mechanism as mathematically or transactionally preventing every destructive scenario.

## 8. Attachment/reference behavior

Attachment removal must be reference-aware.

M10 includes attachment reference-aware purge behavior.

An attachment blob must not be deleted merely because one reference is purged if other valid references still require it.

The exact reference-count/index implementation belongs to the applicable attachment and purge milestones.

## 9. Canonical-history interaction

Forget and Purge must respect the immutable-event architecture.

Existing canonical events are not casually edited in place to implement lifecycle state.

Forget remains reversible.

Purge may require content-removal behavior plus minimal integrity/ledger continuity as defined by the accepted architecture.

## 10. Replication interaction

Purge state must survive offline/reconnect scenarios semantically.

A stale replica must not be able to override an accepted purge merely because its data is older or arrives later.

Detailed stale-peer suppression belongs to M10 and the device-trust/replication architecture.

## 11. AI boundary

Forgotten content is removed from normal AI context.

Restricted content remains excluded from AI by default independently of Forget/Purge state.

Purge operations are never exposed as autonomous AI tools.

AI must not be able to bypass purge confirmation or trusted daemon-controlled lifecycle rules.

## 12. User intent and confirmation

Purge is destructive and deliberate.

M10 includes purge confirmation behavior.

This document does not select the exact confirmation UI, but the workflow must make destructive intent explicit rather than treating purge like an ordinary reversible edit.

## 13. Backup interaction

Purge is best-effort across Cryptex-controlled reachable backups/snapshots according to what Cryptex can actually manage.

Backup retention/history may complicate physical deletion and must be handled honestly by M10/M11 behavior.

Cryptex must not claim deletion from provider-internal historical copies it cannot control.

## 14. Integrity and recovery

Purge implementation must preserve the ability to validate surviving canonical history without retaining the purged content itself beyond the accepted minimal integrity bridge.

Recovery procedures must not resurrect purged content as active memory when valid purge state is available.

Detailed recovery/purge interaction must be verified by the applicable milestones.

## 15. Failure behavior

Relevant purge failure cases include:

- stale replica reconnect
- unreachable peer
- incomplete reachable-peer cleanup
- version-history cleanup failure
- attachment still referenced elsewhere
- interrupted purge workflow
- recovery from backup containing older pre-purge data

Failures must be explicit and diagnosable.

The product must not silently report complete physical deletion when the evidence does not support that claim.

## 16. M0 relationship

M0 does not implement the full Forget/Purge feature.

P0-1 includes purge complexity as a benchmark criterion when selecting physical event packaging.

P0-4 includes conflict/versioning visibility needed to establish that later purge behavior is implementable.

M0 must not pull M10 product behavior forward merely to satisfy those proofs.

## 17. M10 scope

M10 includes:

- logical forget/restore
- purge confirmation
- purge ledger/hash bridge
- stale-peer suppression
- version/snapshot cleanup
- attachment reference-aware purge

M10 must satisfy relevant failure, recovery, security and cross-platform checks.

## 18. Explicitly unresolved

This document does not select:

- exact purge-ledger format
- exact stale-peer suppression data structure
- exact snapshot/version cleanup algorithm
- exact attachment reference-counting implementation
- exact confirmation UI
- exact retry/backoff behavior for unreachable peers

These belong to M10 implementation evidence and accepted supporting specifications.

## 19. Source basis

This document consolidates existing lifecycle requirements from:

- `docs/00_MASTER_SPEC.md`
- `docs/07_SYNC_AND_CONFLICT_PROTOCOL.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently strengthen Cryptex deletion guarantees beyond what the architecture can prove.
