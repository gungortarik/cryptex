# P0-2 macOS supervision comparison protocol

**Status:** ACTIVE  
**ADR-0012:** UNRESOLVED (no winner)

## Purpose

Deterministic comparison contract for:

- **Mode A** — native per-user supervision candidate (test-only LaunchAgent)
- **Mode B** — Desktop/tray-sidecar supervision candidate (process supervisor + optional Tauri hide-on-close hooks)

This protocol measures behavior. It does not select architecture.

## Authority

- `docs/11_DAEMON_IPC_RUNTIME.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/26_VERSIONING_MIGRATIONS_AND_UPDATES.md`
- `docs/milestones/M0_FOUNDATION.md`

## Evidence classes

Every field must be tagged as one of:

| Class | Meaning |
|---|---|
| `AUTOMATED` | Reproducible harness/process measurement |
| `PHYSICAL_MANUAL` | Observed on a real interactive Desktop/session action |
| `NOT_YET_VERIFIED` | Required by P0-2 but not proven in this experiment |

Do not fabricate values. Prefer `NOT_YET_VERIFIED` over inference.

## Required measurement fields

Captured as KV under `Cryptex-Dev/p0-2/compare/mode-{a|b}/measurements/<experiment_id>.kv`:

- experiment_id, macOS version, architecture, branch/commit
- runtime_root, supervisor_mode
- daemon_pid, supervisor_pid
- startup_unix_ms, time_to_daemon_ready_ms
- duplicate_launch_result
- main_window_close_result, daemon_alive_after_window_close
- forced_crash_unix_ms, crash_detected
- restart_attempted, restart_success, restart_latency_ms
- stale_lock_recovery
- clean_shutdown_result, runtime_cleanup_result
- login_startup_result, desktop_reconnect_result
- errors, evidence_class_notes

## Commands

Build binaries, then:

```bash
cargo build -p cryptexd -p m0-lifecycle-compare
CRYPTEXD="$(cargo metadata --locked --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/debug/cryptexd"
COMPARE="$(dirname "$CRYPTEXD")/m0-lifecycle-compare"
RUNTIME="$HOME/Library/Application Support/Cryptex-Dev/p0-2/compare"

"$COMPARE" run-mode-a --experiment-id <id> --runtime-root "$RUNTIME" --cryptexd "$CRYPTEXD" --branch m0/p0-2-cursor --commit <sha>
"$COMPARE" run-mode-b --experiment-id <id> --runtime-root "$RUNTIME" --cryptexd "$CRYPTEXD" --branch m0/p0-2-cursor --commit <sha>
"$COMPARE" cleanup --runtime-root "$RUNTIME" --cryptexd "$CRYPTEXD"
```

## Safety rules

- Runtime only under `Cryptex-Dev/p0-2/compare`
- Never touch `syncthing-p0-1*`
- Login/reboot startup proofs remain required and OPEN; do not treat historical "while P0-1 active" harness notes as a current blocker (P0-1 is complete)
- Mode A LaunchAgent label is test-only: `com.cryptex.dev.p0-2.compare`
- Always `cleanup` / `bootout`+`remove` after Mode A
- Do not accept ADR-0012 from this protocol alone

## What this slice intentionally leaves open

- login/reboot startup
- full Desktop IPC reconnect after daemon restart
- interactive Tauri window-close physical proof (hooks exist; interactive GUI run may still be pending)
- Windows Mode B AUTOMATED process-supervisor foundation is recorded in `windows-desktop-supervision-evidence.md`; Windows Mode A native startup, physical Tauri GUI close, login/reboot, and full Desktop IPC reconnect remain OPEN
- production updater/rollback orchestration beyond synthetic marker fixtures
