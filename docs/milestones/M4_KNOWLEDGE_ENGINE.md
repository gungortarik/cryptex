# M4 — Knowledge Engine

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M4 implements deterministic derived knowledge consolidation over canonical event history.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/03_MEMORY_MODEL.md`, `docs/05_ENTITY_FACT_RELATION_MODEL.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M4 includes:

- entity resolution
- aliases
- facts
- relations
- confidence/state
- correction/supersede behavior
- semantic consolidation

## Required invariants

- Materialized knowledge is derived and rebuildable.
- Canonical history remains authoritative.
- Corrections do not edit existing canonical events in place.
- Consolidation must be deterministic for the same accepted history/rules.
- Confidence and classification semantics are preserved.
- Untrusted content cannot silently become protected/security policy.

## Non-goals

M4 does not implement:

- offline Quick Capture product behavior from M5
- Obsidian materialization/write-back from M6/M7
- replication product behavior from M8
- broad Desktop product UI from M13

## Verification

M4 must verify relevant:

- deterministic rebuild from canonical history
- entity/alias consolidation behavior
- fact/relation state
- correction/supersede reconstruction
- confidence/state handling
- failure cases for ambiguous/invalid inputs according to accepted rules

Exact algorithms/thresholds must be documented through accepted implementation evidence rather than silently improvised.

## Completion gate

M4 is complete only when all scope items and relevant tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M5 begin.
