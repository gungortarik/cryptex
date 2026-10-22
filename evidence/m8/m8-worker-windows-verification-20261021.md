# M8 Windows managed worker verification

Date: 2026-10-21. Windows NT 10.0.26300.0 X64. Isolated worktree
`C:/Users/Tarik/.codex/worktrees/m8-device-trust/cryptex`, branch
`codex/m8-device-trust`, uncommitted base
`74bc3b14127962579ae03838bafea9336612a033`.

## Final-source evidence

Proof: `C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/worker-26260879df9e42f1a7417a2be629492a`.
New normalized-LF manifest: `m8-worker-source-sha256-20261021.txt`, **32/32**
verified. Prior manifests/packages remain unchanged. Debug daemon SHA-256:
`6696c0bad4ba5f90b91a0495132a3b0367ecf5e3c81e432a5a2e6dfd62c047ec`.

| Gate | Final result |
|---|---|
| fmt | PASS |
| strict daemon all-target Clippy | PASS |
| `cargo test --locked -p cryptexd -p cryptex-events` | events 61; daemon library 386 / 4 ignored; CLI 8; lifecycle 9; Mode B 3; update/rollback 2 PASS |
| All four explicit native fixtures, serial | PASS |
| daemon build | PASS |
| actual Windows PowerShell 5.1 runner-helper regression | 3/3 PASS |
| source guard / diff check | 32/32 / PASS |

Seven worker tests cover cancellation/idempotent join, deadline retaining the
handle for later join, typed terminal failure, invalid deadline, sticky
cancellation under later publication, unwind and poisoned diagnostics with
continued cancellation/join. No existing assertion or SecureStore behavior
was weakened. The previously recorded intermittent credential/worker failures
remain OPEN; these passes do not establish their cause or a fix.

## Real synthetic native worker

The existing prepared profile was released by the fixture and reopened inside
the new worker thread. Owned process **35728** reached authenticated LocalReady,
then shutdown joined with `CleanShutdown`; a repeated shutdown returned the
same result. Final snapshot was Finished. A second worker while the first
owned the profile failed with exact `Profile(Busy)` before launching.

After joining, the original fixture reopened the profile. Removing only its
synthetic completed native receipt caused reopen-only to return InvalidState;
the missing receipt was not recreated. The exact original bytes were restored
through CREATE_NEW for remaining checks. This is controlled local fault
injection, not canonical security-state creation or a production repair API.

Earlier actual controller cases were rerun on this source: forced whole-job
exit -> replacement -> requested clean stop, backoff cancellation and four-start
exhaustion; native identity stayed unchanged. Wrong API keys remained 403,
foreign endpoint refusal preceded HTTP bytes, protected loopback configuration
remained checked and exact synthetic OS-credential deletion verified Missing.
Public receipts are copied into the proof directory; secrets/log bodies are
not included in the repository. Final Syncthing process count **0**.

## Limits and continuation

This verifies the explicitly invoked backend worker, not a default daemon
startup configuration or status IPC endpoint. Drop/deadline behavior is tested
with local tasks; a blocked real native worker/drop and abrupt whole-daemon
death are not newly claimed. Mac remains later. The updated batch runner
references the new manifest; its individual commands/helper were run, not the
whole batch as one invocation. No peer trust, incoming remote admission,
Actions, commit, push, merge or M8 completion. Original dirty checkouts untouched.
