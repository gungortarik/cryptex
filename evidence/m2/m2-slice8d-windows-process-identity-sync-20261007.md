# M2 Slice 8d — Windows process-identity test lifetime sync

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `PENDING`

## Prior CI (near-final)

- GitHub Actions run: [37653004440](https://github.com/gungortarik/cryptex/actions/runs/37653004440)
- **PASS:** macOS readiness (complete); Windows M2 crate tests, schema drift, workspace Clippy, and earlier readiness steps; Windows IPC ACL
- **FAIL:** Windows Node test `platform adapter proves own executable and parent without command-line capture` in `package-desktop-session.test.mjs`
  - PowerShell `Get-CimInstance Win32_Process -Filter 'ProcessId = …'` failed because the queried PID was no longer present when CIM ran

## Diagnosis

- The test called `verifyOwnedProcess(process.pid, process.ppid, process.execPath)`, racing the Windows CIM query against process lifetime observability for a PID that is not held open by the harness as a dedicated observee.
- Product adapter (`package-process-identity.mjs`) remains read-only and unchanged; assertions (pid/parent match, no `command_line` field) are unchanged in strength.

## Fix (test harness only)

- Spawn a child Node that blocks on `process.stdin.resume()` with an open stdin pipe.
- Wait for a real `child.pid`, prove liveness with `process.kill(pid, 0)`, then run `verifyOwnedProcess(child.pid, process.pid, process.execPath)`.
- Re-check liveness after assertions; only then close stdin / SIGKILL and await `close`.
- No assertion weakening; no CIM retries that hide missing processes.

## Local verification (Windows)

- Focused test ×15 — PASS (0 failures)
- Full `package-desktop-session.test.mjs` — PASS (7)

## Non-goals

- Dispatching Actions
- Changing Windows CIM adapter production behavior beyond the harness

## Next

- Owner review, then **one** more dual-OS `ci.yml` readiness run on this tip for the final green gate.
