# M7 — Obsidian Controlled Write-back

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M7 productizes controlled Obsidian editing without allowing the generated vault to become a direct canonical writer.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/13_OBSIDIAN_INTEGRATION.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Prerequisite spike

Before write-back productization, the required P1 Obsidian write-back spike must prove safe event listening, `Vault.process`-style changes, projection-loop suppression and plugin IPC.

## Scope

M7 includes:

- plugin bridge
- edit detection
- loop prevention
- proposal/review/apply/reject workflow

## Required invariants

- Managed-note edits never directly rewrite canonical files.
- Canonical changes flow through daemon validation/authorization.
- Projection updates must not recursively appear as new user edits.
- User-owned content and generated/managed content remain distinguishable where required.

## Non-goals

M7 does not redesign M6 projection semantics, implement general replication or provide unrestricted filesystem/canonical access from the plugin.

## Verification

M7 must verify relevant edit detection, loop suppression, proposal lifecycle, daemon IPC, failure behavior and cross-platform/path cases required by accepted spike evidence.

## Completion gate

M7 is complete only when the prerequisite spike is resolved, scope items and tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M8 begin.
