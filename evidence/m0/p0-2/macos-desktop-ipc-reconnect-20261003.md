# macOS Desktop IPC reconnect proof

**Date:** 2026-10-03 UTC

**Host:** macOS 27.0.1 / aarch64

**Branch:** `m0/p0-2-cursor`

**Source commit:** `70b85ad64b88ed57920be1059cd07fadaa31fee7`

**Experiment:** `macos-ipc-e2e-20261003T230000Z`
**Classification:** AUTOMATED physical macOS evidence

## Result

The comparison harness started the real `cryptexd` process under the
`DesktopSupervisor` lifecycle, called the shared
`cryptexd::ipc::desktop_health_ping` client used by the Tauri command, forced a
daemon crash, and verified a fresh health response from the restarted daemon.

| Observation | Result |
|---|---|
| Runtime root | `~/Library/Application Support/Cryptex-Dev/p0-2/compare/mode-b` |
| Supervisor PID | `17118` |
| Daemon before crash | `17128` |
| Crash observed / lock released | PASS |
| Restarted daemon PID | `17130` |
| Restart latency | `26 ms` |
| Desktop health IPC before and after restart | PASS; `17128 -> 17130` |
| Duplicate daemon probe | rejected, exit 1 |
| Clean stop and lock release | PASS |
| Runtime control files | cleaned; measurement retained outside repository |
| Interactive login/reboot | not performed by this harness |
| Actual Tauri renderer invocation | not performed |

The retained measurement is:

```text
~/Library/Application Support/Cryptex-Dev/p0-2/compare/mode-b/measurements/macos-ipc-e2e-20261003T230000Z.kv
```

## Scope and limits

This physically verifies the production daemon transport and shared Desktop
Rust client reconnect on macOS across a supervised daemon restart. It does not
verify a visible Tauri UI button invoking the command, literal traffic-light
interaction, production packaging, or ADR-0012 acceptance. Those remain
separate gates.
