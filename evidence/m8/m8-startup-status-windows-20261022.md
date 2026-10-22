# M8 Windows explicit startup and Desktop status

Date: 2026-10-22. **Implementation prepared; runtime verification BLOCKED.**
Isolated `codex/m8-device-trust` worktree remains uncommitted on base
`74bc3b14127962579ae03838bafea9336612a033`. Original dirty checkouts untouched.

## Implemented boundary

- Existing daemon lifecycle hold accepts an explicit complete set of
  `--managed-profile`, `--managed-package`, `--managed-api-port`,
  `--managed-sync-port` options. Default/login startup still selects no transport.
  No profile discovery or durable configuration format is introduced.
- Duplicate/partial options, relative/parent-traversing/Git paths, ambiguous or
  out-of-range ports and identical ports refuse. Non-Windows startup selection
  refuses before runtime setup pending its adapter.
- The daemon owns its instance lock before starting the existing-profile worker.
  It waits for authenticated local readiness before publishing lifecycle ready.
  Normal shutdown joins the worker before releasing the instance lock or writing
  the clean release report. Worker failure/deadline returns an error; no clean
  release is reported. Abrupt daemon-death coverage is still pending.
- A cloneable read-only worker `StatusReader` has no cancellation/launch methods.
  IPC uses a nonblocking snapshot read; busy/poisoned status refuses promptly.
- `transport.status` requires exact current protocol, bounded request ID,
  Desktop role, the existing verified Desktop process decision, exact
  `status.read` capability and no request body. CLI/MCP/health-only contexts do
  not gain access. Existing OS endpoint ACL/peer identity rules remain in place.
- Output contains fixed status labels, attempt/PID and allowlisted failure
  categories only; no paths, keys, transport identities, raw native diagnostics
  or memory bodies. `protected` is always false. Local engine readiness does
  not mean trusted replication. Terminal workers do not advertise stale PIDs.
- This is a backend endpoint, not a new Tauri renderer capability or UI control.
  No trust/peer/folder configuration, remote admission or security writer opens.

## New verification prepared, not claimed PASS

Three library tests cover authorization failures, isolated named-pipe framing
with an injected verified-peer decision, and sanitized failed-worker status.
Two CLI tests cover complete/default selection and malformed/unsafe options.
The actual existing OS peer-credential tests remain unchanged.

The explicit native fixture additionally launches the freshly built daemon
against its ready synthetic profile, waits for ready, checks profile exclusion,
requests normal stop, and checks successful exit, clean lifecycle report and
instance/profile lock release. Retained Child RAII cleans that exact subprocess
on assertion failure; no global/name-based kill is used. CLI-parent startup is
expected to expose health-only IPC, not authenticated Desktop content/status.
The runner passes the exact built daemon through `CRYPTEX_M8_DAEMON_BINARY`.

## Host results

| Gate | Result |
|---|---|
| fmt | PASS |
| strict all-target daemon Clippy | PASS (includes compiling new test code) |
| daemon build | PASS |
| events library | 61 PASS in broad invocation |
| broad test command | BLOCKED at events schema-generator launcher, OS 4551 |
| final daemon library and CLI targets, separate normal invocations | BLOCKED before execution, OS 4551 |
| new native startup fixture | NOT RUN; runtime gates above blocked |
| actual PS5.1 runner-helper regression | 3/3 PASS |

Initial in-development library/CLI test launches were also blocked by Application
Control. No policy, exclusion, executable rename/copy/loader workaround or
Actions was used to bypass this. Prior native PASS does not apply to this new
startup/status candidate. Existing intermittent credential/worker issues remain
open, separately from this execution-policy block.

Proof directory:
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/startup-status-c88a0d4822b844a5bb0f8da3ea08d7ec`.
New source inventory: `m8-startup-status-source-sha256-20261022.txt` (33 files),
including IPC. Prior manifests/sealed packages preserved. Windows development
VM verification bundle is prepared separately; no guest PASS is claimed.

M8 remains open; Mac later; M7 write-back off. ADR-0019 exact protocol/schema/
issuer security review still gates trust activation. No commit/push/merge/CI.
