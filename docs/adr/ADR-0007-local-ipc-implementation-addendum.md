# ADR-0007 — Local IPC implementation addendum

**Status:** ACCEPTED FOR M0 IMPLEMENTATION  
**Decision date:** 2026-10-03  
**Scope:** Exact implementation choice for the existing local IPC decision. The original ADR source record remains unavailable; this addendum records only the implementation details established by the M0 P0-3 evidence and user-approved selection.

## Context

Cryptex requires current-user-only local IPC on macOS and Windows, explicit
client role/capability and protocol negotiation, bounded malformed-safe
framing, and reconnection after daemon restart. No production transport may be
chosen before the P0-3 spike and addendum.

## Decision

Use `interprocess` **2.4.4**, pinned exactly, for local sockets across the
first-class macOS and Windows targets. Keep OS-specific endpoint construction
and access controls behind the daemon IPC transport module.

- Windows uses a namespaced named pipe and an explicit protected DACL whose
  owner and sole granted SID are the current user SID. Do not rely on the
  library or OS default named-pipe ACL.
- macOS uses a Unix-domain socket under a short private directory in the
  system temporary directory. Create/validate the directory as owned by the
  current UID with mode `0700`, and reject an unsafe pre-existing directory or
  an overlong socket path.
- Use newline-terminated UTF-8 JSON frames, bounded to 64 KiB. Protocol version,
  request ID, client role, capability, and operation are explicit. Reject
  incompatible versions, unknown roles, unauthorized capabilities, malformed
  frames, and oversized frames with structured errors.
- The initial P0-3 operation remains read-only `health.ping` under
  `status.read`. It returns `health.pong` and the serving daemon PID. No memory
  body, mutation, purge, recovery, or arbitrary tool operation is authorized
  by this addendum.
- Desktop reconnects by opening a fresh local connection and repeating the
  handshake after connection failure; it must validate response protocol,
  request ID, and operation before using the response. Reconnect must not
  silently widen capabilities.

## Alternatives considered

- **Tokio local sockets:** supports both platforms, but the spike found custom
  Windows security attributes require an unsafe raw-pointer call. The current
  synchronous daemon proof would also need an async-runtime integration.
- **Platform-specific transport libraries:** not selected because the
  cross-platform candidate has passed physical transport/reconnect proofs on
  both operating systems with a single API surface.

## Evidence

- Windows and macOS process-level roles, protocol/capability rejection,
  malformed/oversized handling, and reconnect after server restart passed in
  [`windows-ipc-spike-20261002.md`](../../evidence/m0/p0-3/windows-ipc-spike-20261002.md).
- The protected Windows pipe denied a real authenticated second account on a
  disposable Windows VM in
  [`windows-cross-account-acl-20261002.md`](../../evidence/m0/p0-3/windows-cross-account-acl-20261002.md).
- The candidate comparison is recorded in
  [`adr-0007-candidate-comparison-20261003.md`](../../evidence/m0/p0-3/adr-0007-candidate-comparison-20261003.md).

## Tradeoffs and security boundaries

The dependency gives one API while access control still requires explicit
platform configuration. The Windows SID lookup and Unix ownership/mode checks
remain security-sensitive and require focused tests and physical proof. The
current handshake authenticates the local OS principal through endpoint ACLs;
role strings are authorization inputs, not cryptographic identity. A process
under the same OS account can claim a role, so each operation must still be
authorized by its granted capability and later feature policy.

The spike proves the narrow health operation, not the full future Desktop/CLI/MCP
API surface. The production Desktop health command now uses the shared IPC
client. Physical Windows and macOS harness evidence records that client
reconnecting to real daemons after supervised restarts; physical Tauri renderer
health/reconnect invocation is separately recorded on both platforms after
clean stop/replacement. See the linked P0-2 evidence. Additional client APIs
remain subject to canonical-writer and least-privilege rules.

## Acceptance

The user selected the evidence-backed candidate on 2026-10-03. This addendum is
accepted for the narrow M0 implementation above. Subsequent physical Windows
and macOS evidence verifies production Desktop health/reconnect behavior within
the scopes stated in the evidence files. This does not accept ADR-0012,
complete M0, authorize M1, or claim that the broader Desktop/CLI/MCP API surface
is complete.
