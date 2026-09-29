# M16 — Hardening

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M16 performs the product-wide hardening campaign after the planned V1 feature milestones are implemented.

M16 complements earlier milestone-specific verification; it is not permission to defer obvious correctness/security/recovery failures until the end.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md`, `docs/22_RISK_REGISTER.md`, `docs/25_PERFORMANCE_AND_SCALE.md` and this contract.

## Scope

M16 includes:

- complete failure matrix
- security review
- large-scale tests
- sync stress
- backup/restore drills
- sleep/wake/offline/reconnect soak

## Required invariants

- Hardening evidence must exercise actual product behavior, not configuration presence.
- macOS and Windows are first-class targets.
- Recovery claims require restore/reconstruction evidence.
- Security/privacy diagnostics remain redacted and local by default.
- Known architectural invariants remain unchanged unless a new ADR/specification is explicitly accepted.

## Verification

M16 must retain reproducible evidence for the hardening campaigns it performs, including relevant platform/build revision, workload/data scale and result summaries.

Any failure that reveals an architectural problem must be resolved through the accepted ADR/specification process rather than patched around silently.

## Completion gate

M16 is complete only when the full hardening scope and required tests/reviews pass, required CI is green, Definition of Done is satisfied, material risks are resolved/accepted and human review is complete.

Only then may M17 begin.
