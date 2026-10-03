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
- The user explicitly changed the implementation role to Codex in this conversation; Codex is the active implementation tool on Windows. Resume Mac-specific validation on the Mac host when needed.
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

### Additional Windows physical-host proofs (2026-10-02)

- Tauri `CloseRequested` hide/restore and daemon-survival proof: VERIFIED for the
  automated control-file-triggered physical Tauri event path. See
  `evidence/m0/p0-2/windows-mode-b-gui-window-close-20261002.md`.
- synthetic compatible update, bad-candidate rejection and rollback marker
  safety: VERIFIED for the Windows comparison harness. See
  `evidence/m0/p0-2/windows-update-rollback-evidence-20261002.md`.
- A direct child-process parent-exit control passed: the daemon retained its lock
  after the launching PowerShell exited. This does not reproduce HKCU Run or
  explain the separate post-login daemon exit; see
  `evidence/m0/p0-2/windows-parent-exit-control-20261002.md`.
- Fixed a reproducible HKCU comparison-harness defect: the on-demand
  `--leave-registered` probe now clears its one-shot stop marker before a later
  login. The physical Windows regression run retained the Run value during
  inspection, then removed it; see
  `evidence/m0/p0-2/windows-hkcu-stop-marker-cleanup-20261002.md`. This does
  not explain the historical reboot event, whose cause remains OPEN.
- Additional Shell-Core review confirmed the 9708 `Explorer_ExecutingFromRunKey`
  event is launch-operation completion, not proof of process exit: another
  startup PID from that same sequence remains alive. The old runtime artifacts
  are insufficient to identify why PID 25856 later disappeared.
- Full Desktop IPC reconnect after supervised restart remains OPEN.
- Source inspection confirms the current Desktop only queries status through a
  short-lived CLI process; full reconnect needs the M0 P0-3 transport proof.
  Status observation is not a named-pipe reconnect handshake.
- A separate P0-3 candidate prototype now exists at `tools/m0-ipc-spike/`.
  Its Windows physical run verified role/version/capability handling,
  malformed/oversized rejection and a client reconnecting across an actual
  server-process restart. The DACL is explicitly set to the current token SID,
  but access denial from another account remains untested. The macOS physical
  runtime proof now verifies socket permissions/path, protocol rejection and
  reconnect after server restart. This is not production-client integration or
  an ADR-0007 decision. Evidence:
  `evidence/m0/p0-3/windows-ipc-spike-20261002.md`.
- The new instrumented Windows login at 16:41:48 EDT on 2026-10-02 confirmed a
  regular user-profile logon. After 11.4 minutes, neither the direct daemon Run
  entry nor the PowerShell observer had produced a runtime artifact; the
  observer never attached, so no daemon exit code was captured. Shell-Core had
  no matching Run execution event, and Code Integrity had no matching block.
  Both temporary Run values were then removed and verified absent. Classify this
  as **login start NOT OBSERVED / cause OPEN**, not daemon start-then-exit. See
  `evidence/m0/p0-2/windows-instrumented-login-result-20261002.md`.
- Desktop close/restore/snapshot proof artifacts now query a fresh daemon
  lifecycle observation for the live PID instead of treating `ready` as live;
  status errors are reported as unavailable. A physical stale-ready Windows
  regression proof is included in the GUI evidence.
- These proofs do not explain the separate HKCU Run daemon exit after login,
  select a supervision candidate, or accept ADR-0012.
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

### Follow-up after the read-only audit

The Codex implementation pass keeps the supervision comparison open and changes
only lifecycle status reporting:

- `InstanceObservation` separates a PID associated with a currently held lock
  from the last PID recorded in lock/status metadata.
- `lifecycle status` refreshes the timestamped status snapshot from a fresh lock
  observation.
- the short-lived `lifecycle probe` writes a released/stale snapshot after it
  drops its lock, instead of leaving its temporary `active` report behind.
- platform status results expose `daemon_pid` separately from `recorded_pid`.
- tests cover crash, stale PID reporting, status refresh and the persisted
  snapshot.

