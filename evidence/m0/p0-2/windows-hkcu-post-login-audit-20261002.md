# P0-2 Windows HKCU Run — post-login exit audit

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
- A fresh `observe_instance` call probes the lock. When the lock is free but
  `daemon.lock` remains, the recorded PID is returned even though it is not a
  live holder. `LifecycleReport::from_observation` marks that state `stale` /
  `attention`, but its `pid` field still contains the recorded PID.
- `hkcu-run status` reports `daemon_lock_held` separately from `daemon_pid`;
  that PID may be the stale recorded PID. `ready_present` likewise reports
  marker presence, not process liveness.

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
4. Persisted `health=active`, `daemon_pid`, and `ready_present` values are not
   sufficient to claim current liveness. Current status reporting needs to
   distinguish a live lock holder from retained last-run metadata.
5. The absent raw artifacts limit independent verification of the initial
   reboot observation. Preserve measurement/runtime evidence before subsequent
   harness runs in future experiments, using an isolated experiment runtime
   root where the existing harness allows it.

This audit does not select Mode A, Mode B, or a hybrid and does not accept
ADR-0012. No replacement supervision mechanism was implemented.
