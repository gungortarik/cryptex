# 34 — Current Implementation Handoff

**Snapshot date:** 2026-10-03
**Current milestone:** M0 — Foundation / P0-2 Background Runtime & Atomic Update Lifecycle  
**Current branch:** `m0/p0-2-cursor`  
**Implementation base before this handoff:** `8d71044 docs: close macOS physical window proof`

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
- The project owner explicitly approved Mode B with per-user login startup launching Desktop for implementation. The scoped proposal is at `docs/adr/ADR-0012-background-runtime-implementation-proposal.md`; ADR-0012 remains unaccepted until the listed production proofs pass.
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

## P0-2 evidence status — verified slices; acceptance open

### macOS

The combined Mac release lifecycle slices now PASS, including physical tray
Open/Quit at checkout `dfbaded` using the validated `c77479f` release build.
Desktop `41780` / daemon `41851` survived physical tray left-click restore and
right-click Open with the same daemon PID. Snapshots recorded
`window_restored_via_tray_left_click` and `window_restored_via_tray_menu`.
Physical tray Quit with the daemon active exited 0 and recorded
`clean_quit_from_tray`; both processes and the held lock were gone. Login start
remained disabled. The operator identified the actual black rounded-square
icon left of ChatGPT after its menu-bar position changed; earlier synthetic
input failures are historical, not proof of an application defect.

The initial Desktop-owned lifecycle run at `c77479f` was PARTIAL. Local daemon supervisor
tests (3), sidecar staging tests (3), sidecar resolution tests (2) and release
package-smoke passed. Release Desktop `31422` automatically started daemon
`31562`, rejected a second Desktop launch, and restarted the SIGKILLed daemon
as `32006` within a 500 ms polling observation; renderer IPC answered protocol 1
with the replacement PID. One tray icon and duplicate daemon rejection were
confirmed. Login start was enabled through `Cryptex.plist` for the release
Desktop with `--cryptex-background`. The operator completed real sign-out/sign-in:
Desktop `32654` started with the background argument, no visible window and one
tray icon; daemon `32891` answered renderer IPC with protocol 1 after Dock reopen.
Explicit Stop left no daemon for a measured 10 seconds. Login start was disabled
through the renderer and `Cryptex.plist` removal verified. Final `Quit clean`
removed the Desktop/daemon processes and recorded `state=clean_quit`. Actual
tray Open/Quit could not be exercised; tray exit 0 was not captured. Those tray
checks were NOT YET VERIFIED in that initial run; the test registration was cleaned up and the
runtime evidence is retained.
See `evidence/m0/p0-2/macos-tauri-owned-supervisor-restart-evidence-20261003.md`.
The later physical diagnosis closes those two Mac tray checks. Signed installer
delivery, actual package update/rollback and ADR-0012 acceptance remain open.

The focused tray-only follow-up at checkout `924cc13` reused the validated
release build without repeating login/restart/Stop suites. Desktop `38558`
started daemon `38569`; red close-control activation hid the window and
preserved that PID. Native right/left clicks at the accessible tray item's
observed center did not expose Open/Quit or restore the window. Physical
display captures are retained outside Git in `/tmp/cryptex-tray-only-*.png`.
Tray Open/Quit remained NOT YET VERIFIED in that synthetic follow-up. Fallback single-instance focus plus
renderer `Quit clean` exited 0; no test processes or held lock remain and
login start remained disabled. The UI limitation's root cause is unproven.

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
  and macOS. The Mac release Desktop-owned lifecycle now has partial physical
  evidence (see the current Mac run above); Windows release-mode physical
  verification and the complete cross-platform production gate remain OPEN.
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
- A second physical reboot/login in the `wl-1003h` test produced a fresh
  console daemon PID `26576`, attached observer PID `26356`, and another
  `alive_at_observation_deadline` result after about 601 seconds. The screenshot
  console was the expected output of this test's direct
  `cryptexd.exe lifecycle login-hold` HKCU Run command; it was not the Cryptex Desktop. Both
  test Run values were then removed by exact manifest comparison in the actual
  user's SID-scoped registry view. A cooperative stop was requested; fresh
  status reports `pid=none`, and no matching startup entries or process remain.
  The retained lock file is stale and is not held. Runtime evidence and a
  pre-cleanup registration backup remain under LocalAppData. See
  `evidence/m0/p0-2/windows-reboot-login-survival-repeat-20261003.md`.
