# P0-2 Windows Mode A — Task Scheduler FAILED CANDIDATE / PERMISSION evidence

**Date (UTC):** 2026-10-02T05:27:51Z  
**Host:** Windows physical machine / x86_64  
**Branch:** `m0/p0-2-cursor`  
**Commit:** `b25c030edf07b0e5573beeaa48a3a97670bd9303`  
**Experiment ID:** `windows-mode-a-r1-20261002T052751Z`  
**Mode:** `native_logon_task`  
**Classification:** FAILED CANDIDATE / PERMISSION (valid physical evidence)

## Summary

This is a **valid** physical evidence point against the Windows Mode A spike
candidate that registers via `schtasks /Create`. It is **not** invalid harness
evidence and **not** a Mode B lifecycle verdict.

`schtasks /Create` failed in a **normal interactive Windows user session** with:

```text
ERROR: Access is denied.
```

Consequences observed:

- daemon never started
- no task remained registered afterward
- all lifecycle fields after registration correctly remained `NOT_YET_VERIFIED`

## What this proves

| Claim | Status |
|---|---|
| Harness invoked the intended Mode A register path | VERIFIED (attempt reached `schtasks /Create`) |
| Registration succeeds without elevation | **FAILED** |
| `RunLevel=LeastPrivilege` implies no-admin *creation* | **REFUTED** |
| Daemon start / duplicate / crash / restart under Mode A | NOT YET VERIFIED (blocked by registration) |

## Important correction

Earlier comparison text treated Task Scheduler as “per-user, no admin” because the
task XML used `RunLevel=LeastPrivilege` / interactive logon principal.

That conflated two different privileges:

1. **Runtime identity** of the launched process (LeastPrivilege / InteractiveToken)
2. **Registration privilege** required to create/change the scheduled task

Microsoft `schtasks` documentation states that scheduling or changing tasks on the
local computer requires Administrator membership. Physical Windows result matches
that documentation for the harness path (`schtasks /Create`).

`RunLevel=LIMITED` / `LeastPrivilege` only constrains how the task *runs* after it
already exists. It does **not** establish that task *creation* is available without
administrative permission.

## Disposition

- Status: **FAILED CANDIDATE / PERMISSION**
- Do **not** elevate, bypass UAC, or weaken security to force this path green
- Do **not** treat this as automatic selection of Mode B
- Do **not** accept ADR-0012 from this result alone
- Task Scheduler via `schtasks` remains unsuitable as a **no-elevation** Mode A
  registration proof for the intended Cryptex per-user model unless a separate,
  explicitly proven non-elevated registration API is demonstrated later

## Follow-up (not implemented here)

See `windows-supervision-comparison-protocol.md` for the corrected comparison table
and the recommended next experiment (read-only reassessment only).