# P0-2 Windows HKCU Run — post-login exit audit

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Date:** 2026-10-02
**Platform:** Windows x86_64 physical workstation
**Branch under audit:** `m0/p0-2-cursor` at `34073a6`
**Mode:** test-only `native_hkcu_run`
**ADR-0012:** UNRESOLVED

## Evidence basis

The physical reboot/login observation below was recorded in
`docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md` on the updated remote branch. That
handoff reports the test value `CryptexDevP02HkcuRun` was registered before a
real Windows restart. At the next login, the ready file, lock file, and status
file all showed PID `25856` and a matching update time of 2026-10-02 13:00:28
UTC. A later process check found no running `cryptexd`, no held instance lock,
and no stop file. The persisted status still said `instance=held` and
`health=active`.

The handoff therefore classifies login launch as **PHYSICALLY VERIFIED** and
post-login survival as **FAILED / ROOT CAUSE OPEN**. The raw runtime files were
not copied into Git. A later automated harness run reused the same test runtime
root and refreshed its `ready`, `stop`, and `lifecycle.status` files, so those
original raw files are no longer available for reinspection. The values above
are preserved here as the handoff's recorded observation, not as a new physical
reproduction.

## Read-only source audit

### Proven by source

- `HkcuRunSpec::command_line` starts the executable directly and passes
  `lifecycle hold --runtime-root ... --ready-file ... --stop-file ...`.
- Windows arguments are quoted by `quote_windows_arg`. The automated probe and
  an earlier physical on-demand run demonstrate that this command shape starts
  the expected binary and writes its ready marker.
- `lifecycle hold` acquires the exclusive instance lock, writes
  `lifecycle.status` as active, writes the ready marker, then polls for the stop
  file every 25 ms.
- With HKCU's stop-file command, the normal completion path is reached only
  after the stop file appears. Normal completion releases the lock and writes a
  clean-shutdown status. The HKCU Run registry entry itself provides no crash
  supervisor.
- If the process is terminated outside that normal path, the last persisted
  status and ready marker are not cleared by `cryptexd`; they can outlive the
  process.
- At audit time, a fresh `observe_instance` call probed the lock but returned
  the recorded PID through `holder_pid` even when the lock was free. The
  follow-up fix separates `holder_pid` from `recorded_pid`; unlocked
  observations now leave the live-holder field empty.
- At audit time, `lifecycle status` printed a fresh observation but did not
  refresh the persisted status file. The follow-up fix refreshes that snapshot
  on each status check and after the short-lived `probe` command releases its
  lock.
- The platform status structs now expose `daemon_pid` only from an observed
  held lock and expose the retained metadata as `recorded_pid`. The ready
  marker still indicates a historical startup event, not current liveness.

### Not established

The source explains how stale markers can remain after abnormal termination,
but it does not explain why Windows terminated this specific post-login
process. The inspected Windows Application/System logs for 2026-10-02
12:50–13:15 UTC contained no `cryptexd` Application Error or Windows Error
Reporting event. That absence does not identify the termination source.

Possible causes such as logon-session process cleanup, a console/session
lifetime interaction, endpoint protection, or an unrecorded process exit remain
hypotheses only. No source or event-log evidence in this audit distinguishes
them.

## Audit conclusion

1. Login launch is proven by the handoff's matching ready/lock/status PID and
   timestamp.
2. Continued daemon survival is disproven for that experiment by the later
   process and lock observation.
3. The exact exit cause remains unknown; do not select a different startup or
   supervision mechanism based on a guessed cause.
4. Persisted `health=active`, PID, and ready-marker values are not sufficient
   to claim current liveness. The follow-up status change now refreshes the
   snapshot when queried, separates lock-associated `pid` from diagnostic
   `recorded_pid`, and preserves the last PID without calling it live.
5. The absent raw artifacts limit independent verification of the initial
   reboot observation. Preserve measurement/runtime evidence before subsequent
   harness runs in future experiments, using an isolated experiment runtime
   root where the existing harness allows it.

This audit does not select Mode A, Mode B, or a hybrid and does not accept
ADR-0012. No replacement supervision mechanism was implemented.

The follow-up changes address status truthfulness only; they do not identify
why Windows ended the post-login process or add crash supervision.

## Later Shell-Core event review

A later read-only review found Shell-Core Operational events 9707 and 9708 in
the same logon window. Provider metadata identifies their task as
`Explorer_ExecutingFromRunKey`; the 9708 event records `Command=stop` and
`PID=25856`. This is evidence tied to Explorer's HKCU Run command execution,
but it does not by itself establish that PID 25856 exited at that timestamp.
The event text says the Run-key command execution finished and includes the
created PID; it does not by itself prove process exit. In the same logon
sequence, Shell-Core recorded PID 17492 (`GoogleDriveFS.exe`) as having finished
Run-key command execution at 09:00:26 local time. That same PID is still running
with a 09:00:22 start time on this Windows installation, confirming event 9708
records completion of the shell's command launch operation, not the lifetime of
the launched process. No matching cryptexd record was found in the inspected
Application, System, Defender, Code Integrity, or available Security logs for
the time window.

The retained source path was also audited: a successful
`run-mode-a-hkcu --leave-registered` probe previously left its one-shot `stop`
file after stopping the pre-login test daemon. That could make a later login
launch exit immediately if the marker were still present. The physical reboot
record says the marker was absent when inspected and the status still said
`holding instance lock`, so this harness defect is **not established as the
cause of the recorded post-login exit**. It has been fixed, and a separate
isolated Windows run verified the stop marker is removed while the test Run
value remains registered. See `windows-hkcu-stop-marker-cleanup-20261002.md`.

The original post-login exit cause therefore remains OPEN pending a new
instrumented interactive login observation.