This does not explain the post-login process exit or add a supervision
mechanism. A persisted snapshot may still age between checks; consumers must
query current status and must not treat the ready marker as liveness evidence.

### Benign Explorer Run dispatch probe — INCONCLUSIVE

The `wd-1003a` test-only registration had reported present before the user's
sign-out/sign-in. The post-login status output reported a registration mismatch
or absence and no marker; its old boolean does not distinguish the two cases.
Read-only inspection later found the exact registered
command present; rerunning Status returned `run_registered=true`. The
marker remains absent. Shell-Core shows that the Run key was enumerated but has
no launch event for the test marker. Its enumeration event does not expose the
value list, and the Security log was unavailable without elevation. Preserve
the session artifacts; use the currently matching registration for another
real sign-out/sign-in observation before cleanup. Do not infer a dispatch skip
or registration-removal cause. Evidence:
`evidence/m0/p0-2/windows-run-dispatch-followup-20261002.md`.

## Syncthing state after reboot

Before reboot, the running Windows Syncthing process was confirmed to be the old P0-1 development instance:

```text
syncthing.exe serve --home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1 --no-browser --no-restart --no-upgrade
```

It was launched from a user PowerShell process and was not registered as a Windows service, scheduled task, HKCU Run value, or Startup-folder item.

The reboot therefore stopped it. P0-1 is already complete. Do not restart it merely to continue P0-2, and do not delete retained P0-1 forensic/corpus data.

## Immediate next task

Windows release foundation build is now verified: the full Rust workspace and
the Tauri Desktop with embedded frontend built successfully. The release IPC
process-level check passed. CI now includes the IPC spike on both platforms;
the first dispatched run passed on macOS and exposed a Windows-only redundant
borrow lint. The fix passed full local Windows workspace Clippy; combined-source
CI revalidation passed on Windows and macOS for source commit `a0f1c6f`.
See `evidence/m0/windows-release-build-20261002.md`. The user currently has no
accessible second Windows account, so the cross-account IPC ACL test remains
pending. A disposable GitHub-hosted Windows CI job now prepares a temporary
non-admin user, verifies its real SID and requires an access-denied pipe
connection plus successful owner connections before/after. Its runtime result
is pending; the script refuses local workstation execution.

Before implementing another supervision mechanism or changing architecture:

1. repeat a real sign-out/sign-in with the still-registered `wd-1003a` benign
   marker command; then collect Status plus Shell-Core/process-start evidence.
   The first post-login Status read conflicted with a later registry read, so
   the result is inconclusive. Preserve the existing session evidence. This
   probe does not establish daemon survival or explain a termination cause.
2. if Explorer dispatches the marker, repeat the instrumented Cryptex login
   attempt with a fresh runtime root and preserve process-start/shell-launch
   evidence. Do not infer a termination cause if no observer attaches. See
   `evidence/m0/p0-2/windows-instrumented-login-result-20261002.md`.
3. prove Windows second-account denial for the P0-3 named-pipe ACL, then
   compare the transport candidates and record ADR-0007 details before
   production daemon APIs or Desktop reconnect claims. The Mac process-level
   socket proof is recorded in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`.
4. use the existing physically verified macOS LaunchAgent login/reboot proof
   in `evidence/m0/p0-2/macos-native-supervision-evidence.md`; production
   registration/package integration remains unverified.
5. compare the candidates and document evidence before accepting ADR-0012

## OPEN

- root cause of Windows HKCU Run post-login daemon exit
- Windows crash-restart parity for a no-elevation/native candidate
- full Desktop IPC reconnect after daemon restart remains open because the
  current Desktop still uses short-lived CLI status observation
- Windows second-account ACL denial not tested; production Desktop/CLI/MCP
  integration absent
- remaining update/rollback acceptance coverage beyond the recorded synthetic Windows harness run
- final Mode A vs Mode B vs justified hybrid comparison
- ADR-0012 acceptance

## BLOCKED

P0-2 cannot be considered complete and ADR-0012 must not be accepted until the remaining required proofs and comparison are resolved.

M1 is not authorized until all M0 P0 gates are complete.
