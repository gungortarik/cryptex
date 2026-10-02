# 34 — Current Implementation Handoff

**Snapshot date:** 2026-10-02  
**Current milestone:** M0 — Foundation / P0-2 Background Runtime & Atomic Update Lifecycle  
**Current branch:** `m0/p0-2-cursor`  
**Implementation base before this handoff:** `34073a655abc4ef5589b6e24a7192b4b9e1ef70c`

> This file is an operational status snapshot, not an architecture authority. If it conflicts with `docs/00_MASTER_SPEC.md`, accepted ADRs, the milestone contract, or other authoritative project documents, stop and report the conflict instead of silently reconciling it.

## Required reading before continuing

1. `AGENTS.md`
2. `docs/00_MASTER_SPEC.md`
3. `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
4. `docs/20_MILESTONE_ROADMAP.md`
5. `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
6. `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
7. relevant `docs/adr/` records
8. relevant `evidence/m0/p0-2/` evidence

## Working agreement

- Work only on the current milestone.
- Do not redesign architecture or choose a new supervision strategy without explicit user approval.
- If the user's intent about tool roles, workflow, architecture, branch strategy, milestone scope, or integration strategy is ambiguous, ask before changing direction.
- Do not assume that moving between Cursor, Codex, and Work changes which tool is the write-capable implementer.
- Cursor remains the current implementation tool unless the user explicitly changes that role. Codex/Work may be used for read-only review, audit, or verification.
- Do not merge, rebase, switch branches, amend, force-push, or rewrite history without explicit approval.
- Runtime data must remain outside the source repository.
- Do not touch retained P0-1 corpora unless the current approved test requires it.
- Never label behavior VERIFIED unless it was actually physically/test verified.

## P0-1 status

P0-1 physical event packaging is complete and ADR-0013 is accepted.

Decision:
- canonical physical packaging uses immutable sealed per-device segment/pack files
- one-file-per-event was rejected at scale
- hybrid packaging was not selected
- the benchmark profile's `segment_events=1000` is not a frozen production constant

Important branch note:
- the current P0-2 branch forked before final P0-1 acceptance landed
- stale P0-1 references on this branch must not be silently "fixed" by inventing or recreating the P0-1 integration
- integrate branches only through an explicit approved integration step

## P0-2 status — DECIDED / VERIFIED evidence

### macOS

Existing evidence proves:
- single-instance rejection
- crash detection / restart under the macOS native supervision candidate
- Mode B DesktopSupervisor process-level lifecycle
- Mode B GUI CloseRequested/hide path evidence
- synthetic compatible update / failed update / rollback marker-safety evidence

Still not enough to accept ADR-0012.

### Windows Mode B — valid physical foundation evidence

Experiment:
- `windows-mode-b-r2-20261002T050408Z`

Verified:
- daemon ready
- duplicate daemon rejected
- process-level window-close event leaves daemon alive
- forced crash detected
- DesktopSupervisor restart succeeds
- restart latency observed: 32 ms
- stale-lock wait before respawn
- clean stop
- runtime control-file cleanup
- post-restart status observation

Not verified by that run:
- physical Tauri GUI window close
- full Desktop IPC reconnect
- login/reboot startup

### Windows native Task Scheduler candidate — failed candidate

Experiment:
- `windows-mode-a-r1-20261002T052751Z`

Physical result:
- normal interactive user session
- `schtasks /Create` returned `ERROR: Access is denied.`

Classification:
- valid FAILED CANDIDATE / PERMISSION evidence
- not an invalid harness result
- no UAC/elevation bypass was used
- Task Scheduler via `schtasks` is not accepted as a no-elevation per-user registration path
- this failure does not automatically select Mode B

### Windows HKCU Run candidate

Test-only registration:
- value name: `CryptexDevP02HkcuRun`
- key: `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`
- runtime: `%LOCALAPPDATA%\\Cryptex-Dev\\p0-2\\compare\\mode-a-hkcu`
- no-elevation registration succeeded physically

The candidate is login-start only:
- it does not provide OS crash restart
- crash-restart parity remains OPEN
- ADR-0012 remains unresolved

## Latest physical event — real Windows reboot/login

Both Windows and macOS machines were shut down and restarted on 2026-10-02.

The Windows HKCU Run test value had been registered before shutdown.

After the real Windows reboot/login:

```text
registered=true
daemon_lock_held=false
daemon_pid=25856
ready_present=true
```

`Get-Process cryptexd` returned no running process.

The retained `lifecycle.status` contained:

```text
runtime_root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\compare\mode-a-hkcu
pid=25856
instance=held
health=active
supervision=unresolved
registration=not_configured
protocol_version=1
note=holding instance lock for harness
updated_unix=1790946028
```

Artifact inspection showed:
- `ready` exists, contains `25856`, last written 2026-10-02 13:00:28 UTC
- `daemon.lock` exists, contains `25856`, last written 2026-10-02 13:00:28 UTC
- `lifecycle.status` last written 2026-10-02 13:00:28 UTC
- `stop` is absent
- no live `cryptexd` process exists when checked
- the instance lock is no longer held

### Approved interpretation

**Windows HKCU Run login launch is PHYSICALLY VERIFIED.**

The matching timestamps and ready/lock/status artifacts prove the login registration launched `cryptexd` and it acquired the instance lock long enough to publish ready/status.

**Post-login daemon survival is FAILED / ROOT CAUSE OPEN.**

The process later disappeared without a stop file and without writing a clean-shutdown status. Do not claim HKCU Mode A passed.

This also exposes a truthful-health problem:
- stale `daemon_pid` can remain visible after the process has exited
- persisted `lifecycle.status` can still say `instance=held` / `health=active` even though a fresh lock observation says the lock is not held

Persisted status is therefore not sufficient evidence of current runtime health.

## Syncthing state after reboot

Before reboot, the running Windows Syncthing process was confirmed to be the old P0-1 development instance:

```text
syncthing.exe serve --home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1 --no-browser --no-restart --no-upgrade
```

It was launched from a user PowerShell process and was not registered as a Windows service, scheduled task, HKCU Run value, or Startup-folder item.

The reboot therefore stopped it. P0-1 is already complete. Do not restart it merely to continue P0-2, and do not delete retained P0-1 forensic/corpus data.

## Immediate next task

Before implementing another supervision mechanism or changing architecture:

1. perform a read-only root-cause audit of why the HKCU Run-launched `cryptexd`:
   - launched successfully
   - acquired the lock
   - wrote `ready` and active status
   - then exited without a stop file
2. inspect:
   - Windows startup command construction and quoting
   - console/session/process-lifetime behavior at login
   - `lifecycle hold` semantics
   - inherited handles / process-group behavior if relevant
   - runtime artifact semantics
3. separate proven facts from hypotheses
4. audit the stale PID / persisted health truthfulness issue
5. record the reboot/login evidence under `evidence/m0/p0-2/`
6. do not implement a replacement supervision mechanism until the audit is reviewed

## OPEN

- root cause of Windows HKCU Run post-login daemon exit
- truthful runtime-health handling for stale PID/status artifacts
- Windows crash-restart parity for a no-elevation/native candidate
- Windows physical Tauri GUI close proof
- full Desktop IPC reconnect after daemon restart
- macOS login/reboot startup proof
- remaining update/rollback acceptance coverage
- final Mode A vs Mode B vs justified hybrid comparison
- ADR-0012 acceptance

## BLOCKED

P0-2 cannot be considered complete and ADR-0012 must not be accepted until the remaining required proofs and comparison are resolved.

M1 is not authorized until all M0 P0 gates are complete.