- A current-source Windows release executable physically started the
  Desktop-owned daemon, rejected a second Desktop launch (exit 0), replaced an
  exactly identified child after controlled termination, and passed the
  automated Tauri close/show request path with the replacement daemon live.
  This run used the existing executable built from `c77479f`; literal tray
  Open/Quit and login startup were not exercised. A fresh
  `tauri:package-smoke` build is blocked on this host: Code Integrity events
  3033/3077 deny `rustc.exe` loading its generated unsigned
  `serde_repr` proc-macro DLL. No policy bypass was attempted. See
  `evidence/m0/p0-2/windows-release-desktop-owned-supervisor-20261003.md`.
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

## Current continuation

### Windows implementation slice — committed; Mac release lifecycle passed

The project owner approved Mode B with per-user login startup launching
Desktop. The proposal is recorded at
`docs/adr/ADR-0012-background-runtime-implementation-proposal.md`; it is
**PROPOSED, not accepted**. The committed Windows implementation slice adds
unexpected child-exit detection and bounded restart handling to the current Tauri P0-2
supervisor, plus lock-aware restart behavior to the shared process controller.
A target-matched daemon sidecar is now built into the Tauri package-smoke
artifact. Release builds use the product app-data runtime root, start or attach
the daemon, watch adopted daemon processes, enforce one Desktop instance,
expose reversible per-user login-start controls, and create a tray menu for
Open/Quit. Signed installer packaging and complete Windows physical validation
of the startup/tray/supervisor flow remain open; Mac release-mode results are
summarized below.

Verification on Windows:

- `pnpm --dir apps/desktop build`: PASS.
- `cargo check -p cryptex-desktop --locked`: PASS.
- `pnpm --dir apps/desktop test:sidecar-build-script`: PASS (3 target/path
  tests).
- `pnpm --dir apps/desktop tauri:package-smoke`: PASS; Tauri's package-flavored
  release build staged `cryptexd-x86_64-pc-windows-msvc.exe` and produced
  `target/release/cryptex-desktop.exe`. The sidecar is a synthetic development
  build, not yet a signed installer. The current run includes tray and
  autostart integration and completed without compiler warnings.
- `cargo check -p cryptex-desktop --tests --locked`: PASS; all Rust test targets
  compile, including the pinned Tauri single-instance integration.
- `cargo test -p cryptex-desktop --bin cryptex-desktop --locked`: PASS (8 tests)
  on the current implementation, including release runtime paths and sidecar
  resolution.
- `cargo clippy -p cryptex-desktop --all-targets --locked -- -D warnings`:
  blocked because Smart App Control rejected `cargo-clippy` with OS error 4551.
- Rust formatting and `git diff --check`: PASS.
- `cargo test -p cryptexd --test mode_b_supervisor --locked`: test binary built,
  but two process-spawning cases were blocked by Windows Smart App Control
  (OS error 4551). Do not bypass that policy. `cargo check -p cryptexd --tests
  --locked` passed. The Mac run passed all three targeted process tests; Windows
  release-mode physical lifecycle verification remains open.
- `cargo build -p cryptex-desktop --release --locked`: PASS.
- Latest `pnpm --filter @cryptex/desktop tauri:package-smoke`: frontend build
  and sidecar preparation passed, but Tauri Rust compilation failed with
  `E0463` because Windows Code Integrity blocked the generated
  `serde_repr` proc-macro DLL (events 3033/3077). The previously built release
  executable was byte-matched to the latest Desktop source commit and used for
  partial physical lifecycle checks. Do not bypass the host policy. A fresh
  Windows release package must be built on an approved host/configuration.
- No GitHub Actions workflow ran.

A later single-job manual Windows package build was dispatched at `0867b9f`:
`https://github.com/gungortarik/cryptex/actions/runs/37168036553`.
Readiness/Mac and IPC jobs were skipped. It builds an unsigned development
NSIS installer and retains Desktop/daemon hashes; it is not signing or physical
host acceptance. See `evidence/m0/p0-2/windows-ci-package-build-20261003.md`
for execution status before requesting another run. That run completed:
NSIS build, 3 staging tests and 8 release helper tests passed. Artifacts were
downloaded outside Git and their hashes verified. The loose CI Desktop/daemon
ran on the Windows host; installer execution and signing remain unverified.

Windows physical tray Open and active-daemon Quit now PASS in
`evidence/m0/p0-2/windows-physical-release-tray-20261003.md`: Desktop `8140`,
daemon `22828`, native title-bar close hid the window, physical Open preserved
the daemon PID, and physical Quit exited 0 with both processes gone and lock
released. Actual Desktop login startup remains open. A login toggle attempt
from the CI artifact showed a direct-HKCU versus Windows startup-list mismatch;
it was disabled again and the CI processes cleaned up. Launch from the
operator's ordinary session before arming the real login test. Do not dispatch
another workflow for these local checks.

