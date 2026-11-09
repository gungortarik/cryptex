# Cryptex

Cryptex is a cross-platform, local-first personal memory and knowledge system for macOS and Windows.

Current implementation milestone: **M8 — Device trust and replication (in progress, not complete)**.

## Implementation status — 2026-11-09

Active branch: `codex/m8-device-trust`. **M8 remains OPEN.** This is an
implementation checkpoint; normal transport/publication activation and M9 are gated.

| Area | Verified evidence | Remaining work |
| --- | --- | --- |
| Pairing and owner actions | Signed bilateral exchange, both durable anchors, duplicate-safe reopen/replay, Windows review/cancel/refusal and journal/IPC regressions | Native authorization/platform parity, final protocol and product-flow review |
| Windows live transfer | Actual host-to-VM Syncthing delivery, controlled process interruption, unchanged identity on restart, one published segment/three events, real IPC busy/search, immutable hashes and zero-new replay | Supported production peer/folder policy and startup feeding remain disabled pending acceptance |
| Offline and restriction integration | Daemon tests: both peers write offline then converge under reversed arrival hints; owner-confirmed retired/compromised cutoff rejects the disconnected peer's later signed event | Actual cross-device offline/reconnect and restriction propagation through supported product integration |
| Windows persistence | Long Unicode-path regressions cover key/identity/journal/counter/writer/seal operations, persisted-seed retry and no-replace invariants; full standard-user CI passed | Scoped native authorization/product review and remaining activation gates |
| macOS | Earlier selected native primitive/credential receipts; general readiness CI passed on source6de8dbe | Managed native profile/worker, pinned publication/containment and exact live Desktop connection identity are incomplete |
| Security and completion | Independent technical source reviews; corrected findings and explicit regression evidence | Final candidate security/product review, required human diff review, platform/protocol acceptance |

The completed Windows native pilot is recorded in
[native transfer evidence](evidence/m8/m8-native-controller-result-20261108.md).
It uses synthetic canonical identities and a test peer/folder policy; it does not
activate normal replication. **Do not repeat the completed pairing/VM packages.**
The owner can keep that VM closed until a concrete new device action is needed.

Local offline/cutoff checks and Windows persistence corrections are recorded in
[integration evidence](evidence/m8/m8-offline-restriction-integration-20261108.md)
and [CI/review evidence](evidence/m8/m8-ci-offline-review-20261108.md).
Current full CI run: [42564404285](https://github.com/gungortarik/cryptex/actions/runs/42564404285)
on `6de8dbea8b6cf0221cd114ed91c77290ce1a6165` — **SUCCESS**: Windows/macOS
readiness and Windows different-user IPC ACL passed. Daemon library: Windows596
passed/0failed/5ignored; Mac464passed/0failed/4ignored. Existing explicit native
fixtures remain outside ordinary CI; installer packaging was skipped by design.

**Next continuation point:** the consolidated Mac stage. The previously authorized
[synthetic connection-identity feasibility probe](evidence/m8/m8-macos-xpc-peer-identity-feasibility-20261025.md)
remains NOT RUN. It must establish exact process/connection binding before any
production IPC architecture amendment. Complete the Mac profile/publication
adapters and native proof under the accepted contracts; retain fail-closed
operations and disabled normal feeding until acceptance. Then verify real
cross-device offline/restriction propagation and consolidate final review.

The [one-click Mac identity prerequisite package](evidence/m8/m8-macos-identity-preflight-20261109.md)
is verified on Mac CI42687722399/source59411433. The latest receipt from the
user's personal MacBook passed API compile/presence checks and enumerated one
valid file-Keychain signing certificate (the earlier borrowed Mac reported zero).
Next select the existing identity locally for the bounded synthetic probe;
private-key use and the full signed matrix remain untested. No trust change
occurred. Do not repeat completed API or Windows VM pairing/transfer checks.

The [M8 completion ledger](checklists/M8_DEVICE_TRUST_REPLICATION_GATE.md) maps
requirements to evidence and open gates. The [current handoff](docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md)
is the operational resume record. Older dated entries are historical receipts;
read its latest status first. ADR-0019's protocol addendum remains PROPOSED overall;
acceptance of the Windows implementation basis does not close M8.

Authoritative project documentation lives under `docs/`. Runtime data must remain outside this source repository.

## Current implementation handoff

Before starting implementation or review work, read [`docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md`](docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md). It records the current milestone, verified evidence, open blockers, and the exact next task. Authoritative specifications and accepted ADRs still take precedence.

## Standalone Desktop build

Use `pnpm --filter @cryptex/desktop tauri:package-smoke` for a standalone probe
or `pnpm --filter @cryptex/desktop tauri:package --bundles app` for the macOS
application bundle. These commands build frontend assets and stage the sidecar.

Rust `--release` alone does not select Tauri's embedded frontend. A release
compiled without `tauri/custom-protocol` now exits with
`RELEASE_FRONTEND_MODE_REJECTED` before Tauri/daemon startup. Direct release
Desktop tests/builds must explicitly enable `--features tauri/custom-protocol`;
they do not replace frontend build or native package preparation. Debug
development through the existing Vite/Tauri commands remains supported.
