# Cryptex Master Specification

**Status:** Locked baseline  
**Repository:** `gungortarik/cryptex`  
**Platforms:** Windows and macOS  
**Document role:** Canonical product and architecture contract

---

## 1. Purpose

Cryptex is a local-first, encrypted personal vault designed to operate as a durable private data layer across Windows and macOS.

The system must prioritize:

- user ownership of data,
- local availability,
- privacy,
- integrity,
- recoverability,
- deterministic synchronization,
- cross-platform operation,
- independence from any single AI tool or cloud service.

Cryptex must remain useful without requiring a permanently available central server.

---

## 2. Source of Truth

The Git repository is the canonical source of truth for Cryptex architecture, implementation decisions, security rules, and development workflow.

Chat conversations, coding agents, IDE state, and temporary planning notes are not authoritative when they conflict with repository documentation.

Previously accepted architectural decisions remain active until explicitly superseded through the project decision process.

---

## 3. Supported Platforms

Cryptex V1 treats both of the following as first-class platforms:

- Windows
- macOS

Neither platform is considered a secondary port.

Cross-platform compatibility must be considered during architecture, implementation, testing, packaging, storage, IPC, synchronization, and recovery design.

---

## 4. Application Stack

The locked application stack is:

### Desktop application

- Tauri 2
- React
- TypeScript

### Core

- Rust

### JavaScript package management

- Node.js LTS
- pnpm `11.28.0`

The project uses a single private monorepo.

---

## 5. Local-First Architecture

Cryptex uses a local-first architecture.

Each participating device maintains a full usable local replica of the Cryptex data required for normal operation.

The system must not depend on a central cloud database for normal reads or writes.

Network unavailability must not make the local vault unusable.

Synchronization is asynchronous and must tolerate devices being offline for extended periods.

---

## 6. Canonical Writer Model

`cryptexd` is the sole canonical local writer.

Other components must not directly mutate canonical Cryptex storage.

Clients may include:

- desktop UI,
- CLI,
- MCP integration,
- Obsidian integration,
- future approved clients.

Clients communicate with the local Cryptex service through a controlled IPC boundary.

This rule exists to centralize:

- validation,
- authorization,
- event creation,
- sequencing,
- signatures,
- conflict handling,
- indexing,
- purge enforcement,
- recovery behavior.

---

## 7. Event Model

Cryptex uses immutable signed events as the canonical logical history model.

Events must support:

- globally unique identifiers,
- UUIDv7 identifiers where applicable,
- device provenance,
- per-device sequencing,
- integrity verification,
- authenticated origin,
- deterministic replay.

Canonical history is append-oriented.

Existing historical events must not be silently rewritten as part of ordinary application behavior.

The exact physical event packing strategy is subject to a required technical spike.

Possible physical layouts under evaluation include:

- individual event files,
- sealed event segments.

The logical event contract is independent of that storage-layout decision.

---

## 8. Device Identity and Trust

Each Cryptex device has its own cryptographic identity.

The trust model must support:

- explicit device pairing,
- trusted-device membership,
- event provenance,
- device revocation,
- rejection or isolation of data from untrusted devices.

A revoked device must not regain trusted write authority simply by reintroducing stale synchronized files.

---

## 9. Derived Local Index

SQLite with FTS5 may be used as a local derived index.

The SQLite database is not canonical history.

It must be possible to rebuild the index from canonical Cryptex data.

Loss or corruption of the derived index must not mean loss of canonical vault history.

---

## 10. Synchronization

Syncthing is the selected V1 transport for device-to-device synchronization.

Syncthing is transport infrastructure, not the Cryptex consistency model.

Cryptex itself remains responsible for:

- validating synchronized data,
- detecting conflicts,
- determining trusted provenance,
- enforcing deletion and purge semantics,
- rebuilding local derived state.

A file appearing through synchronization does not automatically make it valid canonical state.

---

## 11. Full Local Replicas

The intended V1 topology uses full local replicas rather than a permanently centralized authoritative server.

Windows and macOS devices must be capable of normal local operation using their own replica.

Synchronization convergence must not require a VPS.

---

## 12. Forget and Purge

Cryptex distinguishes between logical forgetting and physical purging.

### Forget

Forget is a logical operation.

