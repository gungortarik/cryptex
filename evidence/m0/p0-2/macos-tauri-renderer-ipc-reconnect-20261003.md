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

## Historical coordinated attempt — NOT YET VERIFIED

On 2026-10-03 UTC, the release renderer and Mode B harness were coordinated
from source `7615b08` on macOS 27.0.1 / aarch64 using experiment
`mac-renderer-crash-wait-20261003-r7`.

The retained harness measurement recorded `restart_success=... pid=23330`,
`restart_latency_ms=30`, and shared-client reconnect `pid=23314 -> 23330`;
the harness exited 0. The visible renderer was invoked with `IPC health ping`
and `Wait for IPC reconnect` during the 15-second pre-crash window. Its final
visible status remained `IPC health.pong · daemon pid=23314 · protocol=1` and
did not display `IPC reconnected after restart · pid 23314 → 23330`.

This run used the pre-crash coordination window but not the post-restart hold;
the replacement daemon's availability window for the renderer was therefore
not controlled. The updated repeat procedure is in
`macos-renderer-crash-reconnect-test-plan-20261003.md`.

Therefore the shared Rust-client result is retained as harness evidence, but
the coordinated Tauri renderer crash/restart requirement remains **NOT YET
VERIFIED**. This does not change the clean stop/replacement renderer proof
above, and does not accept ADR-0012.

## Coordinated supervised-crash repeat — PASS / VERIFIED

**Source:** `dc0ae10c218e0113515f029089bf44a93dcfe3e6`

**Host:** macOS 27.0.1 / aarch64, 2026-10-03 UTC

**Experiment:** `mac-renderer-crash-wait-20261003-r9`

Both release builds passed locally. The visible release Desktop was started
without launching its own daemon. The external Mode B harness used
`--pre-crash-hold-ms 15000 --post-restart-hold-ms 10000`. During the pre-crash
window the renderer invoked health and then `Wait for IPC reconnect`; the
initial ready-file PID was `24966`. The application stayed open until its
visible renderer status, read through macOS accessibility, displayed:

```text
IPC reconnected after restart · pid 24966 → 24987
```

The status appeared after the forced restart and before the 90-second command
timeout. The harness printed `post_restart_hold_started_ms=10000` and exited 0.
The retained measurement contains:

```text
restart_success=AUTOMATED: restarted pid=24987
restart_latency_ms=30
desktop_reconnect_result=AUTOMATED: shared Tauri Desktop IPC client reconnected after supervised restart; pid=24966 -> 24987; actual Tauri UI invocation remains PHYSICAL_MANUAL
errors=
```

The renderer's replacement PID exactly matches both measurement fields.
The raw measurement remains outside Git at
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/renderer-crash-wait-20261003-r9/mode-b/measurements/mac-renderer-crash-wait-20261003-r9.kv`.
Harness control files were cleaned by the harness; Desktop exited 0 after
the renderer's `Quit clean` action.

An earlier repeat (`r8`) reached health after the restart and is excluded from
the crash-before-wait proof. The valid `r9` repeat closes the macOS renderer
crash/reconnect gate. External Mode B owns the tested supervision; production
Desktop supervision, packaging and ADR-0012 acceptance remain separate gates.
