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
| `interprocess` 2.4.4 | Physical Windows named-pipe and macOS Unix-socket process proofs passed. Both exercised role handshakes, protocol/capability rejection, malformed and oversized requests, and a client reconnecting after server restart. Windows used a protected current-token-SID DACL; a disposable Windows CI VM verified a different authenticated account is denied. | It remains an isolated spike: production `cryptexd`, Desktop, CLI and MCP do not use it. The protocol and lifecycle/API behavior still need an implementation addendum and integration proof. |
| Tokio local sockets | The spike notes this as a cross-platform alternative. No corresponding physical transport evidence is recorded here. | The spike reports that custom Windows security attributes require an unsafe raw-pointer call. It would add an async-runtime choice not exercised by the current synchronous daemon proof. |

## Evidence references

- [`windows-ipc-spike-20261002.md`](windows-ipc-spike-20261002.md): physical Windows and macOS candidate runs.
- [`windows-cross-account-acl-20261002.md`](windows-cross-account-acl-20261002.md): authenticated second-account denial on a disposable Windows VM.
- [`tools/m0-ipc-spike/README.md`](../../../tools/m0-ipc-spike/README.md): candidate rationale, protocol slice and reproduction steps.
- [`docs/30_ADR_INDEX.md`](../../../docs/30_ADR_INDEX.md): ADR-0007 remains `SOURCE RECORD MISSING / ADDENDUM REQUIRED`.

## Boundary and next step

This comparison does not accept ADR-0007, choose a production transport, or
claim Desktop reconnect. The Desktop currently observes daemon status using a
short-lived CLI process; reconnecting a spike client is not the same as
reconnecting the actual Desktop. After the transport decision and implementation
addendum are approved, integrate the selected transport in the narrow P0-3
scope, then verify Desktop reconnect after a supervised daemon restart on both
platforms. Do not start production IPC integration before that decision.
