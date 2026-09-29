# 13 — Cryptex Obsidian Integration

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the accepted Cryptex–Obsidian integration boundary.

Obsidian is a local derived view and controlled editing surface, not the canonical source of truth.

## 2. Derived-vault rule

Each trusted desktop can materialize a complete local Obsidian-compatible vault from canonical Cryptex memory.

The generated vault is:

- local
- derived
- rebuildable
- disposable

It is not the replication source of truth.

## 3. Canonical writer boundary

Obsidian must not directly mutate canonical Cryptex event files.

Canonical changes requested from Obsidian flow through Cryptex-controlled daemon/IPC boundaries.

The generated vault must not become an alternate canonical-writer path.

## 4. Materialization

M6 Obsidian Materializer includes:

- complete local generated vault
- safe names/frontmatter/links
- rebuild
- attachment projection
- Open Vault/Graph integration

Materialization must remain deterministic/rebuildable from canonical state according to the applicable milestone.

## 5. Full graph visualization

V1 uses Obsidian Graph for full graph visualization.

Cryptex may display compact relationship summaries but does not need to reimplement a separate full graph engine for V1.

## 6. Managed-note editing

Manual edits to managed/generated notes do not directly rewrite canonical memory.

They are captured as proposals and become canonical only after daemon validation and, where required, user review.

M7 owns this controlled write-back workflow.

## 7. User-owned content

User-owned Notes/Inbox content may be ingested as events according to the applicable capture/integration behavior.

User-owned content must remain distinguishable from generated/managed projection content where required to avoid write loops and ownership confusion.

## 8. Controlled write-back spike

Before productizing Obsidian write-back, the required P1 spike must prove safe:

- event listening
- `Vault.process`-style changes
- projection-loop suppression
- IPC from the plugin

The exact plugin implementation must follow the spike evidence rather than being guessed early.

## 9. Loop prevention

M7 includes edit detection and loop prevention.

A generated projection update must not be misinterpreted as a new user edit and recursively create canonical changes.

Origin tracking/projection-loop suppression is required by the audited architecture.

Exact markers/metadata fields are not selected by this document.

## 10. Safe Vault API use

The audited architecture requires safe use of the Obsidian Vault API rather than arbitrary direct filesystem manipulation where the plugin controls Obsidian-managed content.

The exact API calls and plugin design are finalized by the relevant P1 spike and M7 implementation.

## 11. Attachment projection

Attachment projection requires a separate P1 benchmark comparing hard-link behavior with copy fallback and cleanup on both operating systems.

Cryptex must not assume hard links are universally available or correct.

Canonical attachment storage remains separate from the derived Obsidian projection.

### M6 / M9 attachment boundary

M6 proves the attachment **projection side** using synthetic attachment fixtures and a stable projection boundary/interface.

M6 may prove:

- projected path/name behavior
- hard-link versus copy fallback behavior
- cleanup/rebuild behavior
- cross-platform projection semantics

M6 must not implement:

- the canonical SHA-256 attachment blob store
- canonical deduplication
- canonical attachment reference tracking
- canonical attachment integrity repair

Those responsibilities belong to M9.

M9 later connects the real canonical attachment subsystem to the projection boundary already proven by M6.

This clarification preserves the roadmap order; it does not move M9 implementation into M6.

## 12. Cross-platform behavior

macOS and Windows are first-class Obsidian integration targets.

Generated names, paths and links must avoid assumptions that only work on one platform.

Canonical data must not depend on symlinks.

Attachment-projection behavior must be verified on both platforms.

## 13. Privacy and authority

The generated vault is not an authority bypass.

AI integrations supported by Cryptex use controlled MCP/adapters and do not require direct access to the generated vault path.

User-granted arbitrary external filesystem access is outside Cryptex classification enforcement and must not be confused with supported Cryptex AI integration.

## 14. Rebuild behavior

Loss/corruption of the generated vault must not damage canonical memory.

Cryptex must be able to rebuild the projection from canonical state.

Relevant later tests must cover projection rebuild and stale/generated-state cleanup.

## 15. M6 boundary

M6 productizes the read/materialization side:

- complete generated vault
- safe names/frontmatter/links
- rebuild
- attachment projection over synthetic fixtures through the stable projection boundary
- Open Vault/Graph integration

M6 does not authorize uncontrolled direct canonical write-back from Obsidian.

M6 also does not implement the canonical attachment/blob backend owned by M9.

## 16. M7 boundary

M7 productizes controlled write-back:

- plugin bridge
- edit detection
- loop prevention
- proposal/review/apply/reject

M7 must use the required P1 spike evidence.

## 17. M0 boundary

M0 does not implement Obsidian materialization or write-back product behavior.

The monorepo may contain the plugin package skeleton, but feature implementation belongs to M6/M7.

## 18. Explicitly unresolved

This document does not select:

- final generated vault directory structure
- final note filename algorithm
- final frontmatter schema
- final origin-tracking marker format
- exact plugin event-listening strategy
- exact hard-link/copy threshold/fallback rules
- exact proposal/review UI

These require P1 evidence and M6/M7 implementation decisions.

## 19. Source basis

This document consolidates existing Obsidian requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not make the Obsidian vault canonical.
