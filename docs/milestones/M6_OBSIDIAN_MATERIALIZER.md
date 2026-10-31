# M6 — Obsidian Materializer

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M6 productizes the local rebuildable Obsidian projection over canonical Cryptex memory.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/13_OBSIDIAN_INTEGRATION.md`, `docs/15_PLATFORM_MACOS_WINDOWS.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Prerequisite spike

Before attachment projection is finalized, the required P1 attachment-projection spike must benchmark hard-link versus copy fallback behavior and cleanup on macOS and Windows.

## Scope

M6 includes:

- complete local generated vault
- safe names/frontmatter/links
- rebuild
- attachment projection
- Open Vault/Graph integration

## Attachment boundary with M9

M6 proves and productizes the **derived projection side** of attachment handling without implementing the later canonical attachment backend.

For M6:

- attachment projection uses synthetic attachment fixtures and a stable projection boundary/interface
- copy-only projection and cleanup are proven according to the required P1 spike; M6 never hard-links projected files because writes through a hard link mutate the source
- generated attachment output remains derived and rebuildable
- no canonical attachment blob store is introduced
- no SHA-256 blob-store implementation, deduplication, reference tracking or integrity-repair subsystem is implemented

Those canonical attachment responsibilities remain owned by M9.

M9 later connects the real canonical attachment subsystem to the projection boundary already proven by M6.

This boundary preserves milestone order and must not be used to pull M9 implementation into M6.

## Required invariants

- The Obsidian vault is derived and rebuildable.
- It is not the replication source of truth.
- Canonical writes remain daemon-controlled.
- Cross-platform path/name behavior must be safe.
- V1 uses Obsidian Graph for full graph visualization rather than requiring a separate full graph engine.
- M6 attachment fixtures are synthetic and do not create a second canonical attachment store.

## Entity-note and link contract

M6 renders the active consolidated entities, facts, and relations already
produced by the M4 reducer. This is a derived projection only: it does not
change the M4 event model, infer relationships, or create a second relation
store. The exact format and safety rules are defined in
`docs/13_OBSIDIAN_INTEGRATION.md` §5, “M6 deterministic entity links”.

In summary, each active consolidated entity has one note at
`Cryptex/Entities/<representative_entity_id>.md`; the canonical lowercase
representative UUID is the filename identity, never a display label. Facts
appear as content in the subject entity note. Each active explicit relation
adds one directed outgoing Obsidian link from subject to object. No reverse
link is inferred. Only the shared `RetrievalSafetyState` may authorize exposing
these contents; a missing, stale, corrupt, or held state prevents creation or
refresh of managed plaintext. M6 does not add an exception to that gate.

## Link verification requirements

Focused projection tests must prove:

- exact UUID-based entity paths and frontmatter, with duplicate labels remaining distinct
- an explicit relation creates exactly one subject-to-object WikiLink, and no inferred inverse link
- changing a display label changes rendered labels but not the entity path or link target
- pending, withdrawn, conflicted, unresolved, or endpoint-less relations create no links
- unsafe labels, predicates, and fact values cannot escape Markdown/frontmatter or alter link targets
- identical reducer input produces byte-identical LF output regardless of input order or supported OS
- rebuild reproduces the same graph; a held/missing/stale safety state leaves the existing generated tree unchanged
- edits to generated notes do not change canonical events; ownership validation continues to reject modified generated output before replacement

## Non-goals

M6 does not implement managed-note controlled write-back from M7.

M6 also does not implement the canonical attachment/blob subsystem owned by M9.

## Verification

M6 must verify projection/rebuild behavior, safe naming/link behavior, attachment projection according to accepted spike evidence and relevant macOS/Windows cases.

Attachment verification in M6 must demonstrate the projection contract using synthetic fixtures; it must not claim that canonical attachment storage, deduplication, reference tracking or integrity repair are complete.

## Completion gate

M6 is complete only when scope, required P1 evidence, tests/CI, Definition of Done and human review are green.

Only then may M7 begin.
