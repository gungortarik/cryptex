# Cryptex Private Beta / Release Candidate Gate

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

This checklist is the high-level release gate for M17 Private Beta / Release Candidate.

No checkbox may be marked complete without actual evidence from the applicable milestone, CI, runtime verification or release artifact.

## A. Milestone and architecture gates

- [ ] All required prior milestone contracts are complete through M16.
  - Open prerequisite: [M7 controlled Obsidian write-back](../docs/milestones/M7_OBSIDIAN_CONTROLLED_WRITEBACK.md#completion-gate). The owner approved a change in sequence. M7 still requires evidence that the Vault API confines reads and writes to the managed directory on both platforms. On November 23, 2026 the [public API request](https://forum.obsidian.md/t/api-request-physically-contained-vault-operations-for-managed-paths/119512) contained the original post and zero public replies.
- [ ] All required accepted ADRs/addenda are current.
  - Evidence:
- [ ] No unresolved architecture conflict is being hidden by implementation convenience.
  - Evidence:
- [ ] Master Spec reflects any accepted architecture changes discovered during implementation.
  - Evidence:

## B. Recovery confidence

- [x] Encrypted backup creation is implemented and verified.
  - Evidence: [Windows and macOS native backup and installed restore](https://github.com/gungortarik/cryptex/actions/runs/45241783920)
- [x] Backup verification goes beyond successful upload/write.
  - Evidence: [The receiving OS rejects damaged ciphertext and the wrong Kit, then verifies restored signed records](https://github.com/gungortarik/cryptex/actions/runs/45241902996)
- [x] Sandbox restore rehearsal has passed.
  - Evidence: [Reconstruction after the source profile was removed](https://github.com/gungortarik/cryptex/actions/runs/45241783915)
- [x] Recovery Kit can restore required backup decryption capability.
  - Evidence: [Bidirectional recovery with disposable Kits](https://github.com/gungortarik/cryptex/actions/runs/45241902996)
- [x] Recovery Kit can bootstrap the first trusted device after total device loss.
  - Evidence: [First-device approval with offline Recovery Kit and fresh device credentials](../evidence/m11/backup-container.md#first-device-approval-on-a-single-replacement-computer).
- [x] Backup/restore drills required by M16 have passed.
  - Evidence: [M16 closeout](../evidence/m16/hardening.md#final-candidate-review)

## C. Security and privacy

- [x] M16 security review is complete.
  - Evidence: [Technical review, conditional owner approval and personal-use hold](../evidence/m16/hardening.md#final-candidate-review)
- [ ] No custom cryptography has been introduced.
  - Evidence: Release receipt and executable verification use [Windows CryptoAPI and Authenticode](../apps/updater/native/windows.c) and [Apple Security](../apps/updater/native/macos.c) with the [existing CMS decoder](../services/cryptexd/src/m8_owner_channel/native/inventory.c). Include these adapters in final candidate review.
- [ ] Renderer/webview remains least privilege.
  - Evidence: The [M16 webview and IPC review](../evidence/m16/hardening.md#security-and-recovery-review) covers the shipped CSP and explicit invoke list. Neither changed between M16 and `03857d4`.
- [ ] UI code has no arbitrary shell or unrestricted filesystem access.
  - Evidence: The [Desktop invocation review](../evidence/m16/hardening.md#security-and-recovery-review) applies to the same renderer source at `03857d4`. The updater runs as a separate native executable.
- [ ] Secrets/private keys use appropriate OS secure storage.
  - Evidence: The [owner release signer](../evidence/m17/personal-packaging.md#signing-an-owner-approved-update-receipt) used Windows certificate-store and macOS Keychain identities for installed update receipts. Real owner key provisioning and final candidate review remain open.
- [x] Restricted memory remains excluded from AI by default.
  - Evidence: [Signed classification checks on the guarded public reader](../evidence/m12/local-mcp-review.md#security-failure-handling-and-ui).
- [x] Purge/recovery are not autonomous AI tools.
  - Evidence: [Authenticated client discovery and three-tool catalog](../evidence/m12/local-mcp-review.md).
- [ ] Logs/diagnostics do not expose memory bodies, secrets, credentials or recovery material by default.
  - Evidence: [Reviewed health and diagnostic surfaces](../evidence/m16/hardening.md#security-and-recovery-review). All 27 status, connection and capture policy tests passed on Windows at `03857d4`. Final candidate log review remains open.

## D. Cross-platform product verification

- [ ] macOS release candidate builds and launches.
  - Evidence: [Sealed macOS install and live peer startup](../evidence/m17/personal-packaging.md#installed-macos-owner-session) in a disposable account. Final owner candidate pending.
- [ ] Windows release candidate builds and launches.
  - Evidence:
- [ ] Required platform-native behavior/polish is verified on macOS.
  - Evidence:
- [ ] Required platform-native behavior/polish is verified on Windows.
  - Evidence:
- [x] DPI/Retina/theme/accessibility requirements are verified as applicable.
  - Evidence: [M14 display and keyboard evidence](../evidence/m14/platform-closeout-review.md). Coverage uses the native WebView and emulated densities. Hardware monitor changes and assistive-technology sessions remain outside that evidence.
- [x] Packaging smoke has passed on both platforms.
  - Evidence: [M16 installed package smoke on Windows and macOS](https://github.com/gungortarik/cryptex/actions/runs/45241883226). Repeat for the final M17 candidate.

## E. Hardening

- [x] Complete failure matrix required by M16 has passed.
  - Evidence: [M16 failure matrix and reviewed limits](../evidence/m16/hardening.md#failure-coverage)
- [x] Large-scale tests have passed.
  - Evidence: [10,000, 100,000 and 500,000 signed records on both platforms](https://github.com/gungortarik/cryptex/actions/runs/45241784252)
- [x] Sync stress testing has passed.
  - Evidence: [Repeated native transport partitions with verification](https://github.com/gungortarik/cryptex/actions/runs/45241784035)
- [x] Sleep/wake/offline/reconnect soak has passed.
  - Evidence: [Mac and Windows VM kernel-observed sleep receipts](../evidence/m16/hardening.md#offline-and-system-sleep)
- [x] Backup/restore drills have passed.
  - Evidence: [Restore after purging an imported note on both platforms](../evidence/m16/hardening.md#restore-after-purging-an-accepted-cortex-note).
- [ ] No known high-severity blocker remains open for the release scope.
  - Evidence: [Personal-use release hold for managed Syncthing](../evidence/m16/hardening.md#personal-use-release-hold).

## F. Release artifacts

- [ ] Signed/notarized packaging is complete as appropriate for each platform.
  - Evidence: [Disposable local signing on both platforms](https://github.com/gungortarik/cryptex/actions/runs/45253534985). The [first-install verifiers](https://github.com/gungortarik/cryptex/actions/runs/45260525337) and [signed updater](https://github.com/gungortarik/cryptex/actions/runs/45257477174) passed with disposable identities. [Live Mac roles](../evidence/m17/personal-packaging.md#installed-macos-owner-session) were verified with a disposable signer. Owner-controlled signing and final candidate acceptance remain open.
- [ ] Final third-party notices are included.
  - Evidence: [Frontend, native Rust dependencies and embedded component notices](../evidence/m17/personal-packaging.md). Final installed inventory and transport review are pending.
- [ ] Syncthing/other bundled dependency notice obligations are satisfied.
  - Evidence: [Official transport archive contents and linked dependency notices](https://github.com/gungortarik/cryptex/actions/runs/45262814860). Installed-payload verification awaits selection of the corrected transport.
- [ ] Release artifacts are traceable to repository-controlled source and pinned dependencies.
  - Evidence: [Installed provenance matched the clean source revision and build inputs on both platforms](https://github.com/gungortarik/cryptex/actions/runs/45261329297). Repeat against the final release candidate.
- [ ] Final release checklist has been reviewed by the human owner.
  - Evidence:

## G. Runtime truthfulness

- [ ] Health/Doctor surfaces reflect actual observed runtime state.
  - Evidence: [Disposable desktop health, maintenance and native owner controls at `3b0fd56`](https://github.com/gungortarik/cryptex/actions/runs/45263640097). Repeat with the signed final configuration.
- [ ] Configured/Ready/Active/Verified/Attention distinctions are preserved where applicable.
  - Evidence: [Guarded desktop status and privacy review](../evidence/m16/hardening.md#final-candidate-review). The Windows policy tests at `03857d4` rejected remote-durability claims based only on local activity.
- [ ] No major subsystem is represented as working solely because it is configured.
  - Evidence:

## H. Personal data onboarding

- [ ] Backup/recovery confidence is sufficient for real personal Brain onboarding.
  - Evidence:
- [ ] Cortex fresh re-ingestion M15 acceptance/cutover is complete where applicable.
  - Evidence: [M15 accepted the synthetic cutover and restored signed receipts](../evidence/m15/cortex-import-review.md). The owner has not authorized import of the personal export.
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
