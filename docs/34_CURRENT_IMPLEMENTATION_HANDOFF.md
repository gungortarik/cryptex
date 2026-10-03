# 34 — Current Implementation Handoff

**Snapshot date:** 2026-10-03
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
- GitHub Actions minutes are constrained. Prefer relevant local checks and
  retained physical-host evidence. Run workflows only when required evidence
  cannot be produced locally; avoid redundant progress runs.
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
- physical traffic-light close and operator Dock restore; daemon PID unchanged
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
- The shared production Desktop Rust IPC client has now physically reconnected
  to real daemons after supervised restart on both Windows and macOS; see the
  platform-specific proofs below. The actual Tauri renderer-to-command
  invocation during external Mode B crash/restart is VERIFIED on both Windows
  and macOS. Production Desktop ownership of the supervisor remains OPEN.
- A separate P0-3 candidate prototype now exists at `tools/m0-ipc-spike/`.
  Its Windows physical run verified role/version/capability handling,
  malformed/oversized rejection and a client reconnecting across an actual
  server-process restart. The DACL is explicitly set to the current token SID,
  and access denial with an authenticated second account subsequently passed
  on a disposable Windows CI VM (`windows-cross-account-acl-20261002.md`). The macOS physical
  runtime proof now verifies socket permissions/path, protocol rejection and
  reconnect after server restart. This is not production-client integration or
  an ADR-0007 decision. Evidence:
  `evidence/m0/p0-3/windows-ipc-spike-20261002.md`.
- A coordinated Windows release-renderer attempt kept the Tauri reconnect
  command active while Mode B force-killed and restarted the daemon. Mode B and
  its shared Rust IPC reconnect passed (`25988 -> 35808`, 34 ms), but the
  renderer's status did not change during the 12-second observation. A separate
  login daemon using the same named-pipe endpoint was active; this may confound
  the observation, but its effect is not proven. Classify the renderer crash
  reconnect as **NOT YET VERIFIED** and preserve the login daemon. The optional
  `--pre-crash-hold-ms` and `--post-restart-hold-ms` harness windows support a
  coordinated repeat that leaves the replacement daemon available for the
  renderer.
  See `evidence/m0/p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md`.
- A controlled Windows repeat used both bounded holds. The release renderer
  displayed `35224 -> 3432`, matching the Mode B measurement; restart latency
  was 32 ms, and both the harness and Desktop exited 0. The Windows external
  Mode B renderer crash/reconnect gate is **PASS / VERIFIED**. The earlier
  uncontrolled-window attempt remains historical NOT YET VERIFIED evidence.
  See `evidence/m0/p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md`.
- The coordinated physical macOS renderer crash/reconnect test is recorded at
  `evidence/m0/p0-2/macos-renderer-crash-reconnect-test-plan-20261003.md`.
  The repeat at `dc0ae10` used both bounded holds (15000 ms before crash,
  10000 ms after restart). The visible renderer displayed
  `IPC reconnected after restart · pid 24966 → 24987`; both PIDs match the
  retained harness measurement (30 ms restart, exit 0). The macOS renderer
  crash/reconnect gate is **PASS / VERIFIED**. Desktop remained open until
  the result appeared, then exited 0 through `Quit clean`. The prior attempt
  without a post-restart hold remains historical NOT YET VERIFIED evidence.
- `cryptexd lifecycle update-proof` now rejects a synthetic mismatched
  Desktop/daemon protocol pair before staging, keeps active daemon `v1`, accepts
  a matching pair, and then runs the existing failed-candidate/rollback proof.
  Windows local tests passed (44 tests); the physical-host measurement is
  retained under `%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\windows-update-compat-20261003`.
  This remains test-only protocol metadata; it does not update a Desktop
  package. The physical Mac repeat at `46169ee` is **PASS / VERIFIED** for
  the synthetic fixture: 33 local tests passed, all ten requested measurement
  fields matched, marker digest remained `d914619dfbb07c95`, and final instance
  count was 0. Tests, build and update-proof exited 0. Evidence is in
  `evidence/m0/p0-2/macos-update-compat-evidence-20261003.md`.
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
- The test-only native login/reboot path is verified on both platforms (Windows
  HKCU Run; macOS LaunchAgent). This does not establish production registration
  or select a supervision candidate.
