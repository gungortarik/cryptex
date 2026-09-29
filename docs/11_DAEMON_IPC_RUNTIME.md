# 11 — Cryptex Daemon, IPC and Runtime

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the fixed Cryptex daemon, local IPC and runtime boundaries already established by the authoritative architecture.

It covers:

- `cryptexd` authority
- canonical writer boundaries
- local client communication requirements
- IPC authorization requirements
- daemon lifecycle requirements
- cross-platform runtime behavior
- update/reconnect safety
- runtime-health semantics

It intentionally does not select the exact IPC library or daemon-supervision implementation that must be resolved by the M0 P0 spikes.

## 2. Authority and scope

This document must remain consistent with:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs relevant to daemon/runtime/IPC
3. `docs/06_STORAGE_ARCHITECTURE.md`
4. `docs/20_MILESTONE_ROADMAP.md`
5. `docs/21_DEFINITION_OF_DONE.md`
6. `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
7. `docs/30_ADR_INDEX.md`
8. `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
9. `docs/milestones/M0_FOUNDATION.md`

If implementation evidence conflicts with an accepted decision, stop and resolve the conflict through the ADR/specification process.

## 3. Daemon authority

`cryptexd` is the only supported writer to canonical local Cryptex storage on a device.

The following components must not directly mutate canonical memory files:

- Cryptex Desktop
- CLI clients
- MCP clients
- Obsidian integration
- AI adapters

Canonical mutations must pass through daemon-controlled validation and authorization boundaries.

Security/system namespaces are daemon-authoritative.

No alternate client-side writer may be introduced for convenience.

## 4. Local-first runtime model

Cryptex is a local-first desktop system.

The daemon and its local clients must not require a public Internet endpoint for core operation.

V1 does not require a public Cryptex API.

Local daemon access must remain available according to the selected platform lifecycle architecture even when network connectivity is unavailable.

No remote cloud service may become required for daemon authority, canonical writes or local IPC.

## 5. Client model

The M0 IPC proof must support the local client roles required by the technical spike:

- Desktop
- CLI
- MCP

Later integrations may build on the authorized daemon boundary but must not bypass it.

Client identity and authorization must include an explicit role/capability handshake.

A connected local process must not gain capabilities merely because it can reach the IPC endpoint.

Detailed later-feature capability policy must remain within the applicable later milestone.

## 6. IPC security boundary

Local IPC must enforce current-user-only access.

The implementation must prove:

- current-user-only permissions or ACL behavior
- client role/capability handshake
- protocol version negotiation
- rejection of incompatible or unauthorized clients
- safe malformed-request handling
- safe oversized-request handling

IPC authorization must not rely solely on the assumption that an endpoint is local.

The exact IPC library and implementation are intentionally unresolved until M0 P0-3 evidence is complete.

## 7. Platform IPC requirements

macOS and Windows are first-class IPC targets.

### macOS

M0 must prove local Unix-domain-socket semantics with:

- current-user-only access
- a short and safe socket path
- daemon restart/reconnect behavior
- protocol negotiation and authorization behavior

The exact library, socket filename and final endpoint placement are not selected by this document.

### Windows

M0 must prove named-pipe semantics with:

- current-user-only security
- appropriate Windows ACL behavior
- daemon restart/reconnect behavior
- protocol negotiation and authorization behavior

The exact library and pipe name are not selected by this document.

Cross-platform domain logic must not be polluted with scattered operating-system-specific IPC assumptions.

Platform-specific endpoint behavior belongs behind an appropriate platform boundary.

## 8. Protocol requirements

The M0 IPC proof must demonstrate enough protocol structure to verify:

- protocol version negotiation
- client role identification
- capability authorization
- compatible request/response behavior
- daemon restart and client reconnect
- malformed input rejection
- oversized input rejection

The following are not fixed by this document:

- exact serialization format
- exact message framing implementation
- exact IPC Rust library
- exact endpoint names
- exact reconnect timing/backoff constants

Those details must not become production assumptions without the required M0 evidence and ADR update.

## 9. Single daemon instance

Cryptex must prevent multiple competing `cryptexd` writer instances for the same local runtime context.

M0 P0-2 must prove:

- one daemon instance
- safe detection of an already-running daemon
- correct client behavior when the daemon already exists
- recovery from daemon crash/restart
- Desktop reconnection

No second daemon may be allowed to create a competing canonical-writer path.

The exact single-instance mechanism is part of the M0 lifecycle decision.

## 10. Background lifecycle requirements

Cryptex Desktop window lifecycle and daemon lifecycle are separate concerns.

M0 P0-2 must prove:

- expected behavior when the main window closes
- daemon availability after window close as designed
- reboot/login startup behavior
- crash detection and restart behavior
- one daemon instance
- Desktop reconnect after daemon restart
- safe coordinated Desktop + daemon update behavior
- rollback/error behavior

The exact supervision architecture is intentionally unresolved until M0 evidence exists.

M0 must compare:

1. native per-user background registration
2. Tauri/tray supervisor spawning the daemon sidecar
3. a justified hybrid only if evidence supports it

The selected approach must be recorded in ADR-0012.

