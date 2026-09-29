# 08 — Cryptex Security, Device Trust and Recovery Root

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the fixed Cryptex security, device-trust and disaster-bootstrap boundaries already established by the Master Spec and audit.

It does not select unresolved cryptographic/container implementation details that belong to later milestones or mandatory feature-specific spikes.

## 2. Security principles

Cryptex security is local-first and least privilege.

Core principles are:

- device trust is explicit
- device private keys live in OS secure storage
- canonical events are attributable and tamper-evident
- local at-rest protection relies primarily on FileVault/BitLocker
- no custom cryptography
- renderer/webview is least privilege
- no public Cryptex Internet API is required for V1
- security/system namespaces are daemon-authoritative
- untrusted documents and AI output cannot silently install trust or global security rules

## 3. Device trust

A device is not trusted merely because it can see files or communicate through Syncthing.

Cryptex application-level trust is separate from transport configuration.

M1 establishes device/installation identity, secure-store integration, sequence-allocation foundation, pairing data structures and Recovery Kit data model/public recovery metadata.

M8 productizes pairing, incoming validation, revoke/retire/compromise behavior, sequence cutoff and replication protection status.

## 4. Private-key storage

Device private keys belong in the operating-system secure store.

They must not be committed to the repository, stored in generated Markdown, or exposed to UI code for convenience.

The exact OS secure-store library/API is selected by the applicable implementation milestone and accepted technology/security process.

## 5. At-rest protection

Cryptex relies primarily on full-disk/device encryption for local at-rest protection:

- FileVault on macOS
- BitLocker/Device Encryption on Windows where available

Cryptex must not invent custom full-disk encryption or claim protection that depends on unsupported home-grown cryptography.

Application-level encryption is still required where explicitly specified, such as encrypted backups.

## 6. Event integrity

Canonical events must be attributable and tamper-evident.

M2 productizes hashes, signatures, integrity-chain behavior, quarantine and schema handling.

Integrity/signature failures must not be silently swallowed.

Exact algorithms/libraries are not selected by this document unless later accepted specifications do so.

## 7. Recovery root

Disaster recovery must restore both content and trust.

The Recovery Kit therefore includes standardized recovery secret material for two distinct purposes:

1. decrypting encrypted backups
2. authorizing the first new trusted device after all ordinary trusted devices are lost

The recovery trust-bootstrap capability is part of the security model, not an optional convenience.

## 8. Offline recovery signing secret

The offline recovery signing secret is not routinely stored on normal devices.

Its purpose is disaster bootstrap after loss of trusted devices.

Normal device operation must not require routine access to that offline recovery secret.

The exact Recovery Kit encoding/container belongs to the applicable backup/recovery/security milestone and feature-specific evidence.

## 9. Revocation ordering

Security-sensitive revocation ordering must not rely on wall-clock time.

The audited architecture uses device sequence cutoff semantics to decide post-revocation acceptance.

Clock values are display/timeline data, not security authority.

Exact sequence representation is implemented through the M1/M8 contracts.

## 10. Writer and namespace authority

`cryptexd` is the only supported canonical writer on a device.

Security/system namespaces are daemon-authoritative.

Desktop, CLI, MCP, Obsidian and AI integrations must not gain a bypass that directly mutates trusted canonical state.

## 11. Untrusted input

Imported memory, Markdown, attachment metadata and AI output are untrusted input.

Untrusted content must not silently become:

- trusted device policy
- global security policy
- Recovery Kit configuration
- system-level authority
- unrestricted canonical mutation

Authority tiers/protected namespaces must prevent memory poisoning from becoming security configuration.

## 12. Renderer/webview boundary

The Tauri renderer/webview operates with least privilege.

UI code must not gain arbitrary shell access or unrestricted filesystem access.

Privileged operations flow through narrowly scoped native/runtime boundaries.

Remote navigation and a public Cryptex Internet API are not V1 requirements.

## 13. IPC authorization

Local IPC must not rely on locality alone as authorization.

The accepted runtime architecture requires:

- current-user-only permissions/ACLs
- client role/capability handshake
- protocol version negotiation
- explicit rejection of unauthorized/incompatible clients

The exact IPC library is resolved by M0 P0-3 and the ADR-0007 implementation addendum.

## 14. AI privacy/security boundary

Restricted memory is not exposed to AI by default.

Supported AI integrations use controlled local MCP/adapters rather than direct canonical-folder access.

Purge and recovery operations are never exposed as autonomous AI tools.

AI output remains untrusted until accepted through Cryptex-controlled paths.

## 15. Logging and diagnostics

Logs and diagnostics must not include by default:

- memory bodies
- secrets
- credentials
- private keys
- Recovery Kit material

Diagnostics are local/redacted by default, with opt-in diagnostic bundle creation according to the operations specification.

## 16. Replication trust boundary

Syncthing is transport, not Cryptex trust.

A Syncthing device ID or successful file synchronization is not sufficient proof that canonical data is trusted.

Incoming canonical data must pass applicable Cryptex validation, trust and integrity checks before trusted application use.

## 17. Purge and stale devices

Purge semantically wins over stale replicas.

Minimal content-free ledger/integrity bridge data may remain where required to preserve stale-device suppression and integrity verification.

Full purge behavior belongs to M10.

Security claims must not promise deletion from inaccessible lost devices that Cryptex cannot control.

## 18. Recovery claims

Recovery capability must be verified through restore/reconstruction evidence.

A backup upload or successful file copy is not sufficient proof of recoverability.

Recovery testing must exercise both encrypted-content restoration and required trust/bootstrap behavior under the applicable milestone.

## 19. Cross-platform requirement

macOS and Windows are first-class security targets.

Platform APIs may differ, but product-level guarantees must remain equivalent.

Platform-specific secure-store, ACL, background-runtime and installer behavior belongs behind platform boundaries.

## 20. Milestone boundaries

### M0

M0 proves foundation-sensitive security boundaries for IPC, daemon lifecycle and managed Syncthing integration. It does not implement full device trust/recovery product behavior.

### M1

M1 establishes Brain/device identity, secure-store integration, sequence foundation, pairing data structures and Recovery Kit model/public metadata.

### M2

M2 productizes event integrity/signature behavior.

### M8

M8 productizes device trust and replication workflows.

### M11

M11 productizes encrypted backup and disaster restore, including Recovery Kit first-device bootstrap.

### M16

M16 performs broader security review and hardening.

## 21. Explicitly unresolved

This document does not select:

- exact secure-store library
- exact event-signature/hash library
- exact backup encryption/container format
- exact Recovery Kit serialization/container
- exact pairing UX
- exact revocation UI
- exact key-rotation policy beyond later accepted specifications

These require their milestone/spike/ADR evidence.

## 22. Source basis

This document consolidates existing security/recovery requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently introduce cryptographic design.