The ordinary-session follow-up is now armed: Explorer launched Desktop `12084`
and daemon `8052`; the renderer enabled startup and Windows' startup list
verified the exact CI-artifact Desktop command with `--cryptex-background`.
Preparation is outside Git at
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\windows-desktop-login-1003`.
The real sign-out/sign-in has now completed. Explorer launched Desktop `29872`
with `--cryptex-background` and daemon `26588` in the new session. The main
window was hidden, physical tray Open restored it, and renderer status/IPC
confirmed the held lock, active daemon and protocol 1. The renderer disabled
the product startup value; Windows startup-list removal was verified. Renderer
Quit clean exited 0 and both processes disappeared. These login slices PASS;
see `evidence/m0/p0-2/windows-desktop-login-result-20261003.md` for exact scope
and the stale agent filesystem records excluded from this proof.

The operator reported login-time consoles. A daemon-owned console host was
observed; all Desktop status/preflight/hold commands now use a platform adapter
with Windows `CREATE_NO_WINDOW`. Local check, all-target Clippy, 11 Desktop
tests and release package-smoke PASS. No host policy was changed and no new
workflow ran. The rebuilt release launched locally, then was cleaned up; its
physical no-terminal/flash observation remains inconclusive, and UI geometry
errors prevented the new-release status/IPC check. Do not count that visual
gate as passed. Product login is now disabled; test processes are gone.
The later observed foreground launch at checkout `73263ca` closes that visual
check: Desktop `14768` / daemon `8968`, renderer status held/active and IPC
protocol 1 passed, and the operator watching the screen confirmed no terminal
window/flash during launch and queries. Renderer Quit clean exited 0; processes
and held lock were gone. No login/reboot, rebuild or workflow was repeated.
Evidence: `evidence/m0/p0-2/windows-console-visual-result-20261003.md`.
Mac compile/lint request:
`evidence/m0/p0-2/macos-background-command-check-plan-20261003.md`.
Executed on macOS 27.0.1 arm64 at `83a770a`, Rust 1.98.1: format PASS
(exit 0), Desktop check PASS (exit 0, dead-code warning), strict all-target
Clippy FAIL (exit 101). `main.rs:64:4` reports unused
`mac_product_runtime_root` in the debug binary; its production caller is
release-only and its other use is test-only. The failure reproduced in an
individual command run. Exact evidence:
`evidence/m0/p0-2/macos-background-command-check-evidence-20261003.md`.
No application code, lint allowance, release package or Actions was changed.
That initial run did not close the strict Mac lint gate.

Source follow-up: `mac_product_runtime_root` is now compiled only for macOS
release or test targets, matching its actual callers and the Windows helper
pattern. No dead-code allowance was added. The Mac runtime tests also cover
missing HOME. Windows format, Desktop strict Clippy and the 11-test Desktop
suite were rerun locally. Mac revalidation has now completed at
`808bb1c039ab3735e3a6a777fafa6eae49790b10` on macOS 27.0.1 arm64,
Rust 1.98.1: format, Desktop check, strict all-target Clippy and targeted
`mac_runtime_path_tests` all exit 0 (2 tests passed, 2 filtered out). Check
and Clippy emit no warnings. The Mac compile/lint gate is now PASS/closed;
the evidence above retains the initial failure and appends exact new results.
No application code, physical lifecycle tests, release package rebuild,
runtime configuration or GitHub Actions was changed/run in this revalidation.

The Mac execution request at
`evidence/m0/p0-2/macos-tauri-owned-supervisor-restart-test-plan-20261003.md`
has been executed. The evidence at
`evidence/m0/p0-2/macos-tauri-owned-supervisor-restart-evidence-20261003.md`
records release Desktop startup and crash/restart, duplicate rejection, real
login startup, Dock reopen plus renderer IPC, Stop without restart, login-start
removal and process cleanup. The subsequent physical-input diagnosis verified
actual tray **Open Cryptex** and **Quit Cryptex**, including active-daemon
shutdown and foreground exit code 0. The test registration and processes are
cleaned up. Do not repeat the passed steps merely to fix the document state.

The focused diagnostic procedure at
`evidence/m0/p0-2/macos-tray-input-diagnostic-plan-20261003.md` is complete;
its physical result closes the Mac tray gate. Windows release login-start and
physical tray/menu slices are now verified, and the earlier release run
verified daemon replacement. The rebuilt Windows release's observed console-free
launch/query behavior and renderer status/IPC are now verified. The old CI
package predates the fix. The Mac debug-target lint failure is resolved and
revalidated at `808bb1c`; do not repeat its passed compile/lint or physical
lifecycle suites for this request. Next, complete the real compatible
Desktop+daemon package update/rollback and canonical-data checks on both
platforms. Keep ADR-0012 unaccepted until every required P0-2 proof passes.

Windows native-package candidate preparation now passed locally: retained
unsigned NSIS `0.0.0` and test-labelled `0.0.1` installers at source `5015ff1`,
with payload hashes outside Git under
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\native-package-proof-1003`.
No version/dependency change was made in the repository and no workflow ran.
Execution plan: `evidence/m0/p0-2/windows-native-package-replacement-plan-20261003.md`.
Baseline installation from the ordinary Windows session now passed its scoped
identity/status/IPC/clean-exit checks: Desktop `26124`, daemon `9412`, protocol 1,
exit 0 and no remaining processes. Evidence:
`evidence/m0/p0-2/windows-native-package-baseline-20261003.md`.
The installed Desktop differs from the retained loose binary only in Tauri's
three-byte `UNK` to `NSS` bundle marker; the exact comparison reproduces its hash.
Candidate `0.0.1` replacement now passed installed version/payload/status/IPC
checks: Desktop `9416`, daemon `28480`, protocol 1, clean exit 0 and no remaining
processes. Evidence: `evidence/m0/p0-2/windows-native-package-update-20261003.md`.
Operator rollback to `0.0.0` also passed: installed Desktop/daemon hashes match
the first baseline, Desktop `25436`, daemon `15340`, renderer protocol 1,
clean exit 0 and no remaining processes. Login startup remains disabled.
Evidence: `evidence/m0/p0-2/windows-native-package-rollback-20261003.md`.
The final installation is stopped at `0.0.0`. The real package sequence passed
its scoped install/replace/operator-rollback checks. A separate serialized
filesystem transaction candidate now passes 28 Windows tests, including five
actual updater-subprocess kill boundaries, plus 8 retained-binary-copy scenarios.
It is not integrated with installed Desktop and its health is injected fixture
behavior. Proposal: `docs/adr/ADR-0012-update-orchestration-candidate.md`.
Windows evidence: `evidence/m0/p0-2/windows-package-transaction-candidate-20261003.md`.
Mac executed `evidence/m0/p0-2/macos-package-transaction-candidate-test-plan-20261003.md`
at `780c0473bdb85b52abc994661f10671cc85e4992`, macOS 27.0.1 arm64,
Node 24.19.0. The default `/var` temporary path failed closed with
`UNSAFE_PATH` (1 passed, 27 failed): `/var` is a symlink and initialization
rejects symlink ancestors. With process-local TMPDIR set to the verified
`/private/var/.../T` real path, all 28 tests passed, none skipped, including
five actual owned-worker SIGKILL boundaries, pre-commit baseline recovery,
post-commit candidate selection, idempotence, corrupt-baseline refusal and
unchanged synthetic fixture inventories. POSIX directory fsync succeeded;
this is not power-loss proof. All four syntax checks and diff checks passed.
Evidence: `evidence/m0/p0-2/macos-package-transaction-candidate-evidence-20261003.md`.
No code repair or security-check weakening was made. Temporary fixtures were
cleaned, including 27 verified leftovers from the failed setup run. Optional
retained-binary-copy proof was not run; no two retained pairs were established.
That initial Mac serialized candidate run was scoped PASS with the explicit
non-symlink temporary-path prerequisite; its unprefixed command failed.
After reviewing `12f76ac`, the test fixture allocator now resolves only the OS
temporary directory before creating owned roots and registers cleanup before
experiment initialization. The package engine's link/junction rejection is
unchanged. A new alias regression passes on Windows: 29/29 tests, no skips.
Evidence: `evidence/m0/p0-2/windows-package-fixture-path-fix-20261003.md`.
Mac follow-up now PASS at `fc31489fc619b9b4b12f93cd350b215efa7f3c7c`:
the ordinary test command with the host's original `/var/.../T/` TMPDIR,
without override, passed all 29 tests (zero failed/skipped, exit 0). The alias
regression and all five actual SIGKILL boundaries passed; POSIX directory
fsync and fixture inventory checks completed. All six syntax checks and diff
check passed. Owned temporary roots were absent before and after execution.
The evidence above appends exact output and both temporary-path forms while
preserving the prior default-path failure and overridden 28-test PASS.
The Mac default-path fixture repair is now verified; no external override is
required. No optional binary staging, source repair or physical test was repeated.
Next: review native helper/startup integration and failure/data-preservation
gates before production architecture selection. Publisher trust, actual
runtime compatibility, concurrency, machine power loss and canonical-data
acceptance remain open; ADR-0012 remains PROPOSED. No repeated physical Mac
lifecycle suite, installed-package change or GitHub workflow ran.

