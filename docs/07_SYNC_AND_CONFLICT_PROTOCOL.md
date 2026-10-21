# 07 — Cryptex Sync and Conflict Protocol

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the fixed replication and conflict boundaries of Cryptex.

It covers:

- offline-first peer behavior
- Syncthing's transport role
- canonical versus derived replication
- trust and validation boundaries
- conflict/versioning visibility
- revocation ordering constraints
- purge interaction with replication
- M0 managed-Syncthing proof boundaries

It intentionally does not productize the full device-trust and replication controller that belongs to later milestones.

## 2. Authority and scope

This document must remain consistent with:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs relevant to replication
3. `docs/06_STORAGE_ARCHITECTURE.md`
4. `docs/15_PLATFORM_MACOS_WINDOWS.md`
5. `docs/17_TEST_AND_FAILURE_MATRIX.md`
6. `docs/20_MILESTONE_ROADMAP.md`
7. `docs/21_DEFINITION_OF_DONE.md`
8. `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
9. `docs/30_ADR_INDEX.md`
10. `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
11. `docs/milestones/M0_FOUNDATION.md`

If implementation evidence conflicts with an accepted decision, implementation stops and the ADR/specification process must be followed.

## 3. Offline-first rule

Every trusted desktop may create memory while disconnected.

Normal local operation must not require:

- a primary server
- a primary desktop
- an always-online peer
- a cloud database

Reconnection converges through canonical immutable data plus deterministic Cryptex processing.

No device is the canonical master.

An always-on Mac/iMac may be operationally useful for replication or scheduled backup but remains only a peer.

## 4. Replication transport

Syncthing is the V1 transport candidate and accepted architecture direction.

Its responsibility is transporting Cryptex canonical files and blobs between trusted peer devices.

Syncthing does not define Cryptex semantics.

Cryptex remains responsible for:

- device trust
- schema validation
- signatures/integrity
- knowledge consolidation
- purge semantics
- stale-device handling
- recovery behavior
- authorization

Transport success must never be interpreted as semantic acceptance of received data.

## 5. Canonical versus derived replication

Canonical durable data may participate in replication according to the applicable milestone architecture.

Canonical data includes:

- immutable durable events
- attachment blobs
- immutable Brain identity metadata
- required minimal purge/integrity ledger data

Derived state is not the replication source of truth.

Derived/disposable data includes:

- SQLite databases
- FTS/search indexes
- materialized knowledge state
- caches
- generated Obsidian projection
- diagnostics cache

Derived state must be rebuildable locally rather than becoming canonical because it happened to synchronize successfully.

## 6. Immutable-data convergence

The logical Cryptex model assumes immutable canonical events.

Existing canonical events are not edited in place as a conflict-resolution mechanism.

Trusted peers may independently create valid immutable events while offline.

After reconnection, canonical event data can converge by immutable event union, followed by deterministic Cryptex validation/consolidation behavior.

Detailed knowledge-level conflict resolution belongs to the milestone implementing the relevant knowledge semantics.

Syncthing file conflict behavior is not itself Cryptex semantic conflict resolution.

## 7. Writer boundary

`cryptexd` remains the only supported canonical writer on a device.

Replication does not grant Desktop, CLI, MCP, Obsidian or AI clients permission to mutate canonical files directly.

Incoming replicated material must pass through the validation/integrity boundaries required by the applicable milestone before becoming trusted application state.

M0 does not implement the full production incoming-validation pipeline.

## 8. Device trust boundary

Replication is intended for trusted peer devices.

Syncthing pairing alone must not silently become Cryptex application-level trust.

Cryptex trust semantics remain separate from transport configuration.

Full pairing, revoke/retire/compromise behavior and replication protection status belong to M8 Device Trust + Replication.

M0 P0-4 proves managed Syncthing integration only.

## 9. Revocation ordering

Security-sensitive device revocation decisions must not depend on wall-clock ordering.

The audited architecture requires device sequence cutoff semantics for deciding post-revocation acceptance.

Clock values may support display and timeline behavior but are not security authority.

The detailed implementation of device sequence allocation and revocation belongs to later milestones.

M0 must not invent the final revocation protocol.

## 10. Conflict visibility

M0 P0-4 must prove that relevant Syncthing conflict/versioning state is observable.

This proof establishes integration feasibility.

It does not define the later product UX or full semantic resolution workflow.

Conflict visibility must not imply that Syncthing decides:

- which memory fact is semantically correct
- which event should supersede another
- whether a device remains trusted
- whether purged content may return

Those are Cryptex responsibilities.

## 11. Managed Syncthing isolation

Cryptex-managed Syncthing must use isolated Cryptex-controlled configuration/home state.

M0 must prove:

- no collision with user-global Syncthing configuration
- no silent modification of an unrelated user Syncthing setup
- Cryptex can supervise its own pinned Syncthing process/configuration

The exact packaging and supervision mechanism is selected through M0 evidence and recorded in the ADR-0004 implementation addendum.

## 12. Managed process lifecycle

M0 P0-4 must prove managed Syncthing lifecycle on both macOS and Windows.

Required proof includes:

- process/sidecar startup
- process/sidecar shutdown
- status observation
- pause/resume behavior
- isolated configuration
- upgrade-strategy compatibility

This document does not preselect the final process-supervision implementation.

## 13. REST API boundary

M0 must prove authenticated use of the Syncthing REST API for the managed integration.

Required proof includes enough control/observation to demonstrate:

- authenticated API access
- folder/device pairing flow
- status visibility
- pause/resume behavior
- conflict/versioning visibility

The exact internal controller API and product UX belong to later implementation milestones.

Syncthing's REST API must not become a public Cryptex API.

