# Windows Desktop IPC reconnect proof

Date: 2026-10-02 EDT / 2026-10-03 UTC  
Source commit: `c5e059d`  
Experiment: `windows-ipc-e2e-20261002T225603Z`  
Environment: physical Windows host, x86_64

## Result

The real `cryptexd` process started under the `DesktopSupervisor` lifecycle
comparison harness and bound the production `interprocess` health endpoint.
The harness called the same `cryptexd::ipc::desktop_health_ping` function used
by the Tauri command before and after forcing a daemon crash and restarting it.

| Observation | Result |
|---|---|
| Runtime root | `%LOCALAPPDATA%\Cryptex-Dev\p0-2\ipc-e2e-20261002T225603Z\mode-b` |
| Supervisor PID | `27756` |
| Daemon before crash | `2880` |
| Crash observed / lock released | PASS |
| Restarted daemon PID | `24780` |
| Restart latency | `32 ms` |
| Desktop health IPC before and after restart | PASS; `2880 -> 24780` |
| Duplicate daemon probe | rejected, exit 1 |
| Clean stop and lock release | PASS |
| Runtime control files | cleaned; measurement retained outside repository |
| GitHub Actions | not run |

The retained measurement is:

```text
%LOCALAPPDATA%\Cryptex-Dev\p0-2\ipc-e2e-20261002T225603Z\mode-b\measurements\windows-ipc-e2e-20261002T225603Z.kv
```

## Scope

This is physical Windows evidence for the real daemon transport, the shared
Desktop Rust IPC client function, and reconnection after a supervised process
restart. The Tauri window/UI itself was not launched for this run; the Tauri
command-to-renderer invocation and visible UI result remain `PHYSICAL_MANUAL`.
Do not describe this as a full UI reconnect proof or use it to accept ADR-0012.
The earlier `wl-1003h` login daemon and registry registrations were not touched.
