# ADR-0007 candidate comparison

Date: 2026-10-03. Status: evidence summary supporting the accepted, scoped implementation addendum.

## Requirements

P0-3 requires a local transport for Windows and macOS with user-only access,
protocol version negotiation, Desktop/CLI/MCP roles, reconnect after daemon
restart, and malformed/oversized request rejection. The exact production
library and implementation details belong in ADR-0007 before production daemon
APIs are considered complete.

## Evidence-backed candidate comparison

| Candidate | Evidence in this repository | Remaining concerns |
|---|---|---|
| `interprocess` 2.4.4 | Physical Windows named-pipe and macOS Unix-socket process proofs passed. Both exercised role handshakes, protocol/capability rejection, malformed and oversized requests, and a client reconnecting after server restart. Windows used a protected current-token-SID DACL; a disposable Windows CI VM verified a different authenticated account is denied. | A narrow production health endpoint and shared Desktop Rust client are now implemented. The shared client reconnected to real daemons after supervised restart on Windows and macOS (see [`windows-desktop-ipc-reconnect-20261002.md`](../p0-2/windows-desktop-ipc-reconnect-20261002.md) and [`macos-desktop-ipc-reconnect-20261003.md`](../p0-2/macos-desktop-ipc-reconnect-20261003.md)). Actual Tauri renderer invocation remains open. |
| Tokio local sockets | The spike notes this as a cross-platform alternative. No corresponding physical transport evidence is recorded here. | The spike reports that custom Windows security attributes require an unsafe raw-pointer call. It would add an async-runtime choice not exercised by the current synchronous daemon proof. |

## Evidence references

- [`windows-ipc-spike-20261002.md`](windows-ipc-spike-20261002.md): physical Windows and macOS candidate runs.
- [`windows-cross-account-acl-20261002.md`](windows-cross-account-acl-20261002.md): authenticated second-account denial on a disposable Windows VM.
- [`tools/m0-ipc-spike/README.md`](../../../tools/m0-ipc-spike/README.md): candidate rationale, protocol slice and reproduction steps.
- [`docs/30_ADR_INDEX.md`](../../../docs/30_ADR_INDEX.md): original ADR-0007 source remains missing; the scoped implementation addendum is accepted.

## Boundary and next step

The evidence-backed `interprocess` choice and scoped implementation addendum
are accepted. The production health endpoint and shared Tauri Rust client are
integrated. Physical Windows and macOS harness evidence confirms that this
client reconnects to real daemons after supervised restarts. The actual Tauri
renderer-to-command invocation still needs to be recorded. This comparison
does not accept ADR-0012 or complete M0.