## 14. Pairing proof versus production pairing

M0 may exercise synthetic/test folder and device pairing to prove integration feasibility.

This must not be treated as completed production Cryptex device trust.

Production pairing requires the security/device-trust semantics of the appropriate later milestone.

A Syncthing device ID alone is not sufficient proof of Cryptex trust.

## 15. Cross-platform requirements

Managed replication integration must be proven on both macOS and Windows.

A mechanism that works on only one operating system is insufficient for the M0 P0-4 gate.

Platform-specific process and filesystem details must remain behind platform boundaries where applicable.

Canonical replication semantics must remain platform-independent.

## 16. Physical event packaging dependency

Syncthing performance depends partly on the physical packaging selected by M0 P0-1.

Therefore the storage benchmark explicitly measures:

- initial Syncthing scan/sync
- incremental sync
- realistic event counts
- filesystem overhead
- corruption blast radius
- purge complexity

The physical event packaging is accepted in ADR-0013 (sealed per-device segments). This document still does not treat Syncthing as semantic authority. P0-4 remains incomplete.

P0-1 and P0-4 evidence must remain mutually consistent.

## 17. Purge semantics

Purge semantically wins over stale replicas.

Minimal opaque integrity/ledger data may remain where required to preserve verification and stale-device suppression without retaining purged content.

Physical purge is best-effort across reachable Cryptex-controlled replicas, snapshots and backups.

Cryptex must not claim deletion from inaccessible lost devices or provider-controlled history that Cryptex cannot control.

Full purge behavior belongs to M10.

M0 must not implement the full purge system merely because purge complexity is a P0-1 benchmark criterion.

## 18. Syncthing versioning and purge

Where Cryptex controls Syncthing version history on reachable peers, the audited architecture requires purge workflows eventually to scrub relevant Cryptex-controlled version history.

The exact M10 purge implementation is outside M0 scope.

M0 only needs sufficient managed-Syncthing/versioning visibility to prove that the later requirement is implementable and observable.

## 19. Mass-deletion containment

Cryptex must not claim transactional prevention of all replication-driven mass deletion.

The architecture treats mass-deletion protection as best-effort containment.

Later product behavior may use detection, protection state, versioning or recovery mechanisms as specified by the applicable milestone.

M0 must not invent stronger guarantees than the specification supports.

## 20. Failure behavior

Replication-related failures must be explicit and diagnosable.

Relevant later failure cases include:

- peer unavailable
- network unavailable
- interrupted synchronization
- invalid incoming canonical data
- integrity/signature failure
- stale/revoked peer behavior
- transport conflict
- managed Syncthing process failure
- REST authentication failure
- configuration collision

Errors must not silently convert unvalidated transport data into trusted canonical application state.

Privacy-sensitive logs must remain redacted.

## 21. Runtime health semantics

Sync status must reflect actual runtime state.

Configured transport is not automatically Active or Verified.

Where applicable, Cryptex distinguishes:

- Configured
- Ready
- Active
- Verified
- Attention

M0 needs only enough runtime observation to support the managed integration proof and Doctor foundation.

Broad replication-health UX belongs to later milestones.

## 22. M0 P0-4 acceptance boundary

M0 P0-4 must prove:

- managed Syncthing process lifecycle on macOS
- managed Syncthing process lifecycle on Windows
- isolated Cryptex Syncthing config/home
- REST API authentication
- add/pair folder/device flow
- pause/resume/status
- conflict/versioning visibility
- upgrade-strategy compatibility
- third-party license/notices handling
- no user-global Syncthing configuration collision

The selected packaging/supervision mechanism must be recorded in the ADR-0004 implementation addendum.

## 23. M0 non-goals

M0 P0-4 does not complete:

- production device-trust pairing
- incoming event validation productization
- final offline-convergence implementation
- revoke/retire/compromise workflows
- final sequence-cutoff implementation
- replication protection UX
- full stale-peer suppression
- full purge/version-history cleanup
- full sync stress/soak testing

Those belong to later milestones.

## 24. Later milestone boundaries

### M8 — Device Trust + Replication

M8 productizes:

- pairing UI/flow
- Syncthing transport controller
- incoming validation
- offline convergence
- revoke/retire/compromise
- sequence cutoff
- replication protection status

### M10 — Forget + Purge

M10 productizes:

- purge ledger/hash bridge
- stale-peer suppression
- version/snapshot cleanup
- attachment reference-aware purge

### M16 — Hardening

M16 performs broader:

- sync stress
- sleep/wake/offline/reconnect soak
- large-scale/failure testing

These later responsibilities must not be pulled into M0 opportunistically.

## 25. Supply-chain and notices

Managed Syncthing packaging must account for third-party licensing/notices.

The audit explicitly requires Syncthing licensing obligations to be handled.

The exact release packaging/notices work may be refined in later release milestones, but M0 must prove the managed-integration direction can satisfy those obligations.

## 26. Explicitly unresolved until evidence

This document does not select:

- exact Syncthing packaging mechanism
- exact process-supervision mechanism
- exact isolated config-directory layout
- final replication-controller API
- final pairing UX
- final conflict-resolution UX
- final versioning policy
- final mass-deletion protection mechanism
- final stale-peer suppression implementation

These remain governed by M0 evidence, ADR-0004 and later milestone contracts.

## 27. Source basis

This document consolidates existing replication/sync requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/06_STORAGE_ARCHITECTURE.md`
- `docs/15_PLATFORM_MACOS_WINDOWS.md`
- `docs/17_TEST_AND_FAILURE_MATRIX.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/30_ADR_INDEX.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/milestones/M0_FOUNDATION.md`

It does not independently redesign the replication architecture or move M8/M10/M16 behavior into M0.
