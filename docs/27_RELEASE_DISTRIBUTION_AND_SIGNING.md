# 27 — Cryptex Release, Distribution and Signing

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the accepted release/distribution boundaries supported by the current Cryptex roadmap and audit.

It does not prematurely choose final installer, signing-certificate or distribution-channel details that are not yet specified.

## 2. Release-quality principle

Cryptex release artifacts must preserve the same architecture, security and recovery guarantees as development builds.

Packaging convenience must not introduce:

- a required cloud runtime dependency
- a master device
- direct renderer privilege expansion
- runtime data inside the source repository
- unsigned/unreviewed third-party components without required notices

## 3. Platform targets

macOS and Windows are first-class release targets.

Release readiness must account for platform-appropriate:

- packaging
- signing/notarization as appropriate
- installer behavior
- updater coordination
- tray/menu-bar/background integration
- DPI/Retina/theme/accessibility polish

Exact mechanisms may differ while preserving equivalent product guarantees.

## 4. M14 platform polish and packaging smoke

M14 includes:

- macOS menu/window/background behavior
- Windows tray/window/background behavior
- autostart
- updater coordination
- DPI/Retina/theme/accessibility
- packaging smoke

Packaging smoke is not the final release gate, but it must establish that accepted runtime behavior survives packaged execution.

## 5. M17 release-candidate gate

M17 includes:

- signed/notarized packaging as appropriate
- final third-party notices
- release checklist
- real personal Brain onboarding only after backup/recovery confidence

Release-candidate status must not be claimed before these requirements and the broader Definition of Done are satisfied.

## 6. Signing

Release artifacts must use platform-appropriate signing/notarization where required by the accepted release plan and target OS expectations.

The exact certificate/account/notarization workflow is not specified by the current authoritative sources and must be defined before M17 completion.

Private signing credentials must never be committed to the repository.

## 7. Third-party notices

Supply-chain/licensing obligations are product requirements.

The audit specifically requires:

- dependency/security auditing before beta
- third-party notice handling
- Syncthing licensing/notices handling for managed packaging

Final third-party notices are part of M17.

## 8. Syncthing distribution

M0 P0-4 proves that Cryptex can manage a pinned Syncthing binary and that the packaging direction can satisfy applicable third-party notice obligations.

The exact production bundling/supervision mechanism is recorded in the ADR-0004 implementation addendum.

Release packaging must not silently collide with a user's unrelated global Syncthing configuration.

## 9. Reproducibility and pinning

Repository toolchains and dependencies are pinned through accepted baseline files and lockfiles.

Release builds must be traceable to repository-controlled source and accepted dependency/toolchain state.

Dependencies must not be opportunistically upgraded during release packaging without the appropriate validation process.

## 10. Update safety

Release/update packaging must preserve the background runtime/update architecture selected through M0 evidence and ADR-0012.

Desktop and daemon updates must remain compatible and must not damage canonical data.

M14 productizes updater coordination after M0 resolves the architecture.

## 11. Runtime data separation

Installed application/runtime data remains outside the source repository and follows the accepted platform runtime roots.

Packaging must not create visible Cryptex application-data folders in the user's home/Desktop/Documents by default.

Installer/uninstaller behavior must not treat canonical personal data as disposable application binaries.

Exact uninstall/data-retention UX is not selected by this document.

## 12. Security boundaries in packaged builds

Packaging must preserve:

- least-privilege renderer/webview capabilities
- no arbitrary shell/unrestricted filesystem access from UI code
- OS secure-store use for secrets
- current-user IPC authorization
- daemon-controlled canonical writes
- no public Cryptex Internet API requirement

A packaged build must not weaken these boundaries merely to simplify installation.

## 13. Diagnostics and privacy

Release logs/diagnostics follow the local/redacted policy.

Crash/support tooling must not expose memory bodies, secrets, credentials or Recovery Kit material by default.

Any future telemetry/support upload mechanism would require explicit architectural/privacy approval; it is not introduced by this document.

## 14. Backup/recovery before real onboarding

Real personal Brain onboarding waits until backup/recovery confidence exists.

Release packaging success alone is insufficient to authorize real personal data use.

M11 recovery verification and M16 hardening remain prerequisites to release confidence according to the roadmap.

## 15. Release checklist

M17 owns the final release checklist.

It must include applicable evidence for:

- accepted milestone gates
- required CI
- signed/notarized packaging as appropriate
- third-party notices
- backup/recovery confidence
- security/hardening completion
- macOS/Windows packaging behavior

The exact checklist file/content may be refined before M17, but its gate must remain evidence-based.

## 16. Distribution channels

The current authoritative sources do not select final distribution channels, app stores, auto-update hosting providers or installer formats.

Those choices must not be inferred from convenience.

Any selected channel must preserve offline/local-first operation and security/recovery requirements.

## 17. Explicitly unresolved

This document does not select:

- final macOS distribution channel
- final Windows distribution channel
- exact installer formats
- exact code-signing certificate workflow
- exact notarization automation
- exact auto-update hosting mechanism
- exact release-version numbering policy
- exact uninstall/data-retention UX

These are later release/platform decisions and must be accepted before M17 completion.

## 18. Source basis

This document consolidates release/distribution requirements from:

- `docs/00_MASTER_SPEC.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/31_TECH_STACK_BASELINE.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently choose a distribution/signing strategy that the current sources do not specify.
