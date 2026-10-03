# macOS Tauri renderer IPC reconnect proof

**Date:** 2026-10-03 UTC

**Host:** macOS 27.0.1 / aarch64

**Branch:** `m0/p0-2-cursor`

**Source:** current branch at `6cd5b3b`

**Classification:** PHYSICAL_MANUAL Tauri UI evidence

## Result

The real Tauri Desktop development application was launched on the physical
Mac. Through the visible renderer UI, the test entered the repository-built
`cryptexd` path, started the daemon, invoked the IPC health command, stopped
the daemon, started a replacement daemon, and invoked the reconnect command.

| UI action | Observed result |
|---|---|
| `Start probe daemon` | PASS; renderer reported `daemon started pid=18700` |
| `IPC health ping` | PASS; renderer reported `IPC health.pong · daemon pid=18700 · protocol=1` |
| `Stop daemon` | PASS; daemon stop requested through the renderer command |
| Replacement `Start probe daemon` | PASS; fresh daemon process started |
| `Wait for IPC reconnect` | PASS; renderer reported `IPC reconnected after restart · pid 18700 → 18750` |
| Final cleanup | PASS; daemon stopped and Desktop exited through `Quit clean` |

The UI result was visually confirmed in the Cryptex window after the health
ping and after the reconnect command. Runtime data remained under the
test-only `Cryptex-Dev/p0-2/compare/mode-b-desktop` root outside the repository.

## Scope and limits

This verifies the Tauri renderer-to-command path for health and reconnect on a
physical macOS session, using the real daemon transport and a replacement PID.
It does not verify literal traffic-light interaction, production packaging,
or accept ADR-0012. Those remain separate gates.
