# P0-2 Windows instrumented login result

Date: 2026-10-02. Experiment: `wl-1002a`. Preparation/source commit:
`6910f7810e3349cf382f78de80fc182827c9689b`.

## Result

Windows records confirm a regular user profile logon at
`2026-10-02T16:41:48.221-04:00` (session 2). User Profile Service emitted IDs 1
(logon notification received), 67 (regular local profile), and 2 (logon
notification finished). The Windows host did not reboot; its boot time remained
`2026-10-02T08:59:34.500-04:00`.

At `2026-10-02T16:53:14.209-04:00`, 11.4 minutes after that confirmed logon:

```text
registered_daemon=true
registered_observer=true
daemon_command_length=182
observer_command_length=207
ready=false
daemon.lock=false
lifecycle.status=false
stop=false
observations_directory=false
cryptexd_process=false
```

The values were present in the raw 64-bit HKCU Run registry before cleanup and
matched the prepared manifest. StartupApproved contained no entries for either
test value. The 64-bit registry query confirmed the current compact command
strings; WMI `Win32_StartupCommand` reported the older command from the prior
test runtime for the same value name, so that WMI result was stale/inconsistent
and is not used as launch evidence.

Shell-Core logged 12 `Explorer_ExecutingFromRunKey` start/stop events between
16:41:55 and 16:42:05 for other startup commands; none matched either Cryptex
test value. Code Integrity had no matching daemon or observer event after the
confirmed logon. This establishes **no observed Cryptex start during the
11.4-minute window**. It does not prove why Windows did not dispatch the Run
values. The observer never created its output directory, so it could not attach
to the daemon or report an exit code.

Classification: **LOGIN START NOT OBSERVED / CAUSE OPEN**. This attempt is not a
daemon-survival result and does not explain the historical PID 25856 disappearance.
Do not infer that the process started and exited. The Run command stayed within
the documented length; Smart App Control/code-integrity blocking was not
observed for either test executable/script. Neither finding explains the missing
launch.

## Cleanup

After the 11.4-minute window, the reversible Cleanup action verified no ready
PID/process and removed both test Run values. A following Status action reported:

```text
daemon_registered=false
observer_registered=false
stop_present=false
```

The pinned daemon, observer, manifest and empty runtime directory remain under
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\wl-1002a` for inspection. No registry or
runtime data was deleted outside the two test values and the absent stop file.

## Next step

Diagnose the Windows login-start dispatch gap before drawing a conclusion about
daemon survival. Preserve the exact command lines, current-user registry view,
shell events and process-start evidence in the next isolated candidate test.
Do not select a supervision strategy or accept ADR-0012 from this result.

Full Desktop IPC reconnect also remains open: the current Desktop obtains daemon
status through a short-lived CLI process, and there is no P0-3 named-pipe
reconnect handshake to test yet.
