# P0-2 Windows — parent-process exit control

**Date:** 2026-10-02  
**Platform:** Windows x86_64 physical workstation  
**Branch:** `m0/p0-2-cursor`  
**Experiment:** `windows-parent-exit-20261002`  
**Status:** VERIFIED for direct child-process survival after its launcher shell exits

## Method and observation

From a one-shot PowerShell process, launched the built `cryptexd.exe lifecycle
hold` with an isolated runtime root under `%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare`.
The launcher process exited after receiving ready PID `32196`. In a separate
PowerShell process, `Get-Process` found `cryptexd` PID `32196`; a fresh lifecycle
status query reported `pid=32196`, `instance=held`, and `health=active`.

The daemon was then stopped through its stop file. A fresh status query reported
`pid=none`, `recorded_pid=32196`, and `instance=stale`, confirming clean lock
release. The retained KV measurement is stored outside the repository alongside
the isolated runtime data.

## Limits

This control shows that a normal child `cryptexd` process survives termination
of its launching PowerShell process. It does not reproduce the Windows HKCU Run
startup path, a logoff/login or reboot, or the earlier post-login exit. The
specific exit cause remains unknown; this evidence does not select a supervision
candidate or accept ADR-0012.
