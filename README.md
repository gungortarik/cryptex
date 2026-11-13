# Cryptex

Cryptex is a cross-platform, local-first personal memory and knowledge system for macOS and Windows.

Current implementation milestone: **M8 — Device trust and replication (in progress, not complete)**.

**Protocol closeout checkpoint — November 13:** independent reference fixtures
now pin the exact bootstrap/v3 writer bytes, hashes, owner display digests and
mixed-version chain. The Windows crash test now captures the original child
process handle and requires actual termination within one unchanged budget.
The [review record](evidence/m8/m8-protocol-vectors-review-20261113.md) separates
native execution evidence from protocol/product acceptance. The selected
[terminal-retention/recovery implementation](docs/proposals/m8-terminal-journal-retention.md)
now passes full [Mac/Windows CI43518226858](https://github.com/gungortarik/cryptex/actions/runs/43518226858)
and same-source [native CI43518652758](https://github.com/gungortarik/cryptex/actions/runs/43518652758)
on `40da5f7`: seventeen real canonical anchors reopen without duplicate append,
and bilateral transaction IDs reconcile in their correct author namespace.
The [review and receipt](evidence/m8/m8-terminal-journal-review-20261113.md)
record evidence and limits. Next are strict v3/bootstrap schema artifacts and
the remaining protocol/product gates. The owner selected macOS 26+; its
enforcement and ordinary product integration remain open. Full native
[Mac/Windows CI43505609309](https://github.com/gungortarik/cryptex/actions/runs/43505609309)
passed on `e49e318`. No completed VM package is being repeated.

**Latest continuation — November 13:** the default-off Mac transport identity,
pairing and combined-loss owner receipts passed. Two further endpoint/generation
P2 findings are corrected and independently source-reviewed through `3a12bcb`.
Exact-source [Mac/Windows CI43438187261](https://github.com/gungortarik/cryptex/actions/runs/43438187261)
passed eight native socket cases, five HTTP boundary tests per platform and the
existing real Syncthing/Desktop/restart/loss/exported-launcher regressions.
Exact FD witnesses and retained process generations are revalidated before
credential send and observation publication. No product route is activated.

The bounded safety slice is complete; the
[M8 remaining checklist](docs/M8_REMAINING_ACCEPTANCE.md) records two broader
open areas: protocol/Mac product integration decisions, then final integrated
platform/security/human acceptance. See the
[current handoff](docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md) for exact evidence
and continuation. No unchanged owner package rerun is requested. M8 remains open;
older dated notes below are historical checkpoints.

The owner-approved [disabled Mac M8 adapter foundation](evidence/m8/m8-disabled-macos-owner-adapter-20261110.md)
passed exact Windows/Mac CI42960387437 and source review. The remaining
[approval domain/session boundary](evidence/m8/m8-approval-domain-sessions-20261111.md)
is implemented on `449dc3c` (final tests `e0da25e`) and passed
[Windows/Mac CI42963863735](https://github.com/gungortarik/cryptex/actions/runs/42963863735)
and independent source review. All11 new tests passed on both platforms;
complete IPC: Windows80passed, Mac71passed/4preexisting ignored, zero failures.
All Mac M8 families refuse on the legacy Unix path, including
exchange/read/proof; no runtime switch/fallback enables them. Shared wire types,
exchange/approval domain execution and bounded admitted replies are in place.
The [private Mach backend](docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md) now checks
the kernel sender of each message against the supervised process and an expected
signed image. Product activation stays closed pending release image policy,
stable bundle observation and signed Desktop/daemon integration.
The [release-image policy candidate](docs/proposals/m8-macos-release-image-policy.md)
validates publisher and role predicates. Its product factory remains unconfigured
pending publisher provisioning and live process/resource binding.
The [signed inventory and content snapshot](docs/proposals/m8-macos-release-image-policy.md)
now verifies Apple CMS signatures against a pinned certificate and retains
authenticated member bytes in memory. Product trust provisioning and live
Desktop/daemon resource binding remain open.

Latest product connection check: [session lifetime corrections](evidence/m8/m8-connection-session-lifetime-review-20261110.md)
passed exact CI42956476071/source9dd0a40 on Windows and Mac: actual daemon crash/
reconnect, original-stream handle loss, registry lifetime and confirm/cancel response
correlation. Ordinary Unix IPC is unchanged. The [Mac M8 owner adapter](docs/proposals/m8-macos-owner-channel-integration-decision.md)
is approved for disabled development under ADR-0020; Mac mutation gates remain closed.

Latest Mac continuation: [personal-Mac signed bootstrap receipt](evidence/m8/m8-macos-signed-bootstrap-owner-receipt-20261110.md)
passed all19 observations on exact source8e8fabb, with certified self/bundle
eligibility, hostile endpoints/transferred rights and fault/forced cleanup controls.
Scoped CI and source review also passed. This isolated candidate is complete;
product session lifetime, bundle replacement and Unix frame attribution remain
open. No unchanged Mac/VM rerun is needed. Product Unix IPC, owner-operation gates
and minimum-OS policy are unchanged; M8 remains open.

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

**Latest scoped change:** [Desktop readiness/restart correction](evidence/m8/m8-desktop-startup-review-20261110.md)
is wired to the real launch path on source4229585. Local Windows startup/restart
tests passed; native CI42906355843 on867c893 is SUCCESS: startup Windows9/Mac10,
Desktop debug Windows46/Mac39, all zero failures; native lint/format passed. A new Mac custody draft was
withdrawn after review found inheritance and trusted-image-anchor gaps. Mac owner
gates remain closed; this is not a completed custody integration or M8 closure.

**Next continuation point:** the consolidated Mac stage. The previously authorized
[synthetic connection-identity feasibility probe](evidence/m8/m8-macos-xpc-peer-identity-feasibility-20261025.md)
has partial native observations. Exact process/connection binding must be proven
before any production IPC architecture amendment. Complete the Mac profile/publication
adapters and native proof under the accepted contracts; retain fail-closed
operations and disabled normal feeding until acceptance. Then verify real
cross-device offline/restriction propagation and consolidate final review.

The [one-click Mac identity prerequisite package](evidence/m8/m8-macos-identity-preflight-20261109.md)
is verified on Mac CI42687722399/source59411433. The latest receipt from the
user's personal MacBook passed API compile/presence checks and enumerated one
valid file-Keychain signing certificate (the earlier borrowed Mac reported zero).
The [bounded signed XPC observation package](evidence/m8/m8-macos-signed-xpc-observations-20261109.md)
ran on the personal MacBook/sourceee5d417: fixture signing and positive controls
passed, and wrong-ID/ad-hoc messages were rejected. A same-requirement second
process could use the forwarded endpoint; signer-only validation does not establish
the original process instance in this topology. Next investigate parent-launch
connection custody under the existing bounded scope. Full feasibility remains
INCOMPLETE; no production IPC/trust change.

A separate [raw Mach actual-sender experiment](evidence/m8/m8-macos-message-instance-proof-20261110.md)
passed eight native CI cases and eight personal-Mac cases on85ef4b1, including
same-binary intrusion, same-PID exec replacement and queued-message refusal after
sender exit. This verifies the scoped synthetic mechanism on those hosts, not XPC
binding, Desktop-parent custody or production integration. Next review actual
parent custody and the existing accepted Unix-socket adapter before selecting any
transport change. A Mach production channel needs separate ADR acceptance.
Do not repeat completed instance/signing/Windows pairing/API tests.

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
