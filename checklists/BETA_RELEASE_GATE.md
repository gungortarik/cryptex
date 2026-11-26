# Cryptex Private Beta / Release Candidate Gate

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

This checklist is the high-level release gate for M17 Private Beta / Release Candidate.

No checkbox may be marked complete without actual evidence from the applicable milestone, CI, runtime verification or release artifact.

## A. Milestone and architecture gates

- [ ] All required prior milestone contracts are complete through M16.
  - On November 25 the owner kept M7 at the design stage and authorized the remaining M17 work. Its implementation and acceptance remain prerequisites to full M17 closure.
  - Open prerequisite: [M7 controlled Obsidian write-back](../docs/milestones/M7_OBSIDIAN_CONTROLLED_WRITEBACK.md#completion-gate). The owner approved a change in sequence. M7 still requires evidence that the Vault API confines reads and writes to the managed directory on both platforms. On November 23, 2026 the [public API request](https://forum.obsidian.md/t/api-request-physically-contained-vault-operations-for-managed-paths/119512) contained the original post and zero public replies.
- [ ] All required accepted ADRs/addenda are current.
  - [ADR-0004’s two-device addendum](../docs/adr/ADR-0004-managed-syncthing-implementation-addendum.md#m17-owner-package-selection--november-25-2026) records the 2.1.7 pin and the required cold migration from 2.1.5. [ADR-0012](../docs/adr/ADR-0012-background-runtime-implementation-proposal.md#mandatory-production-gates-retained) keeps canonical storage outside package rollback. The installed paired-profile migration remains under test.
- [ ] No unresolved architecture conflict is being hidden by implementation convenience.
  - Evidence:
- [ ] Master Spec reflects any accepted architecture changes discovered during implementation.
  - The [Master Spec](../docs/00_MASTER_SPEC.md#8-replication) assigns transport to Syncthing and peer trust and signatures to Cryptex. The [two-device distribution decision](../docs/27_RELEASE_DISTRIBUTION_AND_SIGNING.md#17-distribution-decisions) selects the signing and delivery method within that boundary. Reconcile any further changes at final review.

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
- [x] No custom cryptography has been introduced.
  - [M17 runtime diff and signed receipt review](../evidence/m17/personal-packaging.md#november-25-owner-package-review). Release receipt and executable verification use [Windows CryptoAPI and Authenticode](../apps/updater/native/windows.c) and [Apple Security](../apps/updater/native/macos.c) with the [existing CMS decoder](../services/cryptexd/src/m8_owner_channel/native/inventory.c).
- [x] Renderer/webview remains least privilege.
  - [1.0.2 renderer, command registration and policy review](../evidence/m17/personal-packaging.md#renderer-review-for-102)
  - Evidence: The [M16 webview and IPC review](../evidence/m16/hardening.md#security-and-recovery-review) covers the shipped CSP and explicit invoke list. The [review at `40dea0f`](../evidence/m17/personal-packaging.md#dependency-and-renderer-review-at-40dea0f) confirmed that the capability files and CSP remain unchanged from M16.
- [x] UI code has no arbitrary shell or unrestricted filesystem access.
  - [1.0.2 release and trial command review](../evidence/m17/personal-packaging.md#renderer-review-for-102)
  - Evidence: The [Desktop invocation review](../evidence/m16/hardening.md#security-and-recovery-review) applies to the same renderer source at `03857d4`. The updater runs as a separate native executable.
- [ ] Secrets/private keys use appropriate OS secure storage.
  - The [Mac update credential trial](../evidence/m17/personal-packaging.md#mac-credential-acceptance-and-rollback) checked both independently enrolled Keychain items and verified the original signed note after two rollbacks and a successful update in a disposable login account.
  - Evidence: The [owner release signer](../evidence/m17/personal-packaging.md#signing-an-owner-approved-update-receipt) used Windows certificate-store and macOS Keychain identities for installed update receipts. Real owner key provisioning and final candidate review remain open.
- [x] Restricted memory remains excluded from AI by default.
  - Evidence: [Signed classification checks on the guarded public reader](../evidence/m12/local-mcp-review.md#security-failure-handling-and-ui).
- [x] Purge/recovery are not autonomous AI tools.
  - Evidence: [Authenticated client discovery and three-tool catalog](../evidence/m12/local-mcp-review.md).
- [ ] Logs/diagnostics do not expose memory bodies, secrets, credentials or recovery material by default.
  - [Installed migration output and receipt review](../evidence/m17/personal-packaging.md#mac-update-admission-and-keychain-access) covers the Windows and Mac updater transactions and the failed hosted Mac pairing trial. Repeat with the final signed packages.
  - Evidence: [Reviewed health and diagnostic surfaces](../evidence/m16/hardening.md#security-and-recovery-review). All 27 status, connection and capture policy tests passed on Windows at `03857d4`. Final candidate log review remains open.

## D. Cross-platform product verification

- [x] macOS release candidate builds and launches.
  - The [signed 1.0.2 application at `8beaead`](https://github.com/gungortarik/cryptex/actions/runs/45733242818/job/139620179406) launched on Apple Silicon in a disposable account. Its archived receipt confirms the restored window, refusal of both external service requests, and release of the daemon lock on quit. The installed paired-profile migration and owner Keychain dialog have separate checks.
- [x] Windows release candidate builds and launches.
  - The [signed 1.0.2 NSIS and installed application at `8beaead`](https://github.com/gungortarik/cryptex/actions/runs/45733242818/job/139620178889) passed first-install verification and reopened the updated paired 1.0.1 profile under a standard account. The [owner VM receipt](../evidence/m17/personal-packaging.md#owner-vm-result) records an earlier first installation on the owner's VM.
- [ ] Required platform-native behavior/polish is verified on macOS.
  - The [native shell and display run at `a3711ec`](https://github.com/gungortarik/cryptex/actions/runs/45502020093) exercised the embedded WebView and keyboard navigation. The [sealed Mac runs](https://github.com/gungortarik/cryptex/actions/runs/45501812934) exercised both temporary profile configurations. Final installed configuration review remains open.
- [ ] Required platform-native behavior/polish is verified on Windows.
  - The [standard-user shell and display run at `a3711ec`](https://github.com/gungortarik/cryptex/actions/runs/45502020093) exercised window restoration, keyboard navigation and six theme/scale combinations. The [M14 acceptance record](../evidence/m14/platform-closeout-review.md#final-candidate) covers real sign-in. Final installed configuration review remains open.
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
  - Evidence: [Owner risk acceptance for official Syncthing 2.1.7 on two devices](../docs/proposals/m17-personal-device-signing.md#owner-acceptance-of-syncthing-217). The 1.0.2 installer checks cover the accepted component. Final candidate review remains open.

## F. Release artifacts

- [ ] Signed/notarized packaging is complete as appropriate for each platform.
  - The owner accepted the [two-device delivery and version policy](../docs/proposals/m17-personal-device-signing.md#accepted-delivery-and-version-policy-for-the-two-personal-devices) and assigned first candidate version `1.0.1`.
  - Evidence: [Disposable local signing on both platforms](https://github.com/gungortarik/cryptex/actions/runs/45253534985). The [first-install verifiers](https://github.com/gungortarik/cryptex/actions/runs/45260525337) and [signed updater](https://github.com/gungortarik/cryptex/actions/runs/45257477174) passed with disposable identities. [Live Mac roles](../evidence/m17/personal-packaging.md#installed-macos-owner-session) were verified with a disposable signer. The [Mac packaging command](https://github.com/gungortarik/cryptex/actions/runs/45285474274) and the [Windows packaging command](https://github.com/gungortarik/cryptex/actions/runs/45286355369) passed with disposable signing identities. Owner-supplied first-install receipts cover the temporary accounts on the [Windows VM](../evidence/m17/personal-packaging.md#owner-vm-result) and [Mac](../evidence/m17/personal-packaging.md#owner-mac-result). Final candidate acceptance remains open.
- [ ] Final third-party notices are included.
  - Evidence: [Frontend, native Rust dependencies and embedded component notices](../evidence/m17/personal-packaging.md). The 1.0.2 transport inventory passed in disposable installations. The final release inventory remains open.
- [ ] Syncthing/other bundled dependency notice obligations are satisfied.
  - Evidence: [Official transport archive contents and linked dependency notices](https://github.com/gungortarik/cryptex/actions/runs/45262814860). [The 1.0.2 installed-file checks](../evidence/m17/personal-packaging.md#installed-syncthing-in-owner-candidate-102) compare the executable and notice digests on both platforms. Repeat for the final signed artifacts.
- [ ] Release artifacts are traceable to repository-controlled source and pinned dependencies.
  - Evidence: [Installed provenance matched the clean source revision and build inputs on both platforms](https://github.com/gungortarik/cryptex/actions/runs/45261329297). Repeat against the final release candidate.
- [ ] Final release checklist has been reviewed by the human owner.
  - Evidence:

## G. Runtime truthfulness

- [ ] Health/Doctor surfaces reflect actual observed runtime state.
  - Evidence: [Disposable desktop health, maintenance and native owner controls at `3b0fd56`](https://github.com/gungortarik/cryptex/actions/runs/45263640097). Repeat with the signed final configuration.
- [ ] Configured/Ready/Active/Verified/Attention distinctions are preserved where applicable.
  - The [M17 Windows and macOS readiness run](https://github.com/gungortarik/cryptex/actions/runs/45504560649) at `d08adb5` passed the native profile status and rendered connection checks. The status tests reject a remote durability claim based on local publication or a live transport. The [M16 review](../evidence/m16/hardening.md#final-candidate-review) records the earlier guarded Desktop checks.
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
  - [General CI at `e18540e`](https://github.com/gungortarik/cryptex/actions/runs/45729144487) passed Windows readiness, macOS readiness and Windows cross-account IPC access control, including the shared native runtime path fix and the approved transport. The installed paired-profile upgrade and final revision still need acceptance.
- [ ] Human review is complete.
- [ ] Cryptex may be reported ready for Private Beta / Release Candidate.

Until all applicable final-gate items are verified, release status remains NOT READY.

## Source basis

- `docs/20_MILESTONE_ROADMAP.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/27_RELEASE_DISTRIBUTION_AND_SIGNING.md`
- `docs/17_TEST_AND_FAILURE_MATRIX.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
