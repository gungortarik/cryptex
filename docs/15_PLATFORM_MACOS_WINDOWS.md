# 15 — Cryptex macOS and Windows Platform Contract

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the cross-platform contract for Cryptex on macOS and Windows.

Both operating systems are first-class product and development targets.

The purpose of this document is to prevent platform-specific implementation convenience from changing canonical behavior, storage semantics, recovery behavior or product architecture.

## 2. Authority and scope

This document must remain consistent with:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs relevant to platform behavior
3. `docs/06_STORAGE_ARCHITECTURE.md`
4. `docs/11_DAEMON_IPC_RUNTIME.md`
5. `docs/20_MILESTONE_ROADMAP.md`
6. `docs/21_DEFINITION_OF_DONE.md`
7. `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
8. `docs/30_ADR_INDEX.md`
9. `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
10. `docs/milestones/M0_FOUNDATION.md`

If a platform constraint appears to require an architectural change, implementation must stop and the conflict must be raised rather than silently encoded.

## 3. First-class platform rule

macOS and Windows are first-class Cryptex targets.

A shared feature that is required on both platforms is not complete merely because it works on one operating system.

Where behavior is platform-sensitive, verification must cover both operating systems unless the applicable milestone explicitly scopes the behavior to one platform.

Cross-platform equivalence does not require identical OS implementation mechanisms.

It requires equivalent product and architecture guarantees.

## 4. Application shell

Cryptex Desktop uses Tauri as the cross-platform application shell.

“Native” in Cryptex means native-feeling platform behavior and conventions, not a requirement to implement the UI using literal SwiftUI on macOS or WinUI on Windows.

Cryptex shares one product design language while adapting platform behavior appropriately.

Platform-specific concerns include, where applicable:

- application menus
- menu-bar or tray behavior
- window lifecycle
- keyboard conventions
- typography
- theme behavior
- high-DPI / Retina behavior
- installer and updater integration

Broad release-level platform polish belongs to the applicable later milestone.

M0 requires only the platform foundation and proofs defined by the M0 contract.

## 5. Runtime roots

Default production application/runtime roots are:

### macOS

`~/Library/Application Support/Cryptex/`

### Windows

