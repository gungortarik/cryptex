# Windows reboot/login daemon survival

Date: 2026-10-02 EDT. Experiment: `wl-1003h`.
Status: VERIFIED for the test-only HKCU Run reboot/login startup and ten-minute
post-login survival observation.

## Preparation and isolation

The user prepared the experiment from ordinary Windows PowerShell at
`2026-10-03T02:02:24.5961792Z`, source `0a7eb0e`, clean working tree.
The exact legacy comparison Run command was backed up at
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\registration-transitions\5e2ed140a7d5485b88f1dd76422f30bc\legacy-run.json`
before replacement. Legacy runtime files, including its retained stop marker,
were preserved. Fresh runtime:
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\wl-1003h\mode-a-hkcu`.

The preparation manifest pins:

- `cryptexd.exe` SHA-256:
  `EDA29D5001CAFCA9091893696E8C46FC4436702152810A9CF764EDD9D0E18DB2`.
- Observer SHA-256:
  `49B77FCB4570D8013044D74A5C4B39F06F0AFDBA92A842C48A5EB591780AA10E`.
- Daemon Run value: `CryptexDevP02HkcuRun`.
- Observer Run value: `CryptexDevP02LoginObserver`.
- Observation window: 600 seconds.

No manual daemon start was performed in this fresh runtime. No GitHub workflow
was dispatched; P0-1 runtime and canonical data were untouched.

## Physical reboot/login correlation

The user restarted Windows and signed into Tarik. `Win32_OperatingSystem`
reported last boot at 22:06:04 EDT; `quser` reported a new console session 1
with the 22:06 login.

| Observation | UTC timestamp / value |
|---|---|
| Daemon process start | `2026-10-03T02:07:32.4958933Z`, PID 31512 |
| Ready marker | `2026-10-03T02:07:32.9645975Z` |
| Daemon Shell-Core 9708 | record 3689, `02:07:33.3770542Z`, PID 31512 |
| Observer process start | `02:07:33.3936336Z`, PID 31192 |
| Observer log start | `02:07:33.9499372Z` |
| Observer Shell-Core 9708 | record 3691, `02:07:34.2702028Z`, PID 31192 |

Event 9708 records launch-operation completion, not process termination.
The observer attached to the daemon after verifying the executable and process
start time. It retained the OS process handle and reports periodic heartbeats.
The held-lock snapshot read failure was explicitly recorded; it did not prevent
attachment. Initial/attached stop marker: absent.

## Duplicate instance control

At `2026-10-03T02:09:19.9086151Z`, a probe using the pinned binary and same runtime
exited 1 with `daemon instance already running (pid 31512)`. No new holder was
started. The observer continued to report the original attached daemon.

## Final observation

Retained result under:
`observations/20261003T020733884Z-16b25704/result.json`.

```json
{
  "outcome": "alive_at_observation_deadline",
  "exit_code": null,
  "exit_hex": null,
  "started_utc": "2026-10-03T02:07:33.8844292Z",
  "ended_utc": "2026-10-03T02:17:34.9596389Z",
  "login_proof": "NOT_ESTABLISHED_BY_OBSERVER_ALONE"
}
```

The final heartbeat reports 600.81 elapsed seconds, attached to the same
daemon, stop absent. After the observer completed, a direct process query
confirmed PID 31512 still running with the pinned executable and original
start time. Matching Code Integrity events: zero. Held-lock snapshot failures
remain explicit in the initial/final logs.

The observer alone does not establish login provenance; the physical reboot,
new console session, fresh ready timestamp and matching Shell-Core PIDs provide
that correlation. These together verify reboot/login start and survival for
the measured window. They do not guarantee indefinite uptime.

The daemon and test startup registrations remain in place pending the next
controlled lifecycle step. No cleanup or stop was performed in this proof.
This experiment does not provide HKCU
crash restart, full Desktop IPC reconnect, production installation, or ADR-0012
acceptance. The older 13:00 UTC disappearance remains a separate unresolved
historical run; the legacy stop-marker finding must not be applied to it.
