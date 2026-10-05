# Windows managed Syncthing evidence — 2026-10-05

**Result: scoped PASS; P0-4 remains IN PROGRESS.**
Source: `7ff485d`. Windows x64; Node `v24.19.0`; accepted Syncthing `2.1.5`.
This is an external retained native binary test, not a shipped Desktop sidecar.

## Actual final run

Root: `%LOCALAPPDATA%\Cryptex-Dev\p0-4\managed-proof-20261005-2932daa744ad47d5ae7eb9684345c684`.
Retained `managed-syncthing-report.json` SHA-256:
`d31f1cd5173b3118fdf0df8a7ecdec3f02ede36d575213f7f5b9ca2f825fad9b`.
Binary SHA-256: `36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344`.
Version: `2.1.5`, Windows amd64, upstream Go `1.27.1` build.

| Observation | Result |
| --- | --- |
| Two fresh homes and native device identities | Separate; no default visible Sync folder |
| GUI/API and peer transport | Explicit loopback addresses only; discovery/relay/NAT off |
| REST authentication | Incorrect key rejected **403/403**; real keys never printed |
| Folder/device pairing, pause/resume, status | PASS; actual synthetic transfer and idle status |
| Versioning | Prior overwritten synthetic file retained in `.stversions` |
| Same-version restart | Native identity/payload retained; new post-restart file transferred |
| Divergent offline versions | Peer connections paused; both scanners recorded their local changes |
| Conflict visibility | Native conflict file plus selected file retained both exact divergent contents |
| Global user config | Observed hash unchanged |
| Final cleanup | Four owned monitors exit **0**, all PIDs absent; no forced cleanup |

PID/clean-stop times: first peer **14880 / 114 ms**; initial second peer
**31864 / 90 ms**; immediate restarted peer **18080 / 10080 ms**; final restarted
peer **448 / 670 ms**. Shutdown uses public pinned native CLI. Test subprocesses
are hidden; no desktop interaction or startup registration occurs.

## Historical failure and correction

Initial runs used an invented **8-second** monitor shutdown budget and reported
`SYNCTHING_STOP_TIMEOUT`; owned trees were forcibly cleaned and those runs are
not PASS. Retained examples:
`managed-proof-20261005-766c4f3705244c27a840f7adca6f73f9` and
`managed-proof-20261005-8ccb816d3ac44bf8921b9ca44fc4d35b`.
An empty two-profile diagnostic closed normally; the immediate retained-profile
restart consistently needed about **10.1 seconds**.

The exact pinned [protocol source](https://github.com/syncthing/syncthing/blob/v2.1.5/lib/protocol/protocol.go)
sets its native connection close timeout to 10 seconds; its
[application shutdown](https://github.com/syncthing/syncthing/blob/v2.1.5/lib/syncthing/syncthing.go)
also bounds database close at 10 seconds. The harness now allows **25 seconds**
for these stages and scheduling margin. It still requires exit 0/PID absence,
and explicitly rejects a native database-stop failure even with exit 0.
No assertion was removed and no Syncthing source/timeout was modified.
The final runner includes the immediate restart/stop regression before its
post-restart transfer and conflict checks. Two consecutive completed runs
observed early-stop times **10095 / 10080 ms**; final source run also verifies
both divergent conflict contents. Diagnostic-only edited runner results are
not acceptance evidence.

## Repository checks

Lifecycle/foundation/managed Node **130/130**, storage capture **5/5**, no failures
or skips. Three changed-script syntax checks and diff checks PASS. The prior
integrated workspace Rust **89/89**, format, strict Clippy and locked build
remain recorded in the native interruption evidence; this change is Node only.

## Limits

No Mac managed run, cross-host managed flow, actual version upgrade, packaged
Syncthing distribution, production API-secret secure storage or canonical-data
acceptance is claimed. Native keys remain inside private synthetic profiles.
Only synthetic bodies are transferred; Syncthing is transport, not the canonical
writer. Upstream license/author inventory is retained separately; actual
distribution and dependency notices still require review. No Actions ran.