Historical information may remain represented in canonical history while the active state records that the object should no longer appear as normal live data.

### Purge

Purge is a stronger physical-removal workflow.

A purge must prevent stale replicas from casually resurrecting data that was intentionally purged.

The purge mechanism requires durable authoritative purge information that outranks stale synchronized copies.

Exact purge mechanics are specified in the storage and security design.

---

## 13. Local Device Protection

Cryptex does not attempt to replace operating-system full-disk encryption.

Supported devices should use platform-native protection:

- BitLocker on Windows,
- FileVault on macOS.

Cryptex application-level cryptography must be designed independently of whether a device currently has full-disk encryption enabled.

---

## 14. Backup

Cryptex must support encrypted backup.

Google Drive may be used as a backup destination, but cloud storage must not become the canonical live database.

Backup data must remain encrypted independently of the cloud provider.

Recovery design must include a recovery-key strategy.

---

## 15. V1 Network Architecture

Cryptex V1 does not require:

- a VPS,
- Tailscale,
- a central always-online application server.

These may be reconsidered in future versions only through an explicit architecture decision.

---

## 16. Security Boundaries

Security-sensitive operations belong inside the trusted Rust/core boundary wherever practical.

The architecture must explicitly protect:

- canonical writes,
- IPC access,
- device identity,
- cryptographic keys,
- event signatures,
- pairing,
- revocation,
- purge state,
- backup encryption,
- recovery procedures.

Filesystem synchronization alone must never be treated as authorization.

---

## 17. Required Technical Spikes

The following questions must be resolved experimentally before dependent implementation is considered production-ready:

### Event physical storage

Validate:

- sealed segments,
- one-file-per-event storage,
- crash behavior,
- atomicity,
- recovery,
- synchronization characteristics.

### Daemon lifecycle and atomic updates

Validate:

- `cryptexd` startup,
- shutdown,
- crash recovery,
- single-instance behavior,
- atomic canonical updates.

### IPC and ACL

Validate secure cross-platform IPC behavior on both:

- Windows,
- macOS.

The spike must include access-control behavior rather than testing connectivity alone.

### Syncthing integration

Validate:

- installation model,
- packaging,
- supervision,
- lifecycle behavior,
- folder configuration,
- conflict behavior,
- recovery after interruption.

No architectural assumption depending on these spikes should be treated as proven before the spike succeeds.

---

## 18. Development Principles

Cryptex development must follow these principles:

- Plan before large or risky changes.
- Preserve existing accepted decisions unless explicitly superseded.
- Keep the repository as the durable source of truth.
- Avoid architecture decisions existing only inside an AI conversation.
- One primary coding agent should make uncontrolled changes to a branch at a time.
- Major architectural changes require review before becoming permanent project history.
- Security and data-integrity shortcuts are not acceptable merely to accelerate implementation.
- Cross-platform behavior must be verified rather than assumed.

---

## 19. Implementation Gate

Application implementation must not proceed beyond the appropriate milestone until the relevant readiness criteria are satisfied.

The detailed gate is maintained in:

`docs/IMPLEMENTATION_READINESS.md`

Required technical spikes must be completed at the milestone defined by the roadmap before architecture depending on them is considered locked.

---

## 20. Related Canonical Documents

This specification is supported by:

- `docs/IMPLEMENTATION_READINESS.md`
- `docs/ROADMAP.md`
- `docs/architecture/ARCHITECTURE.md`
- `docs/architecture/DATA_MODEL.md`
- `docs/architecture/STORAGE.md`
- `docs/architecture/SYNC.md`
- `docs/security/SECURITY.md`
- `docs/security/THREAT_MODEL.md`
- `docs/development/ENVIRONMENT.md`
- `docs/development/WORKFLOW.md`
- `docs/testing/TEST_STRATEGY.md`
- `docs/decisions/`
- `AGENTS.md`

Detailed documents may refine this specification but must not silently contradict it.

If a decision changes, the relevant ADR and affected canonical documents must be updated together.

---

## 21. Current Status

Environment preparation:

- Windows development environment: ready
- macOS development environment: ready
- Private GitHub monorepo: created
- Repository documentation skeleton: created

Application implementation has not started.

The project is currently in pre-implementation documentation and readiness preparation.
