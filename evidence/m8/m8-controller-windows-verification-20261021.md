# M8 Windows isolated controller verification

Date: 2026-10-21. Host Windows NT 10.0.26300.0 X64.
Isolated worktree `C:/Users/Tarik/.codex/worktrees/m8-device-trust/cryptex`,
branch `codex/m8-device-trust`, base `74bc3b14127962579ae03838bafea9336612a033`.
Source remains uncommitted; original dirty checkouts were not changed.

## Exact candidate and proof

Normalized LF source inventory: `m8-controller-source-sha256-20261021.txt`,
31 files. Prior source manifests and sealed packages were not rewritten.
Build target: `%LOCALAPPDATA%/Cryptex-Dev/build/m8-device-trust-target`.
Debug daemon SHA-256:
`b4e9545e720dd91a5f1af03c2375351594a9cc04cddc73e9b7aff84ca5e646bd`.

Raw logs and four native result receipts:
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/controller-812a612863224252ab8edc7d54404fa6`.

## Final verification

| Command / scope | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` | PASS |
| `cargo test --locked -p cryptexd -p cryptex-events` | PASS: events 61; daemon library 379 / 4 ignored; CLI 8; lifecycle 9; Mode B 3; update/rollback 2 |
| `cargo test --locked -p cryptexd --lib -- --ignored --test-threads=1` | All four explicit native fixtures PASS |
| `cargo build --locked -p cryptexd` | PASS |
| Actual Windows PowerShell 5.1 runner-helper regression | PASS 3/3 |
| `git diff --check` | PASS |

Eight new deterministic controller tests are included in the 379 library tests.
They verify finite 1/2/4-second replacement scheduling using an injected wait,
not a real-time native crash experiment. Existing assertions were not weakened.

The real product-provisioning fixture preserved native identity across three
existing direct starts (PIDs 13192/39304/34008), then added two controller
start/health/clean-stop cycles (PIDs **20432/13732**). Both returned
`Outcome::CleanShutdown` and released in-use state without unresolved cleanup.
Actual wrong-key checks remained 403; foreign API-port collision refused before
API bytes; explicit retry after collision remained clean. Synthetic OS
credential deletion was verified. Final Syncthing process count: **0**.

## Failure retained, not explained away

The initial broad run (`tests.log`) failed in the existing
`abrupt_exit_ready_windows_store_reopens_without_replacement`: its worker
returned 101 instead of expected 97. Worker stderr had been discarded, so
the inner cause of that occurrence is unknown. Changed only this test helper's
stderr to inherit typed worker diagnostics; did not alter its expected exit,
product SecureStore code, retries or authorization behavior. Isolated rerun
and the final broad run passed. This does not establish a fix or common cause
with the previously recorded read-after-delete failure; both observations
remain open for diagnosis if reproduced.

## Scope limits / continuation

Controller is backend-only and explicitly invoked. Production startup,
authenticated status IPC, real crash/restart coverage and Mac adapter remain
open. Updated Windows batch runner references the new 31-file manifest; it was
not rerun as a complete batch invocation. Individual commands and the actual
PS5.1 helper were run as listed. No Mac/CI/installer/publisher-authentication,
peer-trust, incoming-admission or M8-complete claim. No Actions, commit, push
or merge.
