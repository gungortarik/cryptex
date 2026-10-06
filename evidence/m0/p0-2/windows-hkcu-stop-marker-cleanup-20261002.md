# P0-2 Windows — HKCU Run stop-marker cleanup regression

**Date:** 2026-10-02  
**Platform:** Windows x86_64 physical workstation  
**Branch:** `m0/p0-2-cursor`  
**Commit under test:** `0f4f518`  
**Experiment:** `windows-stop-marker-cleanup-20261002T`  
**Status:** VERIFIED for the isolated `--leave-registered` harness path

## Defect and correction

`run-mode-a-hkcu --leave-registered` registers the test-only HKCU Run command,
launches the daemon on demand, and stops that probe by writing the same `stop`
file later passed to the login command. The harness previously left that
one-shot marker in place. If a login occurred before the marker was removed,
the newly launched daemon could see the stale stop request and exit cleanly.

The harness now removes the stop marker after waiting for the on-demand daemon
to exit, even when the HKCU value is intentionally left registered. Runtime
control-file deletion reports filesystem failures, and the probe process is
reaped if the clean-stop deadline expires.

## Physical Windows run

Ran `run-mode-a-hkcu --leave-registered` against an isolated runtime under:

```text
%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\windows-stop-marker-cleanup-20261002\mode-a-hkcu
```

Observed:

- on-demand daemon became ready in 49 ms (PID 29200)
- duplicate daemon launch was rejected with exit code 1
- clean stop released the lock; final lifecycle status was `pid=none`,
  `recorded_pid=29200`, `instance=stale`
- after the harness completed, the HKCU test value was present and the stop
  marker was absent
- explicitly removed the HKCU test value immediately after inspection; it was
  absent afterward
- did not sign out or reboot during this isolated regression run

The KV measurement is retained outside the repository in the runtime root.

A separate isolated control with a pre-existing `stop` file showed that
`lifecycle hold` exits immediately and rewrites `lifecycle.status` to
`pid=none`, `instance=stale`, and `note=clean shutdown released instance lock`.
The recorded reboot artifact instead retained `instance=held` and
`note=holding instance lock for harness`; the stale-stop defect therefore does
not match the historical artifact signature.

## Limits

This fixes a reproducible harness defect that could invalidate a subsequent
login-start trial. It does not explain the earlier reboot observation: that
record showed the stop marker absent at inspection and retained an active
status snapshot. The historical post-login exit cause remains OPEN. This proof
does not select a supervision candidate or accept ADR-0012.
