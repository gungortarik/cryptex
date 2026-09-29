# 26 — Cryptex Versioning, Migrations and Updates

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the accepted compatibility, migration and update-safety boundaries for Cryptex.

It does not select the exact updater orchestration before M0 P0-2 or invent unsupported schema-migration policy.

## 2. Compatibility principle

Updates must preserve canonical data safety.

A newer Desktop, daemon, client or schema must not silently interpret incompatible state as valid.

Where compatibility cannot be established, the system must fail explicitly rather than damaging canonical memory.

## 3. Desktop and daemon coordination

Cryptex Desktop and `cryptexd` are separate runtime components whose update lifecycle must remain compatible.

M0 P0-2 must prove:

- compatible Desktop + daemon replacement
- reconnect after restart/update
- rollback or controlled error behavior
- no canonical-data damage from failed/partial lifecycle transitions

The selected supervision/update architecture is recorded in ADR-0012.

## 4. Protocol version negotiation

Local IPC requires protocol version negotiation.

An incompatible client must not silently issue unsafe requests to a daemon with a different protocol expectation.

M0 P0-3 proves the negotiation behavior.

The exact version-numbering scheme is not selected by this document.

## 5. Schema version behavior

M2 includes explicit schema-version behavior for canonical events.

Unsupported or incompatible event schemas must be handled explicitly through compatible processing, rejection or quarantine according to the accepted M2 contract.

Schema incompatibility must never be silently swallowed.

## 6. Generated schema/binding drift

Rust domain types are the source used to generate required JSON Schema/TypeScript bindings according to the accepted architecture.

CI must detect generated-schema/binding drift once the generation system exists.

A generated binding mismatch is a compatibility failure, not a cosmetic difference.

## 7. Canonical migration rule

Canonical immutable history must not be rewritten casually as an ordinary software migration technique.

Existing canonical events remain immutable except where a separately accepted lifecycle/purge architecture explicitly governs content removal.

Version evolution should preserve the ability to validate/rebuild history rather than silently rewriting old events in place.

The exact long-term schema-evolution mechanism belongs to M2 and later evidence.

## 8. Derived-state migrations

Derived state such as SQLite databases, search indexes and generated projections is disposable/rebuildable.

Where rebuilding is simpler/safer than in-place migration, the architecture permits derived state to be discarded and rebuilt from canonical data.

A derived database migration must never become necessary for canonical recovery.

## 9. Obsidian projection versioning

Generated Obsidian state is derived.

Projection format changes may rebuild/regenerate the local vault according to the accepted M6/M7 behavior rather than treating old generated Markdown as canonical.

User-owned content and controlled write-back proposals must still preserve their applicable ownership/provenance semantics.

## 10. Backup compatibility

Backup/restore must account for software/schema evolution according to the M11 contract.

A backup is not verified if it can be created but cannot be restored by the supported recovery path.

Exact compatibility windows and archive format versioning depend on the backup encryption/container decision and are not selected here.

## 11. Recovery Kit compatibility

Recovery Kit evolution must preserve disaster-bootstrap ability for the supported recovery path.

The offline recovery signing secret and backup-decryption material must not become unusable through an undocumented format change.

Exact Recovery Kit versioning belongs to M1/M11 implementation evidence.

## 12. Syncthing managed-binary updates

M0 P0-4 must prove upgrade-strategy compatibility for the managed/pinned Syncthing integration.

Cryptex must not silently collide with or modify the user's unrelated global Syncthing installation/configuration during upgrades.

The selected packaging/supervision approach is recorded in the ADR-0004 addendum.

## 13. Rollback

Update design must include a controlled rollback/error path where required by M0 P0-2.

Rollback must not depend on modifying accepted canonical history into an older incompatible shape.

The exact binary/package rollback implementation is decided by evidence from the lifecycle/update work.

## 14. Platform update behavior

macOS and Windows are first-class update targets.

M14 Platform Polish + Updates productizes:

- autostart behavior
- updater coordination
- packaging smoke
- relevant platform lifecycle polish

A mechanism that only updates safely on one supported OS is insufficient.

## 15. Source/toolchain version pinning

Repository toolchains/dependencies are pinned according to `docs/31_TECH_STACK_BASELINE.md` and lockfiles.

Dependencies must not be upgraded opportunistically merely because a newer version exists.

Technology/source revalidation belongs to the appropriate acceptance process.

## 16. Cortex transition is not a migration of runtime architecture

Cortex is a read-only legacy source for fresh re-ingestion.

Cryptex does not preserve or migrate Cortex runtime machinery/database architecture as though it were an earlier Cryptex schema version.

M15 performs controlled inventory/import/provenance/cutover into the new Cryptex model.

## 17. Failure behavior

Version/update failures must be explicit and diagnosable.

Relevant scenarios include:

- incompatible client/daemon protocol
- unsupported event schema
- interrupted Desktop/daemon update
- daemon restart/reconnect failure
- incompatible managed Syncthing update
- restore of an unsupported backup version
- generated schema drift

Failures must not be converted into silent data corruption.

## 18. Milestone boundaries

### M0

Proves background runtime/atomic update lifecycle and local IPC protocol negotiation.

### M2

Implements event schema-version behavior.

### M11

Implements backup/restore compatibility required by the selected backup format.

### M14

Productizes platform updater coordination and packaging smoke.

### M16/M17

Hardening/release gates exercise broader update/recovery confidence as applicable.

## 19. Explicitly unresolved

This document does not select:

- exact semantic-versioning policy for every component
- exact event schema version format
- exact compatibility support window
- exact updater framework/orchestration
- exact backup format migration policy
- exact Recovery Kit version encoding
- exact Syncthing upgrade mechanism

These require their applicable milestone/ADR evidence.

## 20. Source basis

This document consolidates existing version/update requirements from:

- `docs/00_MASTER_SPEC.md`
- `docs/11_DAEMON_IPC_RUNTIME.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/31_TECH_STACK_BASELINE.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently choose the unresolved updater architecture.
