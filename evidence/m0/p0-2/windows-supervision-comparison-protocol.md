# P0-2 Windows supervision comparison protocol (Mode A harness)

**Status:** ACTIVE (comparison / reassessment)  
**ADR-0012:** UNRESOLVED (no winner)  
**Latest Mode A physical:** `windows-mode-a-r1-20261002T052751Z` — **FAILED CANDIDATE / PERMISSION** (`schtasks /Create` Access denied). See `windows-mode-a-task-scheduler-permission-20261002.md`.

## Purpose

Provide a reversible Windows **Mode A** comparison path aligned with macOS native
per-user supervision evidence, without selecting the production Windows startup
primitive and without accepting ADR-0012.

## Privilege distinction (required)

Every Windows candidate must be scored on **two separate axes**:

| Axis | Meaning |
|---|---|
| **Per-user runtime identity** | After start, does the daemon run as the interactive user without elevated rights? |
| **Registration / install privilege** | Does *creating or changing* the startup registration require Administrator / UAC elevation? |

Do not conflate XML `RunLevel=LeastPrivilege` (runtime) with registration privilege.

## Corrected comparison table (P0-2 reassessment, read-only)

| Mechanism | Per-user runtime identity | Registration privilege | Login start | Crash restart / OS supervision | Single-instance fit | Update / rollback practicality | Reversible test install | P0-2 Mode A fit (no-elevated register) |
|---|---|---|---|---|---|---|---|---|
| Task Scheduler logon task (`schtasks` / XML) | Yes (`LeastPrivilege` / InteractiveToken) | **Admin required** (physical r1 + `schtasks` docs) | Yes (`LogonTrigger`) | Yes (`RestartOnFailure`) | Compatible with exclusive `daemon.lock` | Re-point task / re-register; **re-register also needs admin** | Yes (`schtasks /Delete`; may also need admin) | **FAILED for no-admin registration** |
| HKCU `Run` value | Yes | No admin (HKCU write) | Physically verified with ten-minute survival (`windows-reboot-login-survival-20261002.md`) | **No** (one-shot at logon) | Compatible with `daemon.lock`; duplicate probe rejected in reboot proof | Easy rewrite of value / path | Yes (delete matching value) | Login-only candidate verified; crash restart unsupported |
| Startup-folder shortcut | Yes | No admin (user Startup folder) | Yes | **No** (one-shot at logon) | Compatible with `daemon.lock` only | Replace/remove shortcut | Yes | Same as HKCU `Run` |
| Windows Service | Usually service account / session 0 (poor Desktop affinity) | Typically admin to install | Boot/logon depends on config | SCM can restart | Possible but different trust model | Service binary updates often elevate | Possible with admin | Poor fit for Cryptex personal per-user Desktop model |
| Mode B Desktop/tray supervisor | Yes | N/A for OS login item (app must be running) | **Not by itself** (needs separate login start or always-open Desktop) | Yes (process supervisor) | Compatible | App-owned restart / staged binary proofs | N/A / process cleanup | Separate candidate; **not auto-selected** by Task Scheduler failure |
| Hybrid | Depends | Depends | Possible (login item + supervisor) | Possible | Compatible | More moving parts | Depends | Only if evidence justifies; **not selected** |

### Task Scheduler verdict for Cryptex per-user model

- As implemented (`schtasks /Create`): **rejected for the intended no-elevation per-user registration model**.
- Remains theoretically usable **only if** Cryptex later accepts **one-time elevated installation** (installer elevates, task then runs LeastPrivilege). That is a **different product decision** and is **not** proven or accepted here.
- Physical r1 does not exhaust every Task Scheduler API surface (e.g. Task Scheduler 2.0 COM current-user folder), but Microsoft `schtasks` wording and the interactive-session Access denied result are enough to stop treating “Task Scheduler = no admin” as true for this spike.

## Historical incorrect claim (corrected)

Earlier table row:

> Task Scheduler … Per-user, no admin: Yes (`LeastPrivilege`)

That claim is **withdrawn**. It mixed runtime privilege with registration privilege and is contradicted by:

1. `windows-mode-a-r1-20261002T052751Z` physical Access denied
2. Microsoft `schtasks` documentation requiring Administrator membership to schedule/change local tasks

## Test-only identity (harness artifact; not production)

- Task name: `CryptexDevP02Compare` (must not ship as production identity)
- Runtime root: `%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\mode-a`
- Task XML artifact: `LogonTask.xml` under the mode root (never inside the repository)
- Production `registration_status` remains `not_configured`

## Harness operations (still present; registration currently fails without elevation)