- The separate historical HKCU Run dispatch/exit findings remain without a
  root cause. Do not relabel the later successful reboot/login proof.

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

## Historical physical event — earlier Windows reboot/login

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

### Latest Windows reboot/login result — VERIFIED

The user prepared `wl-1003h` from ordinary Windows PowerShell at
`2026-10-03T02:02:24Z`, source `0a7eb0e`, clean tree, then physically restarted
Windows. Boot was at 22:06:04 EDT; console session 1 logged in at 22:06.
Daemon PID 31512 started at 22:07:32 EDT and observer PID 31192 attached.
Shell-Core launch records match both PIDs. The complete 600-second observation
ended at 22:17:34 EDT with `alive_at_observation_deadline`, no stop marker and
no observed daemon exit. The direct process query afterward confirmed the same
live daemon and executable. A duplicate probe was rejected with exit 1.
See `evidence/m0/p0-2/windows-reboot-login-survival-20261002.md`.

The legacy comparison registration was safely backed up and replaced in the
user's actual context. Its old stop file remains retained. The new test daemon
and registrations remain active; do not run registry-mutating Rust lifecycle
tests. No GitHub workflow ran. Current Windows reboot/login startup and
measured survival are verified; the historical 13:00 UTC disappearance still
has no established cause. The Windows Mode B physical crash/restart proof is
already retained above. The next physical IPC check must use the actual Desktop
client against the actual `cryptexd` transport across a process restart; this
does not by itself accept the still-open ADR-0012 supervision choice.

Windows release foundation build is now verified: the full Rust workspace and
the Tauri Desktop with embedded frontend built successfully. The release IPC
process-level check passed. CI now includes the IPC spike on both platforms;
the first dispatched run passed on macOS and exposed a Windows-only redundant
borrow lint. The fix passed full local Windows workspace Clippy; combined-source
CI revalidation passed on Windows and macOS for source commit `a0f1c6f`.
See `evidence/m0/windows-release-build-20261002.md`. The user currently has no
accessible second Windows account. The cross-account IPC ACL proof instead
passed on a disposable GitHub-hosted Windows VM at source `7668d11`, run
`37085465368`. The test authenticates a temporary non-admin account and checks
the effective client-thread SID, exact access denial, owner connections before
and after, identity restoration and account cleanup. It refuses local workstation
execution. See `evidence/m0/p0-3/windows-cross-account-acl-20261002.md`.
The consolidated Windows result index is recorded at
`evidence/m0/windows-test-results-20261003.md`.

Retained diagnostic history and continuation constraints:

1. the second real login changed session 3 to 4, but the user's external
   collector found `wd-1003a` registration absent while Codex-context reads
   found it present. Do not equate these views or infer deletion/recreation.
   The user prepared fresh `wd-1003b` from ordinary Windows PowerShell at
   `2026-10-03T01:38:11Z`; external collection confirmed
   `run_matches_both_views=true`. The next collection at `01:39:33Z` still
   shows session 4 / Explorer created at `01:33:05Z`, before preparation.
   The subsequent real login changed session 4 to 5. Marker PID 15656 wrote
   at `01:42:14.7693778Z`; Shell-Core 9708 recorded that PID at `01:42:15Z`.
   The external-prepared benign HKCU Run command is now physically verified
   to dispatch after login. This does not prove daemon survival.
   Preserve `wd-1003a` and avoid preparing/cleaning startup values from Codex.
   The collector now records package identity and executable ancestry as well.
   `collect-windows-run-dispatch.ps1` retains observations in each session
   root. This command captures registry views, identity, policy, marker integrity
   and relevant Shell-Core events into a unique retained JSON observation.
   Five synthetic controls passed; collection on the physical Windows host
   recorded matching registry views, unchanged script hash, no Run-disabling
   policy values, Run enumeration and no experiment launch/marker.
   Registry visibility depends on observer context; package virtualization is
   a hypothesis, not yet a verified root cause. Preserve session evidence. This
   probe does not establish daemon survival or explain a termination cause.
