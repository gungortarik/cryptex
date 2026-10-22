# M7 — Obsidian Controlled Write-back

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M7 productizes controlled Obsidian editing without allowing the generated vault to become a direct canonical writer.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/13_OBSIDIAN_INTEGRATION.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Prerequisite spike

Before write-back productization, the required P1 Obsidian write-back spike must prove safe event listening, `Vault.process`-style changes, projection-loop suppression, plugin IPC, and physical containment of managed-note reads/writes on Windows and macOS, including symlink/reparse-point cases.

Managed-note write-back stays disabled until that containment is proven using a Vault-API-compliant design. Metadata-only preflight is insufficient; direct filesystem operations are not an allowed fallback.

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

Owner-approved scheduling exception (2026-10-19): M8 may begin while M7
remains OPEN / DEFERRED. Managed-note write-back stays disabled until
M7's prerequisite spike is resolved. This exception changes implementation
order only; M7's feature and completion requirements remain in force.
Decision: `evidence/m7/m7-temporary-deferral-and-m8-sequencing-proposal-20261019.md`.