Implemented in `services/cryptexd/src/platform/windows.rs` and invoked by
`tools/m0-lifecycle-compare run-mode-a` on Windows:

1. `install` — write task XML under runtime root
2. `register` — `schtasks /Create /TN CryptexDevP02Compare /XML ... /F` (**fails without admin** — see r1)
3. `run_now` — `schtasks /Run` (on-demand; not a login proof)
4. `status` — query registration + `observe_instance`
5. `request_clean_stop` — stop-file handshake
6. `unregister` / `remove` — delete task + local artifacts

Do **not** elevate this harness to make registration pass.

## Evidence classes

- **AUTOMATED:** harness measurements when registration succeeds
- **PHYSICAL_MANUAL:** interactive user logon/reboot after successful registration
- **FAILED CANDIDATE / PERMISSION:** r1 `schtasks /Create` Access denied
- **NOT_YET_VERIFIED:** Windows HKCU Run crash restart; production Desktop-owned supervision; production packaging/update coordination
- The test-only Windows reboot/login start and ten-minute survival are PHYSICALLY VERIFIED (`windows-reboot-login-survival-20261002.md`). The Tauri renderer's reconnect during external Mode B crash/restart is VERIFIED (`windows-tauri-renderer-ipc-reconnect-20261003.md`).

## HKCU Run login-start probe (IMPLEMENTED — test-only)

**Status:** login/reboot start and ten-minute survival PHYSICALLY VERIFIED in the separate evidence below
**Mode string:** `native_hkcu_run`  
**Runtime root:** `%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\mode-a-hkcu`  
**Registry:** `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value `CryptexDevP02HkcuRun`  
**OS crash restart:** **unsupported / NOT_YET_VERIFIED** for this candidate (explicit; do not claim KeepAlive parity)

### Harness commands

```text
m0-lifecycle-compare run-mode-a-hkcu --experiment-id <id> --cryptexd <path> --branch m0/p0-2-cursor --commit <sha>
m0-lifecycle-compare run-mode-a-hkcu ... --leave-registered
m0-lifecycle-compare hkcu-run register --cryptexd <path>
m0-lifecycle-compare hkcu-run status --cryptexd <path>
m0-lifecycle-compare hkcu-run remove --cryptexd <path>
m0-lifecycle-compare cleanup --runtime-root <compare-root> --cryptexd <path>
```

`run-mode-a-hkcu` measures no-elevation register, on-demand hold (not login), duplicate reject, clean stop, and scoped unregister. It does **not** sign out/reboot and does **not** claim OS crash restart.

### PHYSICAL_MANUAL login test procedure (do not automate logout/reboot)

1. Build: `cargo build -p cryptexd -p m0-lifecycle-compare --locked`
2. Register only (leave installed):  
   `m0-lifecycle-compare hkcu-run register --cryptexd <path-to-cryptexd.exe>`  
   Confirm: `hkcu-run status` shows `registered=true` and the expected command line under `mode-a-hkcu`.
3. Sign out and sign back in **manually** (or one controlled reboot). Harness must not force this.
4. After login, verify:
   - `hkcu-run status` → `daemon_lock_held=true` (or `lifecycle status --runtime-root %LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\mode-a-hkcu` shows `health=active`)
   - `ready` / `lifecycle.status` under the mode-a-hkcu runtime root
5. Record PHYSICAL_MANUAL evidence (login start VERIFIED; crash restart still unsupported).
6. Remove only the test value: `hkcu-run remove` (or `cleanup`). Confirm unrelated HKCU Run values remain.

The verified physical reboot/login result is recorded in
`windows-reboot-login-survival-20261002.md`. The historical instrumented login
dispatch failure and its later diagnostics are retained in
`windows-instrumented-login-result-20261002.md` and
`windows-run-dispatch-followup-20261002.md`; they do not supersede the later
successful test-only reboot/login proof. Startup-folder and Task Scheduler COM
remain **not implemented**. ADR-0012 remains unresolved; Mode A/B/hybrid are not
selected.

**Optional later exhaustiveness probe:** Task Scheduler 2.0 / current-user COM registration without `schtasks`, only if needed. Do not elevate `schtasks` to force green.

## Safety rules

- No UAC bypass, no forced elevation for Mode A proofs
- Runtime only under `Cryptex-Dev/p0-2`
- Never touch `syncthing-p0-1*`
- Do not accept ADR-0012 from this protocol alone
- Do not select Mode A or Mode B from a single failed candidate

Historical note: older harness text deferring login proofs "while P0-1 active" is stale; P0-1 is complete.
