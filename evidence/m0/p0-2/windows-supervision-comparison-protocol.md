# P0-2 Windows supervision comparison protocol (Mode A harness)

**Status:** ACTIVE (test harness only)  
**ADR-0012:** UNRESOLVED (no winner)

## Purpose

Provide a reversible Windows **Mode A** comparison path aligned with macOS native
per-user supervision evidence, without selecting the production Windows startup primitive.

## Spike mechanism choice (P0-2 harness only)

| Mechanism | Per-user, no admin | Login start | Crash restart / supervision | Reversible test install | Notes |
|---|---|---|---|---|---|
| **Task Scheduler logon task** | Yes (`LeastPrivilege`) | Yes (`LogonTrigger`) | Yes (`RestartOnFailure`) | Yes (`schtasks /Delete`) | **Selected for Mode A harness** |
| HKCU `Run` value | Yes | Yes | No (one-shot at logon) | Yes | Poor parity with macOS KeepAlive |
| Startup folder shortcut | Yes | Yes | No | Yes | Same limitation as `Run` |

Production may still choose differently after ADR-0012; this document covers the **M0 spike harness** only.

## Test-only identity

- Task name: `CryptexDevP02Compare` (must not ship as production identity)
- Runtime root: `%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\mode-a`
- Task XML artifact: `LogonTask.xml` under the mode root (never inside the repository)

## Harness operations

Implemented in `services/cryptexd/src/platform/windows.rs` and invoked by
`tools/m0-lifecycle-compare run-mode-a` on Windows:

1. `install` — write task XML under runtime root
2. `register` — `schtasks /Create /TN CryptexDevP02Compare /XML ... /F`
3. `run_now` — `schtasks /Run` (on-demand start; does **not** substitute for login proof)
4. `status` — query registration + `observe_instance`
5. `request_clean_stop` — stop-file handshake
6. `unregister` / `remove` — delete task + local artifacts

## Evidence classes

- **AUTOMATED:** `run-mode-a` measurements (ready, duplicate reject, crash/restart via RestartOnFailure, clean stop, cleanup)
- **PHYSICAL_MANUAL:** interactive user logon/reboot after leaving registration installed
- **NOT_YET VERIFIED:** physical Tauri GUI window-close, full Desktop IPC reconnect, production packaging

## Manual physical login proof (required next)

Do **not** reboot automatically from harness scripts.

1. Build: `cargo build -p cryptexd -p m0-lifecycle-compare`
2. Run automated compare (optional baseline):  
   `m0-lifecycle-compare run-mode-a --experiment-id <id> --cryptexd <path> --branch m0/p0-2-cursor --commit <sha>`
3. For login proof, install/register only (skip final remove), e.g. run harness steps manually or stop before teardown.
4. Sign out and sign back in (or reboot once during a controlled window).
5. Verify under `%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\mode-a`:
   - `schtasks /Query /TN CryptexDevP02Compare` succeeds
   - `daemon.lock` held / `lifecycle.status` shows `health=active`
6. Record PHYSICAL_MANUAL evidence; then `m0-lifecycle-compare cleanup --runtime-root ... --cryptexd ...`

Historical note: older harness text deferring login proofs "while P0-1 active" is stale; P0-1 is complete. Login/reboot remains OPEN because it was not executed, not because P0-1 blocks it.