## 11. Platform lifecycle proof

M0 must evaluate lifecycle behavior on both supported platforms.

Potential platform-native mechanisms may differ, but the product-level requirements must remain equivalent.

A mechanism that works only on macOS or only on Windows is insufficient for the M0 gate.

Platform integration must remain isolated behind platform-specific adapters rather than changing core daemon semantics.

## 12. Desktop interaction model

Normal Cryptex UI is not designed around a prominent Start/Stop engine control.

Product behavior includes:

- Restart Engine as an available control
- Stop Engine as an advanced control
- separate controls for Pause Sync, Pause AI Access and Maintenance Mode where applicable

The runtime implementation must support the product lifecycle rather than requiring the user to manually manage daemon processes during normal use.

M0 needs only the foundation/proof required by its milestone.

Broad production UI belongs to later milestones.

## 13. Runtime health semantics

Runtime health must describe actual observed state rather than configuration presence alone.

Cryptex distinguishes:

- Configured
- Ready
- Active
- Verified
- Attention

A configured daemon or integration must not be reported as Active or Verified unless runtime evidence supports that state.

Doctor and Desktop health surfaces must eventually consume real runtime checks rather than optimistic configuration flags.

M0 provides only the Doctor/runtime-health foundation required by the milestone contract.

## 14. Runtime data placement

Production runtime roots are:

- macOS: `~/Library/Application Support/Cryptex/`
- Windows: `%LOCALAPPDATA%\Cryptex\`

Development uses corresponding `Cryptex-Dev` roots.

Runtime state must never be stored inside the source repository.

This document does not choose the final IPC endpoint location inside or outside those runtime roots.

The macOS P0-3 proof must independently satisfy safe socket-path-length requirements.

## 15. Update and compatibility safety

Daemon/Desktop updates must not endanger canonical data.

M0 P0-2 must prove:

- compatible Desktop + daemon replacement
- reconnect after update/restart
- rollback or controlled error behavior
- no canonical-data damage from failed or partial lifecycle transitions

Protocol version negotiation must prevent incompatible clients from silently issuing unsafe requests.

The exact updater/productization mechanism is not finalized by this document.

M0 resolves the architecture and proof.

Later platform/update milestones may productize and polish the accepted mechanism without redefining it silently.

## 16. Failure behavior

Runtime and IPC failures must be explicit and diagnosable.

Implementation must not silently swallow:

- malformed requests
- incompatible protocol versions
- authorization failures
- daemon startup failures
- duplicate-instance failures
- reconnect failures
- integrity-relevant runtime errors

Errors should use typed/structured diagnostics where applicable.

Failure handling must not log memory bodies, secrets, recovery material or credentials by default.

## 17. Renderer and UI privilege boundary

The Tauri renderer/webview must remain least privilege.

UI code must not gain arbitrary shell access or unrestricted filesystem access merely to control the daemon.

Privileged operations must flow through narrowly scoped, authorized native/runtime boundaries.

Local IPC must not become a general-purpose arbitrary command execution interface.

## 18. M0 P0-2 acceptance boundary

M0 P0-2 resolves the background runtime / atomic update lifecycle.

Evidence must cover:

- main-window close behavior
- reboot/login startup
- crash detection/restart
- one daemon instance
- Desktop reconnect
- compatible Desktop + daemon update behavior
- rollback/error path
- canonical-data safety

The accepted mechanism must be recorded in ADR-0012 before M1/M2 authorization as required by the project gate.

## 19. M0 P0-3 acceptance boundary

M0 P0-3 resolves the exact local IPC implementation.

Evidence must cover:

- macOS local socket behavior
- Windows named-pipe behavior
- current-user-only access
- role/capability handshake
- protocol version negotiation
- Desktop/CLI/MCP roles
- daemon restart/reconnect
- malformed request handling
- oversized request handling

The exact implementation/library and relevant security details must be recorded in the ADR-0007 implementation addendum before production daemon APIs are treated as complete.

## 20. M0 scope limits

M0 may implement only the daemon/runtime/IPC work required by the accepted M0 milestone contract.

M0 must not opportunistically implement:

- full M1 device identity/trust behavior
- production M2 event-store APIs
- M8 replication product behavior
- M12 full MCP/AI integration policy
- M13 broad Desktop UI
- M14 final platform polish/update UX

Proof code may establish architecture but must not silently expand milestone scope.

## 21. Explicitly unresolved until M0 evidence

The following remain intentionally open:

- exact daemon supervision mechanism
- exact per-platform background registration implementation
- exact atomic Desktop/daemon update orchestration
- exact IPC Rust library
- exact IPC endpoint naming
- exact macOS socket location
- exact Windows pipe name
- exact serialization/framing implementation
- exact reconnect/backoff constants
- detailed production client-capability matrix beyond the P0 proof

These must not be silently selected before the corresponding M0 evidence and ADR update.

## 22. Source basis

This document consolidates existing daemon/runtime/IPC requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/30_ADR_INDEX.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/milestones/M0_FOUNDATION.md`

It does not independently select the unresolved daemon-supervision or IPC implementation.