Windows real-health candidate follow-up now PASS: the transaction health callback
executes the copied daemon and the actual shared Rust Desktop IPC client, matches
PID/protocol/held lock, then verifies clean exit 0 and lock release. Compatible
replacement, non-executable daemon rollback and actual protocol mismatch rollback
passed (3 scenarios); restored baselines also answered IPC. Active endpoint
preflight refusal was measured. Rust tool tests 10/10, Node adapter tests 4/4,
format, strict Clippy, local build and syntax checks passed. Evidence:
`evidence/m0/p0-2/windows-package-real-health-20261003.md`.
Mac real-health plan `evidence/m0/p0-2/macos-package-real-health-test-plan-20261003.md`
now PASS at source `1e07ca4` on macOS 27.0.1 arm64 with Node 24.19.0. Format,
Rust tests 10/10, strict Clippy, local helper build, Node adapter tests 4/4,
both syntax checks and diff check passed. The retained arm64 release Desktop /
daemon pair from `c77479f` was supplied as identical baseline and candidate
inputs (Desktop SHA-256 `0bfd52594a4a30bfcd6e37678835bb7d833cfe6be867f44996e45e18de36f1a5`,
daemon SHA-256 `bfbfa26c6df1934312ba0d483f5351ac759c8f3f96458f8753d5b4a70157feb4`).
Three real health scenarios passed: compatible pair, non-executable candidate
rollback and actual protocol 2/2 declaration against protocol 1 daemon rejection
and baseline restoration. Four healthy IPC observations confirmed Desktop role,
matching daemon PID, held lock and protocol 1; candidate/recovery exits were 0
with locks released. The invalid daemon error was `DAEMON_START_FAILED`, child
exit `-13`. Synthetic fixture bytes were unchanged. Full report remains outside
Git at `~/Library/Application Support/Cryptex-Dev/p0-2/mac-package-real-health-20261003-b15a85f/real-health-report.json`.
Detailed evidence: `evidence/m0/p0-2/macos-package-real-health-evidence-20261003.md`.
Desktop bytes were staged only, not run. This is not a version-changing Mac
upgrade or production canonical-data proof; installed updater handover, signing,
concurrency, power loss and canonical-data gates remain open. No GUI/lifecycle
repeat, Desktop rebuild or Actions ran; ADR-0012 remains PROPOSED.
Canonical-data preservation and an operator-context post-exit lock query were
not established by this baseline. The stock installer force-shutdown and sequential
Desktop/daemon copy behavior does not prove safe atomic updates. Automatic
rollback and incompatible-pair preflight remain unimplemented; keep these
distinct from the planned operator-controlled replacement/rollback experiment.

