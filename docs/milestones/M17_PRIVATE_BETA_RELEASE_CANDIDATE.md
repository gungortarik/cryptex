# M17 — Private Beta / Release Candidate

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M17 is the final planned V1 private-beta/release-candidate gate.

It packages and verifies the hardened product for real-world use only after recovery confidence exists.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/21_DEFINITION_OF_DONE.md`, `docs/27_RELEASE_DISTRIBUTION_AND_SIGNING.md`, `checklists/BETA_RELEASE_GATE.md` and this contract.

## Scope

M17 includes:

- signed/notarized packaging as appropriate
- final third-party notices
- release checklist
- real personal Brain onboarding only after backup/recovery confidence

## Required invariants

- macOS and Windows remain first-class release targets.
- Release packaging must preserve local-first/offline architecture.
- No cloud service or device becomes canonical master.
- Required security/privacy boundaries remain intact in packaged builds.
- Backup/recovery must be proven before real personal onboarding.

## Verification

M17 must verify applicable signed/notarized artifacts, packaging behavior, third-party notices, release checklist, required CI, human review and the accepted beta-release gate.

Real personal Brain onboarding is not authorized until the recovery/security/hardening evidence required by prior milestones is complete.

## Completion gate

M17 is complete only when `checklists/BETA_RELEASE_GATE.md` is green, Definition of Done is satisfied, required release artifacts and notices are verified and human release approval is explicit.
