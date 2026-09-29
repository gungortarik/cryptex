# Cryptex Private Beta / Release Candidate Gate

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

This checklist is the high-level release gate for M17 Private Beta / Release Candidate.

No checkbox may be marked complete without actual evidence from the applicable milestone, CI, runtime verification or release artifact.

## A. Milestone and architecture gates

- [ ] All required prior milestone contracts are complete through M16.
  - Evidence:
- [ ] All required accepted ADRs/addenda are current.
  - Evidence:
- [ ] No unresolved architecture conflict is being hidden by implementation convenience.
  - Evidence:
- [ ] Master Spec reflects any accepted architecture changes discovered during implementation.
  - Evidence:

## B. Recovery confidence

- [ ] Encrypted backup creation is implemented and verified.
  - Evidence:
- [ ] Backup verification goes beyond successful upload/write.
  - Evidence:
- [ ] Sandbox restore rehearsal has passed.
  - Evidence:
- [ ] Recovery Kit can restore required backup decryption capability.
  - Evidence:
- [ ] Recovery Kit can bootstrap the first trusted device after total device loss.
  - Evidence:
- [ ] Backup/restore drills required by M16 have passed.
  - Evidence:

## C. Security and privacy

- [ ] M16 security review is complete.
  - Evidence:
- [ ] No custom cryptography has been introduced.
  - Evidence:
- [ ] Renderer/webview remains least privilege.
  - Evidence:
- [ ] UI code has no arbitrary shell or unrestricted filesystem access.
  - Evidence:
- [ ] Secrets/private keys use appropriate OS secure storage.
  - Evidence:
- [ ] Restricted memory remains excluded from AI by default.
  - Evidence:
- [ ] Purge/recovery are not autonomous AI tools.
  - Evidence:
- [ ] Logs/diagnostics do not expose memory bodies, secrets, credentials or recovery material by default.
  - Evidence:

## D. Cross-platform product verification

- [ ] macOS release candidate builds and launches.
  - Evidence:
- [ ] Windows release candidate builds and launches.
  - Evidence:
- [ ] Required platform-native behavior/polish is verified on macOS.
  - Evidence:
- [ ] Required platform-native behavior/polish is verified on Windows.
  - Evidence:
- [ ] DPI/Retina/theme/accessibility requirements are verified as applicable.
  - Evidence:
- [ ] Packaging smoke has passed on both platforms.
  - Evidence:

## E. Hardening

- [ ] Complete failure matrix required by M16 has passed.
  - Evidence:
- [ ] Large-scale tests have passed.
  - Evidence:
- [ ] Sync stress testing has passed.
  - Evidence:
- [ ] Sleep/wake/offline/reconnect soak has passed.
  - Evidence:
- [ ] Backup/restore drills have passed.
  - Evidence:
- [ ] No known high-severity blocker remains open for the release scope.
  - Evidence:

## F. Release artifacts

- [ ] Signed/notarized packaging is complete as appropriate for each platform.
  - Evidence:
- [ ] Final third-party notices are included.
  - Evidence:
- [ ] Syncthing/other bundled dependency notice obligations are satisfied.
  - Evidence:
- [ ] Release artifacts are traceable to repository-controlled source and pinned dependencies.
  - Evidence:
- [ ] Final release checklist has been reviewed by the human owner.
  - Evidence:

## G. Runtime truthfulness

- [ ] Health/Doctor surfaces reflect actual observed runtime state.
  - Evidence:
- [ ] Configured/Ready/Active/Verified/Attention distinctions are preserved where applicable.
  - Evidence:
- [ ] No major subsystem is represented as working solely because it is configured.
  - Evidence:

## H. Personal data onboarding

- [ ] Backup/recovery confidence is sufficient for real personal Brain onboarding.
  - Evidence:
- [ ] Cortex fresh re-ingestion M15 acceptance/cutover is complete where applicable.
  - Evidence:
- [ ] Real personal memory is not introduced before this gate is authorized.
  - Evidence:

## Final gate

- [ ] M17 milestone acceptance criteria are satisfied.
- [ ] `docs/21_DEFINITION_OF_DONE.md` is satisfied for the release scope.
- [ ] Required CI is green at the release-candidate commit.
- [ ] Human review is complete.
- [ ] Cryptex may be reported ready for Private Beta / Release Candidate.

Until all applicable final-gate items are verified, release status remains NOT READY.

## Source basis

- `docs/20_MILESTONE_ROADMAP.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/27_RELEASE_DISTRIBUTION_AND_SIGNING.md`
- `docs/17_TEST_AND_FAILURE_MATRIX.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
