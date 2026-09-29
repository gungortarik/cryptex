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
- copy/hard-link fallback behavior and cleanup are proven according to the required P1 spike
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

## Non-goals

M6 does not implement managed-note controlled write-back from M7.

M6 also does not implement the canonical attachment/blob subsystem owned by M9.

## Verification

M6 must verify projection/rebuild behavior, safe naming/link behavior, attachment projection according to accepted spike evidence and relevant macOS/Windows cases.

Attachment verification in M6 must demonstrate the projection contract using synthetic fixtures; it must not claim that canonical attachment storage, deduplication, reference tracking or integrity repair are complete.

## Completion gate

M6 is complete only when scope, required P1 evidence, tests/CI, Definition of Done and human review are green.

Only then may M7 begin.
