# P0-2 Windows Mode A — HKCU Run automated probe

**Date (UTC):** 2026-10-02T18:06:07Z
**Host:** Windows physical machine / x86_64
**Branch:** `m0/p0-2-cursor`
**Commit:** `34073a6`
**Experiment ID:** `windows-mode-a-hkcu-r1-20261002T180600Z`
**Mode:** `native_hkcu_run`
**Classification:** AUTOMATED physical-host harness evidence; interactive login remains open

## Result

Ran `m0-lifecycle-compare run-mode-a-hkcu` with the built `cryptexd.exe` on the
Windows workstation. The test-only HKCU Run registration path completed and
was removed by the harness. No sign-out or reboot was performed.

| Field | Result | Class |
|---|---|---|
| daemon ready | 56 ms | AUTOMATED / VERIFIED |
| duplicate launch | rejected, exit 1 | AUTOMATED / VERIFIED |
| clean shutdown | stop-file shutdown succeeded | AUTOMATED / VERIFIED |
| test registry value cleanup | removed | AUTOMATED / VERIFIED |
| interactive login start | not performed | PHYSICAL_MANUAL / NOT YET VERIFIED |
| OS crash restart | unsupported by HKCU Run | NOT APPLICABLE |
| full Desktop IPC reconnect | not performed | NOT YET VERIFIED |

Measurement KV is retained outside the repository under:

`%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\mode-a-hkcu\measurements\windows-mode-a-hkcu-r1-20261002T180600Z.kv`

## Reproduction

```powershell
cargo build -p cryptexd -p m0-lifecycle-compare --locked
cargo run -p m0-lifecycle-compare --locked -- run-mode-a-hkcu `
  --experiment-id windows-mode-a-hkcu-r1-20261002T180600Z `
  --cryptexd target/debug/cryptexd.exe `
  --branch m0/p0-2-cursor --commit 34073a6
```

## Limits and next physical check

This verifies registration/run/cleanup behavior of the harness on this Windows
host. It does not prove that Windows starts the daemon at interactive login.
That proof requires manually registering with `hkcu-run register`, signing out
and back in (or a controlled reboot), checking the runtime status, and removing
only the test value with `hkcu-run remove`. Do not automate sign-out or reboot.

HKCU Run is a one-shot login launcher and does not supervise crashes. It does
not establish a Mode A/Mode B winner and does not resolve ADR-0012.
