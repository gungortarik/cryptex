# M4 Desktop retrieval integration — architecture blocker

**Status:** implementation was stopped at the daemon safety API; the owner has
since accepted the narrow M4 resolution in ADR-0018. Desktop memory retrieval
operation/UI implementation is now the next M4 task.

## M4 requirement

The accepted M4 contract requires the same daemon-owned
`RetrievalSafetyState` to guard current M3 search and Desktop content results.
The verified backend guard and M3 search wrapper are recorded in
[`m4-slice5-retrieval-safety-windows-20261008.md`](m4-slice5-retrieval-safety-windows-20261008.md).

## Repository facts checked

- `services/cryptexd/src/ipc.rs` implements only the M0 `health.ping` operation.
  ADR-0007's accepted M0 addendum deliberately leaves memory APIs absent and
  says future client APIs need their own least-privilege authorization.
- Desktop currently has no memory search/results command or view; its current
  IPC invocation is health-only.
- M1's persisted `InstallIdentity` is not a `DeviceId`.
- M1 `PairingRecord` contains peer ID, pairing state, Brain ID and display
  timestamp; it does not contain or bind a trusted daemon verification key.
- The M4 rebuild/search functions require an explicit trusted-device set and
  `TrustedDaemonKeyResolver`. Production runtime cannot construct those
  inputs from the current accepted persisted records.
- Pairing/trust product flows are assigned to M8. M4 authorization expressly
  forbids implementing M8 opportunistically.

## Security boundary

Do not infer trust from canonical directory names, event-provided public keys,
or the fact that a peer has a `trusted` display state. Do not let Desktop open
the canonical files or projection database directly. Do not add a content IPC
operation without daemon-side runtime identity, verified-key provenance,
bounded query/response framing, and an explicit capability policy. Until those
inputs exist, the only safe product result is a Brain-wide hold.

## Historical decision point (superseded by ADR-0018)

Choose one of these scope paths before adding a Desktop content API:

1. **Keep M4 scope unchanged:** implement a M4-authorized local-only retrieval
   identity/trust source, and fail closed whenever any other device stream is
   present. This requires an explicit contract for durable local `DeviceId`
   ownership and local public-key binding; it must not imply cross-device
   trust.
2. **Defer Desktop retrieval to M8:** revise the M4 completion contract so M4
   completes the tested daemon safety API and M3 guarded search library, while
   the first product Desktop retrieval is implemented after M8 trust/key
   provisioning. This changes an accepted milestone gate and needs owner
   approval.

No production code was changed during the original blocker review. Production
v2 writing remains disabled.

## Follow-up decision

The owner accepted the narrow local-only identity binding and guarded Desktop
search IPC in `docs/adr/ADR-0018-local-device-retrieval-identity.md` on
2026-10-08. Implementation may proceed only within that decision: unknown peer
streams remain fail-closed; M8 trust management and production v2 writing stay
out of scope.