2. The `wl-1003g` attempt was prepared through `Shell.Application` from Codex.
   Its process chain reaches Explorer, but its status wrapper still inherited
   Codex's registry view; this is not proof of the user's unvirtualized Run
   values. After the latest user sign-out/sign-in, no daemon or observer runtime
   files were present under `wl-1003g`. It is NOT a verified armed login test.
   Prepare a fresh instrumented root in the user's ordinary Windows PowerShell,
   confirm the manifest there, then sign out/in and return. Codex can read the
   retained results. Earlier `wl-1003c/d/e/f` preparations are incomplete.
   `wl-1003f` identified missing `Get-FileHash`; framework SHA-256 fixed that
   failure but not registry-view separation. No CI was dispatched. Preserve
   process-start/shell-launch
   evidence. Do not infer a termination cause if no observer attaches. See
   `evidence/m0/p0-2/windows-instrumented-login-result-20261002.md`.

   Latest external registry output identifies the occupied daemon value as the
   original `target/debug/cryptexd.exe lifecycle hold` command for
   `%LOCALAPPDATA%/Cryptex-Dev/p0-2/compare/mode-a-hkcu`; the external observer
   value is absent. This explains why cleanup against `wl-1003g` refused.
   The legacy root retains a stop file and records a clean shutdown at that
   login. The user ran `replace-legacy-windows-login.ps1 -Apply` from ordinary
   PowerShell and prepared `wl-1003h`; reboot/login and ten-minute survival
   subsequently passed as recorded above. The helper requires the
   exact confirmed old command and an empty observer slot, saves the old value,
   refuses a live legacy ready PID and restores the saved command on preparation
   failure if both slots are vacant. It preserves legacy runtime files.
3. P0-3 candidate comparison is recorded in
   `evidence/m0/p0-3/adr-0007-candidate-comparison-20261003.md`. The user
   selected the evidence-backed `interprocess` 2.4.4 candidate; the scoped
   implementation addendum is accepted at
   `docs/adr/ADR-0007-local-ipc-implementation-addendum.md`. The production
   daemon health endpoint and shared Tauri Rust client are implemented. A
   physical Windows Mode B comparison at source `c5e059d` verified that the
   client connected before forced crash and reconnected to the restarted daemon
   (`2880 -> 24780`, 32 ms restart); see
   `evidence/m0/p0-2/windows-desktop-ipc-reconnect-20261002.md`. Separate
   physical Tauri renderer proofs are now recorded for Windows and macOS in
   `evidence/m0/p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md` and
   `evidence/m0/p0-2/macos-tauri-renderer-ipc-reconnect-20261003.md`. Both drove
   health/reconnect commands through the real renderer and observed a new daemon
   PID after clean stop/replacement. The macOS and Windows repeats also prove
   renderer reconnect while external Mode B force-restarts the daemon. The
   production supervisor/packaging decision remains open; do not accept ADR-0012
   yet.
4. use the existing physically verified macOS LaunchAgent login/reboot proof
   in `evidence/m0/p0-2/macos-native-supervision-evidence.md`; production
   registration/package integration remains unverified.
5. use `evidence/m0/p0-2/supervision-candidate-comparison-20261003.md` as the
   current test-evidence comparison; production supervision/update proofs and
   the ADR-0012 decision remain open.

## OPEN

- root cause of Windows HKCU Run post-login daemon exit
- Windows crash-restart parity for a no-elevation/native candidate
- Tauri renderer health/reconnect invocation is physically verified on both
  platforms for clean daemon stop/replacement and external Mode B crash/restart.
  Literal macOS traffic-light close and operator Dock restore are VERIFIED at
  evidence commit `52e9dc4` (test build source `aed973d`): Desktop PID `27505`, daemon PID `27600` unchanged after restore,
  `source=tauri_close_requested`, clean exit 0 and released daemon lock.
  See `evidence/m0/p0-2/macos-physical-traffic-light-close-evidence-20261003.md`.
  Production Desktop supervision, tray UX and packaging remain open.
- Windows cross-account ACL denial verified on a CI VM; production Desktop/CLI/MCP
  integration remains absent
- real compatible Desktop + daemon package coordination and rollback/canonical-data proof
- production-backed Mode A vs Mode B vs justified hybrid comparison
- ADR-0012 acceptance

## BLOCKED

P0-2 cannot be considered complete and ADR-0012 must not be accepted until the remaining required proofs and comparison are resolved.

M1 is not authorized until all M0 P0 gates are complete.
