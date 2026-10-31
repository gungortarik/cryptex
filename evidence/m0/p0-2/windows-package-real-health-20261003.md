# Windows package candidate real daemon health — scoped PASS

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

Prior base `b15a85f`; implementation and evidence committed together.
Windows, pinned Node `24.19.0` and existing pinned Rust toolchain.

## Change and local checks

Added a headless `probe-package-health` command to the existing lifecycle tool.
It uses the same Rust `desktop_health_ping` client as Desktop, verifies actual
PID/protocol/accepted health response against the held runtime lock, and bounds
the whole IPC query. Preflight refuses an active or ambiguous endpoint.
Its runtime repository guard uses the compiled manifest path and does not depend
on a runtime environment hint.

The Node adapter starts only an isolated copied daemon with `windowsHide`,
matches ready PID to its owned subprocess, performs actual IPC, requests clean
stop, captures exit, and queries the released lock. On failed health it cleans
the owned candidate before the experiment restores the prior selected pair.
No production Desktop/service/startup or package installer was modified.

- `cargo fmt --all -- --check`: PASS.
- `cargo test -p m0-lifecycle-compare`: **10/10 PASS** (six new health tests).
- Strict Clippy for that package, all targets: PASS, no warnings.
- Local lifecycle tool build: PASS.
- Node daemon adapter tests: **4/4 PASS**; syntax/diff checks PASS.
- Already pinned `serde_json=1.0.151` was added as a direct dependency of the
  test tool. Lockfile changes only that dependency edge; no version upgrade.

## Actual real-health scenarios

`run-package-real-health-proof.mjs` passed **3/3** scenarios using retained
baseline/candidate copies. Report and derived per-case runtimes are retained at
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\package-real-health-0bd438949ac14e5ca06ccabaf11c43c6`.

1. Compatible replacement: baseline PID `32316`, candidate PID `23888`;
   both replied `health.pong`, Desktop role, protocol 1 and matching held lock.
   Each cleanly exited 0 with released lock. Candidate selection committed.
2. Non-executable daemon bytes: spawn failed `EFTYPE`, prior baseline selected
   automatically. Restored baseline PID `19080` passed actual IPC and clean
   exit 0/lock release.
3. Actual protocol mismatch: metadata declares a compatible `2/2` pair but the
   real daemon PID `24680` reports protocol 1. The actual response is rejected
   (`expected protocol 2, observed 1`); candidate clean exit 0 and released lock
   are captured. Selection returns to baseline; restored daemon PID `11116`
   passes protocol-1 IPC and clean exit 0/lock release.

All four positive IPC observations also measured preflight refusing their active
owned endpoint. The negative protocol case is a declared-versus-observed protocol
check through the existing protocol-1 client, not a new protocol-2 implementation.
Automatic selection recovery is in the candidate engine; restored baseline
startup is explicitly driven by the experiment, not an installed native launcher.

Synthetic fixture before/after SHA-256 is identical:
`a21d61aabca6c81f17fff51cec680281df4b0ed45768710e2257793ccd5d52aa`.
No test Desktop, copied daemon or product daemon remained afterward.
No GUI/input/focus operation or GitHub workflow ran.

## Limits and next gate

This replaces injected health with real daemon/IPC lifecycle evidence for the
headless candidate. Desktop bytes are staged but not executed; native successor
launch, installed startup references, signed resources/publisher authenticity,
concurrent transitions, power-loss safety and canonical-data acceptance remain
open. Each probe uses its own fresh derived runtime; this is not an actual
canonical store migration or a full installed Desktop restart.

Mac must execute the focused real-health test plan. The 29 filesystem tests
already passed on both platforms and do not need another repeat for this change.
ADR-0012 remains PROPOSED; production updater orchestration is not accepted.
