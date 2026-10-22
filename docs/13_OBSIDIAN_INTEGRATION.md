# 13 — Cryptex Obsidian Integration

**Status:** ACCEPTED  
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

### M6 deterministic entity links

M6 materializes the already-reduced M4 knowledge view as derived Obsidian
entity notes. This is a projection of existing canonical events and reducer
output; it does not add or infer canonical relationships.

- One generated note represents each active consolidated entity. Its stable
  path is `Cryptex/Entities/<representative_entity_id>.md`, where the ID is the
  reducer's canonical lowercase UUID. Labels are never used in file paths.
- Entity-note frontmatter, in order, is exactly `cryptex_kind: "entity"` then
  `entity_id: "<representative_entity_id>"` between YAML fences. The body has
  an H1 display-label heading, then `## Facts` and `## Relations` sections in
  that order. The heading and link alias use the entity's current display
  label. Labels, predicates, and fact values are escaped as data; they cannot
  alter a path, frontmatter field, or link target.
- Facts are rendered as escaped typed values on their subject entity note:
  `text`, canonical `decimal`, `boolean`, `date`, or `entity_ref` plus its
  canonical UUID. An `entity_ref` fact remains plain content; only an M4
  `relation.assert` produces a WikiLink. Only an active,
  explicit M4 relation creates a graph edge: the subject note contains one
  outgoing list item of the form
  `- <escaped predicate> → [[Cryptex/Entities/<target_representative_id>|<escaped target display label>]]`.
  No inverse edge is inferred.
- A relation is omitted if the M4 reducer does not expose it as active, either
  endpoint has no active consolidated entity note, or the current shared
  `RetrievalSafetyState` does not permit materialization. Pending, withdrawn,
  conflicted, or otherwise unresolved relations never create graph edges.
- Entity ID determines note identity; changing a label updates the heading and
  link alias without changing the path. A changed M4 merge representative may
  change the consolidated note path on a deterministic rebuild, which also
  updates all generated links.
- Entities sort by representative UUID; facts sort by predicate then semantic
  ID; relations sort by predicate then target representative UUID. Filenames,
  link targets, escaping, and LF output are deterministic across Windows and
  macOS. Rebuild from the same verified canonical history reproduces the same
  generated tree.

The generated tree is disposable. Editing an entity note or link does not
write back to canonical memory. If the shared safety state is missing, stale,
corrupt, or held, M6 creates or refreshes no managed plaintext. This gate does
not claim to revoke plaintext already copied outside Cryptex control.

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
- physical containment of every managed-note read/write on Windows and macOS, including symlink/reparse-point cases

The exact plugin implementation must follow the spike evidence rather than being guessed early.

## 9. Loop prevention

M7 includes edit detection and loop prevention.

A generated projection update must not be misinterpreted as a new user edit and recursively create canonical changes.

Origin tracking/projection-loop suppression is required by the audited architecture.

Exact markers/metadata fields are not selected by this document.

## 10. Safe Vault API use

The audited architecture requires safe use of the Obsidian Vault API rather than arbitrary direct filesystem manipulation where the plugin controls Obsidian-managed content.

The exact API calls and plugin design are finalized by the relevant P1 spike and M7 implementation. Managed-note write-back must remain disabled until the P1 spike proves that Vault-API operations stay within the physical managed boundary on both Windows and macOS, including symlink/reparse-point cases. A metadata-only preflight is not proof of physical containment. The plugin must not bypass this requirement by switching to direct filesystem operations.

## 11. Attachment projection

Attachment projection requires a separate P1 benchmark comparing hard-link behavior with copy fallback and cleanup on both operating systems.

Cryptex must not assume hard links are universally available or correct.

Canonical attachment storage remains separate from the derived Obsidian projection.

### M6 P1 outcome

The M6 P1 synthetic spike found that writing through a hard link changes the
source file on both reported Windows and macOS runs. M6 therefore uses
copy-only derived attachment projection, with staged verification and safe
publication. Windows and macOS cross-volume copy fallback passed. This decision does not define or implement the M9
canonical attachment backend. Evidence: `evidence/m6/m6-p1-attachment-projection-decision-20261013.md`.

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

- final origin-tracking marker format for legacy event notes
- final frontmatter/origin-tracking schema for legacy event notes
- exact plugin event-listening strategy
- exact performance-based copy threshold (M6 deliberately uses copies for all projected attachments)
- exact proposal/review UI

The remaining items require their respective M6/M7 implementation decisions.

## 19. Source basis

This document consolidates existing Obsidian requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not make the Obsidian vault canonical.