`%LOCALAPPDATA%\Cryptex\`

Development uses corresponding `Cryptex-Dev` roots.

Runtime state must not be stored inside the source repository.

Cryptex must not create application/runtime folders in the visible home, Desktop or Documents directories by default.

The internal production directory structure beneath the runtime roots is not selected by this document where another unresolved architecture decision applies.

## 6. Repository versus runtime

Source checkout paths are developer source locations, not Cryptex runtime storage.

The Git repository may contain:

- source code
- documentation
- tests
- synthetic fixtures
- schemas
- development configuration

It must not contain:

- real personal memory
- real Cortex data
- runtime databases
- private logs
- secrets
- Recovery Kits

Development and runtime storage must remain clearly separated on both operating systems.

## 7. Canonical filesystem portability

Canonical behavior must not depend on a filesystem assumption that is valid on only one supported platform.

Implementation must not depend on:

- OS-specific absolute canonical paths
- case sensitivity or case preservation assumptions
- Windows-only or macOS-only filename behavior
- platform-specific line endings
- symlinks for canonical durable data
- path separators embedded into domain semantics

Canonical text format is UTF-8 with LF.

Repository source normalization must preserve this contract.

## 8. Platform adapters

Operating-system integration must be isolated behind platform-specific boundaries or adapters.

Core domain code should not accumulate scattered conditional logic for:

- filesystem locations
- secure storage
- daemon registration
- IPC endpoint behavior
- menu/tray integration
- installer/update behavior
- OS-specific process management

Platform adapters may differ internally while preserving the same domain contract.

## 9. macOS baseline

Cryptex targets a supported/up-to-date macOS environment.

Development prerequisites include:

- Xcode Command Line Tools
- repository-pinned Rust toolchain
- repository-pinned Node/pnpm baseline
- required Tauri toolchain dependencies
- Obsidian where applicable to development/testing
- Syncthing baseline where required for integration testing

FileVault is recommended and is the primary local at-rest protection assumption where enabled.

Homebrew may be used for development convenience but must not become a Cryptex runtime dependency.

## 10. Windows baseline

Cryptex targets supported/up-to-date Windows 11.

Development prerequisites include:

- Git for Windows
- Visual Studio Build Tools / Desktop development with C++
- WebView2 runtime
- repository-pinned Rust toolchain
- repository-pinned Node/pnpm baseline
- required Tauri toolchain dependencies
- Obsidian where applicable to development/testing
- Syncthing baseline where required for integration testing

BitLocker or Device Encryption is the primary local at-rest protection assumption where available.

Disabling Windows Defender must not be a normal development or runtime requirement.

## 11. IPC platform contract

The product-level IPC contract is shared, while endpoint mechanisms are platform-specific.

### macOS

M0 must prove Unix-domain-socket behavior including:

- short safe socket path
- current-user-only access
- reconnect after daemon restart
- protocol negotiation
- authorization behavior

### Windows

M0 must prove named-pipe behavior including:

- current-user-only access
- appropriate ACL behavior
- reconnect after daemon restart
- protocol negotiation
- authorization behavior

The exact IPC library, socket location and pipe name remain unresolved until M0 P0-3 evidence and the ADR-0007 addendum.

## 12. Background runtime platform contract

M0 must prove daemon lifecycle behavior on both platforms.

Required product-level behavior includes:

- correct behavior after Desktop main-window close
- reboot/login startup
- crash detection and restart
- one daemon instance
- Desktop reconnect
- compatible Desktop + daemon update behavior
- rollback/error behavior
- no canonical-data damage

M0 compares:

- native per-user background registration
- Tauri/tray-supervised daemon sidecar
- justified hybrid if evidence requires it

The selected implementation is not fixed by this document.

It must be recorded through ADR-0012.

## 13. Storage benchmark platform contract

M0 P0-1 must evaluate physical event packaging on both macOS and Windows.

Evidence must include the required scale and recovery measurements from `docs/23_REQUIRED_TECHNICAL_SPIKES.md`.

A storage layout that is acceptable on only one platform is insufficient.

The accepted physical layout must be recorded in ADR-0013.

## 14. Managed Syncthing platform contract

M0 P0-4 must prove managed Syncthing behavior on both platforms.

Required proof includes:

- process/sidecar lifecycle
- isolated Cryptex Syncthing config/home
- REST API authentication
- pairing flow
- pause/resume/status
- conflict/versioning visibility
- upgrade-strategy compatibility
- no collision with user-global Syncthing configuration

The exact packaging/supervision implementation remains unresolved until M0 evidence and the ADR-0004 addendum.

## 15. UI behavior

Cryptex must feel deliberately designed for desktop use on both platforms.

Shared application structure does not imply identical OS chrome or interaction details.

Platform-appropriate behavior must be preserved while maintaining one Cryptex product language.

The application must not regress into a generic AI-generated web-dashboard appearance.

Before broad UI work, the required UI shell spike must prove applicable native-feeling menu/tray/titlebar/window, theme and system-font behavior on both platforms.

## 16. Keyboard and interaction conventions

Where shared product shortcuts exist, they should respect platform conventions.

For example, global Cryptex Search uses:

- `Cmd+K` convention on macOS
- `Ctrl+K` convention on Windows

Other keyboard behavior must follow platform expectations unless an explicit Cryptex product rule overrides it.

## 17. Security boundary

Platform-specific convenience must not weaken Cryptex security requirements.

On both operating systems:

- renderer/webview remains least privilege
- UI code does not gain arbitrary shell access
- UI code does not gain unrestricted filesystem access
- secrets belong in OS secure storage
- local IPC requires current-user-only authorization
- canonical writes remain daemon-controlled
- no custom cryptography is introduced

Platform APIs may differ, but the security boundary must remain equivalent.

## 18. Offline behavior

Trusted desktops must remain capable of offline operation according to implemented milestone behavior.

Neither macOS nor Windows may require a central server or another desktop to be available for normal local operation.

No device may become canonical master because of platform role or uptime.

An iMac or other always-on device may be operationally preferred for replication or scheduled backup, but it remains a peer.

## 19. Testing requirements

Relevant platform-sensitive functionality must be tested or explicitly verified on both operating systems.

Depending on the work, this includes:

- build
- launch
- filesystem behavior
- runtime root behavior
- IPC
- daemon lifecycle
- update/reconnect
- Syncthing supervision
- failure handling
- recovery/rebuild
- visual/native-feeling behavior

A passing CI configuration alone is not evidence that runtime behavior works.

Manual proof may be required where automation does not yet establish the claim.

## 20. CI contract

Repository CI uses both macOS and Windows runners.

As implementation targets become available, CI must include relevant:

- formatting/lint
- Rust tests
- TypeScript tests
- schema generation/drift checks
- Golden Brain tests
- builds
- later packaging smoke checks

A platform-specific failure must not be ignored merely because the other platform passes.

## 21. M0 platform gate

M0 must not be considered complete until applicable cross-platform foundation requirements are verified.

This includes:

- blank Cryptex Desktop launches on macOS
- blank Cryptex Desktop launches on Windows
- Rust workspace builds
- local IPC proof works on both platforms
- daemon lifecycle/update proof covers both platforms
- physical storage benchmark covers both platforms
- managed Syncthing proof covers both platforms
- required CI is green
- applicable M0 gate evidence is recorded

## 22. Later platform work

M0 establishes platform architecture proofs.

Later milestones productize and polish accepted behavior.

In particular, later platform work may include:

- finalized macOS menu/window/background behavior
- finalized Windows tray/window/background behavior
- autostart productization
- updater coordination
- DPI / Retina behavior
- theme/accessibility polish
- packaging smoke
- signing/distribution requirements

Later milestones must not silently redefine decisions already accepted through M0 ADRs.

## 23. Explicitly unresolved

This document does not select:

- exact background-supervision implementation
- exact autostart registration mechanism
- exact IPC Rust library
- final macOS socket path
- final Windows named-pipe name
- exact updater orchestration
- final installer/signing strategy
- final tray/menu-bar implementation details
- final application-window chrome choices

These remain governed by their technical spikes, ADRs or later milestone contracts.

## 24. Source basis

This document consolidates existing platform requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/30_ADR_INDEX.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/milestones/M0_FOUNDATION.md`

It does not independently introduce a new platform architecture.
