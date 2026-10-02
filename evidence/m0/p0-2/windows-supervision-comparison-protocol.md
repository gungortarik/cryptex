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
| HKCU `Run` value | Yes | No admin (HKCU write) | Yes | **No** (one-shot at logon) | Compatible with `daemon.lock` only | Easy rewrite of value / path | Yes (delete value) | Candidate for *login registration* only; KeepAlive gap remains |
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
- **NOT_YET_VERIFIED:** Mode A lifecycle after registration; physical Tauri GUI close; full Desktop IPC reconnect; production packaging

## Recommended next experiment (DO NOT IMPLEMENT YET)

**Preferred next P0-2 probe:** a **read-only-designed, reversible HKCU `Run` (or Startup-folder) registration experiment** that measures only:

1. registration without elevation
2. login start
3. coexistence with exclusive `daemon.lock`
4. clean unregister

Explicitly leave **OS-level crash restart** as a known gap for that candidate (parity with macOS KeepAlive would then require Mode B, hybrid, or another mechanism — undecided).

**Optional later exhaustiveness probe (also not implemented here):** Task Scheduler 2.0 / current-user COM registration without `schtasks`, only if needed to close the “tool vs API” question. Do not elevate `schtasks` to force green.

## Safety rules

- No UAC bypass, no forced elevation for Mode A proofs
- Runtime only under `Cryptex-Dev/p0-2`
- Never touch `syncthing-p0-1*`
- Do not accept ADR-0012 from this protocol alone
- Do not select Mode A or Mode B from a single failed candidate

Historical note: older harness text deferring login proofs "while P0-1 active" is stale; P0-1 is complete.