# M6 — Obsidian Materializer

**Status:** PROPOSED  
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

## Required invariants

- The Obsidian vault is derived and rebuildable.
- It is not the replication source of truth.
- Canonical writes remain daemon-controlled.
- Cross-platform path/name behavior must be safe.
- V1 uses Obsidian Graph for full graph visualization rather than requiring a separate full graph engine.

## Non-goals

M6 does not implement managed-note controlled write-back from M7.

## Verification

M6 must verify projection/rebuild behavior, safe naming/link behavior, attachment projection according to accepted spike evidence and relevant macOS/Windows cases.

## Completion gate

M6 is complete only when scope, required P1 evidence, tests/CI, Definition of Done and human review are green.

Only then may M7 begin.