### Prior continuation record

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
user's actual context. Its old stop file remains retained. A second physical
reboot/login repeat and cleanup are recorded at
`evidence/m0/p0-2/windows-reboot-login-survival-repeat-20261003.md`. The
`wl-1003h` test registrations are now absent and no matching daemon process is
running, so registry-mutating Rust lifecycle tests no longer risk disturbing an
armed experiment. No GitHub workflow ran. The historical 13:00 UTC disappearance
still has no established cause. The Windows Mode B physical crash/restart proof
is already retained above. The next Windows lifecycle task is the release-build
Desktop physical startup/close/quit flow, not another Mode A login repeat.

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
   in `evidence/m0/p0-2/macos-native-supervision-evidence.md`. The separate
   release Desktop login-start flow now has partial physical evidence on Mac;
   Windows release-mode proof and signed package integration remain open.
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
  That historical run did not test the production tray menu or packaged
  supervision. The later Mac release Desktop-owned run is recorded at the top
  of this handoff; its physical tray Open/Quit checks now pass. Signed package
  verification remains open.
- Windows cross-account ACL denial verified on a CI VM; production Desktop/CLI/MCP
  integration remains absent
- real compatible Desktop + daemon package coordination and rollback/canonical-data proof
- production-backed Mode A vs Mode B vs justified hybrid comparison
- ADR-0012 acceptance

## BLOCKED

P0-2 cannot be considered complete and ADR-0012 must not be accepted until the remaining required proofs and comparison are resolved.

M1 is not authorized until all M0 P0 gates are complete.
