# 06 — Cryptex Storage Architecture

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the Cryptex storage boundaries that are already fixed by the authoritative architecture.

It distinguishes:

- canonical durable data
- derived and rebuildable data
- local runtime placement
- replication boundaries
- writer authority
- recovery requirements

It records the physical immutable-event packaging accepted in ADR-0013: canonical events are stored as immutable sealed per-device segment/pack files.

## 2. Authority and scope

This document must remain consistent with:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs relevant to storage
3. `docs/20_MILESTONE_ROADMAP.md`
4. `docs/21_DEFINITION_OF_DONE.md`
5. `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
6. `docs/30_ADR_INDEX.md`
7. `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
8. `docs/milestones/M0_FOUNDATION.md`

If implementation evidence conflicts with an accepted architectural decision, implementation must stop and the ADR/specification process must be followed.

## 3. Storage principles

Cryptex storage is local-first.

Every trusted desktop holds sufficient local state to operate offline within the behavior implemented for that milestone.

No server or desktop is the canonical master.

Loss or reinstallation of one computer must not make that device's disposable local database, cache, generated vault or UI state necessary for recovery.

Canonical replicated memory and required attachment data are the durable recovery basis.

## 4. Canonical durable data

Canonical storage includes:

- immutable durable events
- attachment blobs
- immutable brain identity metadata
- minimal purge-ledger / integrity-bridge data required by the architecture

Canonical durable memory must not depend on SQLite, search indexes, generated Markdown or UI state for correctness.

Existing canonical events are never edited in place.

The logical event model is independent from the physical event packaging selected by M0.

## 5. Derived and disposable data

The following are derived and rebuildable:

- SQLite database
- search index
- materialized active facts
- relationship graph cache
- Obsidian vault projection
- thumbnails and previews
- diagnostics cache

Derived data is not a replication source of truth.

M4 follows accepted ADR-0017 and adds its derived knowledge tables to M3's
existing `projection.sqlite`; it does not create a second canonical or
projection database and does not change M3 schema v1. M4 SQLite schema version
and reducer/materialization progress are independent from canonical event
schema versions. M4 tables and metadata are rebuilt from canonical events.

Loss or corruption of derived state must be recoverable by rebuilding it from canonical data according to the applicable milestone contract.

A derived database becoming unavailable must not redefine canonical memory.

## 6. Canonical writer boundary

`cryptexd` is the only supported writer to canonical local Cryptex storage on a device.

The following components must not directly mutate canonical memory files:

- Cryptex Desktop
- CLI clients
- MCP clients
- Obsidian integration
- AI adapters

Those components request canonical changes through the daemon.

AI agents must never gain a direct canonical-file mutation path.

## 7. Local runtime placement

Default production application/runtime roots are:

- macOS: `~/Library/Application Support/Cryptex/`
- Windows: `%LOCALAPPDATA%\Cryptex\`

Development uses the corresponding `Cryptex-Dev` roots.

Runtime data must remain outside the source repository.

Cryptex must not create application/runtime folders in the user's visible home, Desktop or Documents by default.

The app-data path `identity/brain_identity.json` is the M1 local canonical Brain identity record, not disposable runtime-control state, a cache, or a derived projection. It must survive normal daemon stop/restart and application upgrades. M2 event records bind to this identity; M8 replication transfers it unchanged; M11 restore preserves it. These later behaviors are not claimed by M1.

The exact internal directory structure beneath the application-data root is not frozen by this document and must not be invented merely for implementation convenience. The M1 Brain identity path is the scoped exception recorded in ADR-0015 §2.1.1; any later relocation must preserve the canonical identity bytes and `brain_id` through an explicit migration.

## 8. Replication boundary

Cryptex is peer-oriented and offline-first.

Each trusted desktop may create memory while disconnected and reconcile later.

No device is the canonical owner.

Syncthing is the V1 transport candidate and accepted architecture direction.

Its storage role is limited to transporting canonical files/blobs between trusted devices.

Syncthing does not define Cryptex semantics.

The following remain Cryptex responsibilities:

- trust
- schema validation
- signatures and integrity
- knowledge consolidation
- purge semantics
- recovery behavior

A replication transport must not become the application database or semantic authority.

## 9. Physical immutable-event packaging

The logical event model is frozen.

The physical packaging of immutable canonical events is **accepted in ADR-0013**:

- **Accepted:** immutable sealed per-device segment/pack files
- **Rejected:** one immutable file per event (failed required 500k Syncthing practicality; worse local enumeration/rebuild/allocation at that scale)
- **Not selected:** hybrid thresholded segments (A/B evidence is conclusive; hybrid is not the simplest option)

M0 P0-1 compared those three candidates at 10k / 100k / 500k events on macOS and Windows, measuring append latency, directory enumeration, initial Syncthing scan/sync, incremental sync, rebuild speed, disk overhead, crash behavior, purge complexity and corruption blast radius.

`segment_events=1000` was the verified P0-1 **benchmark profile**. It is not a permanent production constant unless a later accepted ADR or M2 specification freezes it. Production seal size, directory structure and container filenames remain M2 implementation parameters within the accepted sealed-segment architecture.

Syncthing remains transport only. P0-1 transport measurements do not complete P0-4 or define Cryptex replication semantics.

## 10. Search and projection relationship

V1 search uses SQLite FTS5 with metadata, relationships and filters.

SQLite search state remains derived.

The Obsidian-compatible vault is also a local derived projection.

Neither SQLite nor the Obsidian vault is canonical replicated memory.

Daemon-controlled content retrieval and Obsidian projection generation must
consult `RetrievalSafetyState`; a Brain-wide hold overrides normal M3 search.
This controls daemon-mediated operations only. It cannot revoke plaintext
files, exports, backups, or copies already exposed to the operating system or
user.

Their exact implementation belongs to their respective later milestones and must not be implemented opportunistically during M0 beyond explicitly required foundation work.

## 11. Purge implications

Forget and purge have different storage semantics.

Forget removes content from normal retrieval and AI context while remaining reversible.

Purge is a deliberate content-removal workflow and semantically wins over stale replicas.

Minimal opaque integrity/ledger data may remain after purge where required to preserve integrity verification and stale-device suppression without retaining the purged content.

Physical purge is best-effort across reachable Cryptex-controlled replicas, snapshots and backups.

Cryptex must not claim deletion from inaccessible lost devices or provider-controlled historical versions it cannot control.

Physical event packaging selected by M0 (ADR-0013: sealed per-device segments) must therefore keep pack-scoped purge complexity explicit in later purge productization.

## 12. Cross-platform storage requirements

macOS and Windows are first-class storage targets.

Canonical storage behavior must not depend on:

- an OS-specific path assumption
- filename case behavior available on only one platform
- platform-specific line endings
- canonical-data symlink behavior

Canonical text uses UTF-8 with LF.

Platform-specific filesystem integration belongs behind platform adapters rather than being scattered through core domain logic.

## 13. Recovery and rebuild contract

Recovery architecture assumes canonical data remains the durable source from which disposable local state can be reconstructed.

A local SQLite database, search index, generated Obsidian vault, cache or UI state must never become required recovery state.

Storage implementation must preserve the ability to:

- validate canonical data
- rebuild derived state
- detect integrity failures rather than silently swallowing them
- reconstruct a device without relying on disposable local projections

Recovery-sensitive behavior requires actual recovery/rebuild verification under the applicable milestone Definition of Done.

## 14. Repository boundary

Source code and runtime memory are separate concerns.

The Git repository contains:

- source code
- tests
- synthetic fixtures
- schemas
- documentation
- repository-controlled development configuration

The repository must not contain:

- real personal brains
- real Cortex memory
- runtime databases
- private logs
- secrets
- Recovery Kits

Golden Brain and other test fixtures must contain synthetic data only.

## 15. M0 storage constraints

M0 may implement only the storage work required by the accepted M0 milestone contract.

For storage, M0 must:

- establish enough foundation to execute the P0-1 benchmark
- benchmark the physical immutable-event packaging alternatives
- retain reproducible benchmark evidence
- evaluate both macOS and Windows
- select the simplest option meeting scale and recovery requirements
- record the accepted result in ADR-0013

**Historical status (2026-10-02):** P0-1 was complete and ADR-0013 was ACCEPTED; M0 was still in progress pending P0-2, P0-3 and P0-4. This dated status is superseded by the M0 closure audit and `checklists/M0_FOUNDATION_GATE.md`, which record M0 complete for its synthetic foundation scope. That closeout does not claim production updater, canonical replication, signing or distribution readiness.

M0 must not opportunistically implement the full M2 immutable event store or M3 SQLite/search system.

## 16. Explicitly unresolved after ADR-0013

The following remain implementation parameters for M2 / later ADRs, not frozen by P0-1:

- production events-per-segment / seal-roll policy (`segment_events=1000` was benchmark-only)
- final production on-disk event directory structure
- final filenames for physical event containers
- other M2 productization details inside the accepted sealed-segment architecture

One-file-per-event and hybrid packaging must not be reintroduced as production architecture without a new ADR.

## 17. Source basis

This document consolidates existing storage requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/30_ADR_INDEX.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/milestones/M0_FOUNDATION.md`

Physical packaging of canonical events is accepted in ADR-0013. This document does not independently change that decision or freeze M2 seal-size/filename parameters.

M4's additive projection tables, migration/rebuild behavior, and retrieval
safety boundary are further specified by accepted
`docs/adr/ADR-0017-structured-event-evolution-and-knowledge-materialization.md`.
