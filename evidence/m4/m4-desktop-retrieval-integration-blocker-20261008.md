# M4 Desktop retrieval integration — architecture blocker

**Status:** implementation intentionally stopped at the daemon safety API. No
Desktop memory retrieval operation or UI was added.

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

## Decision needed to resume this integration

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

No production code or accepted ADR was changed to make this choice. Production
v2 writing remains disabled.

