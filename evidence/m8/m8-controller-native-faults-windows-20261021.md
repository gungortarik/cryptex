# M8 Windows native controller fault verification

Date: 2026-10-21. Windows NT 10.0.26300.0 X64.
Worktree: `C:/Users/Tarik/.codex/worktrees/m8-device-trust/cryptex`,
branch `codex/m8-device-trust`, uncommitted on base
`74bc3b14127962579ae03838bafea9336612a033`.

## Scope and mechanism

Extended the existing opt-in synthetic native provisioning fixture with actual
owned-job termination between successful health observations. The fault adapter
wraps the real native driver and production supervision loop, including actual
backoff sleeps. It calls the retained job's bounded force-cleanup API, verifies
`forced=true` and `is_clean=false`, and observes full owned-job exit. It never
looks up or terminates arbitrary PIDs/names. Both fault adapter and entry point
are `cfg(test)` only; no fault command is available in product builds or IPC.

This simulates abrupt whole-job loss. It does not claim an organic Syncthing
crash, root-only death with surviving children, power loss or a long soak.
No personal/global transport profile or canonical memory was used.

## Final native observations

| Scenario | PIDs / result |
|---|---|
| Direct provisioning/reopen/collision retry | 7344 / 18136 / 34136; existing checks PASS |
| Controller requested clean stop twice | 25100 / 37740; both `CleanShutdown` |
| One forced whole-job exit then replacement | 24384 -> 36884; replacement authenticated, requested stop clean |
| Cancel during backoff after forced exit | 26304 only; `Cancelled`, no replacement, no clean-exit claim |
| Exhaust finite restart budget | 35824 / 31472 / 26536 / 39620; `RestartLimit`, attempt 4, no fifth start |

One-crash recovery including final shutdown took 2907ms; repeated-exit
exhaustion took 10319ms. Assertions require at least the actual 1-second retry
and 7-second accumulated backoff, respectively; these are not exact timing/SLA
claims. Every exhausted attempt had a successful health observation, proving
that health does not reset the invocation's retry budget.

After every scenario, native in-use exclusion was released with no unresolved
cleanup. Reopening retained the same native identity and existing OS credential;
no replacement key/identity was generated. Exact synthetic credential deletion
then verified Missing, and ready-profile preparation refused the missing secret.
Existing wrong-key 403, foreign API collision before HTTP bytes and loopback
isolation checks remained PASS. Final Syncthing process count: 0.

The receipt now uses `direct_and_requested_stop_cycles_clean`, replacing the
older ambiguous `all_verified_starts_clean` field: the forced exits are
explicitly unclean. Receipt scope is
`synthetic-pinned-provision-controller-faults-only`.

## Final-source checks

| Gate | Result |
|---|---|
| fmt | PASS |
| strict daemon all-target Clippy | PASS |
| `cargo test --locked -p cryptexd -p cryptex-events` | events 61; daemon library 379 / 4 ignored; CLI 8; lifecycle 9; Mode B 3; update/rollback 2 PASS |
| All four explicit native fixtures, serial | PASS |
| daemon build | PASS |
| actual PowerShell 5.1 runner helper | 3/3 PASS |

Proof directory:
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/controller-faults-8be985b42daa4ca3b64001ac24805dd6`.
Normalized source inventory: `m8-controller-faults-source-sha256-20261021.txt`
(31 files). Debug daemon SHA-256:
`bc82773086e391cfa11afa4107b5b09d590afabca41901b2b087ebd4f6cc8aa2`.
The runner references the new inventory; historical manifests/packages remain
unchanged. Full runner was not executed as one invocation; its individual
commands and PS5.1 helper were exercised.

The earlier intermittent credential read-after-delete and abrupt-worker exit
observations remain unresolved; this run passed and does not establish a fix.
Production startup/worker cancellation/status IPC, later Mac coverage and
ADR-0019 exact protocol/security review remain open. No peer trust, canonical
security writer, Actions, commit, push, merge or M8-complete claim.
