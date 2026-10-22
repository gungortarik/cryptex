# M8 Windows package identity and owned generation verification

Date: 2026-10-21. Isolated `codex/m8-device-trust` worktree, base HEAD
`74bc3b14127962579ae03838bafea9336612a033`; changes remain uncommitted.
Original dirty Windows/Mac checkouts were not modified.

Proof root (outside Git):
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/package-boundary-20261021-5f8f5ab5f6aa4efe8f510bacf58eecf5`.
Current behavior/boundaries: `m8-pinned-package-boundary-contract-20261021.md`.

## Final-source observed results

| Command / target | Result |
| --- | --- |
| `cargo test --locked -p cryptexd -p cryptex-events` | PASS, exit 0 |
| events library | 61 PASS |
| daemon library | 356 PASS, 3 explicit native fixtures ignored |
| daemon CLI | 8 PASS |
| lifecycle integration | 9 PASS |
| Mode B integration | 3 PASS |
| update/rollback integration | 2 PASS |
| schema-generator / doc targets | Executed, no tests |
| Explicit pinned package / native generation fixture | 1 PASS |
| Explicit private profile / Windows Credential Manager / owned endpoint fixture | 1 PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` | PASS |
| `cargo build --locked -p cryptexd` | PASS |
| Actual Windows PowerShell 5.1 runner-helper regression | 3/3 PASS |

The broad default invocation completed this time without Application Control
4551. This does not prove a global policy change or permanent resolution; no
security settings, binary renames or alternate loaders were used.

The earlier isolated package run was 4 PASS / 1 native ignored. The package
native fixture was then extended with abandonment and same-size tampering cases
and rerun on final source. Ordinary notice/executable corruption is refused
before launch. A compile typo (`Development` instead of `Dev`) was corrected
before successful compilation; no test assertion was relaxed.

## Native proof

The retained pinned binary was copied into a fresh synthetic package directory.
Both repository LF notices were copied exactly and verified by product code.
Fresh native generation PID **34436** exited 0, forced false. No server was
started with the generated defaults. Package mutation/deletion/rename attempts
failed while pinned; dropping the factory did not release the process pins.
A second abandoned generation was cleaned up and its retained OS observer was
signaled before the executable became writable. Same-size binary/notice changes
and a missing notice were rejected, then original public bytes were restored.

Final pre-existing private-profile native fixture was also rerun: PIDs
**2660 / 31840**, each exit 0 and forced false, actual invalid-key status **403**.
The established endpoint was attested; only the synthetic DEV credential was
deleted and its absence checked. Public result JSONs are copied into proof.
Final process snapshot: **0 Syncthing processes**.

Built debug `cryptexd.exe` SHA-256:
`ed08a16b3c374b09649b1686ac06885d93d8640900456b3955e636a6a1624015`.

## Credential-deletion investigation remains open

The prior abrupt-ready test's read-after-delete failure was NOT suppressed,
retried inside the assertion or fixed by changing SecureStore behavior. On this
candidate it passed in both broad library runs and **12/12** separate unchanged
exact-test invocations. This does not establish its earlier root cause or an OS
eventual-consistency guarantee. Keep the earlier failure in the audit record;
production credential replacement/recovery still fails closed.

## Source/runner continuity

The adjacent `m8-pinned-package-source-sha256-20261021.txt` covers the prior
22-file endpoint set plus three package files and both exact LF notices (27).
The batch runner now uses it and includes an explicit pinned-generation native
step/result. The sealed historical ZIP, prior manifests and guest receipts were
not rewritten; no new guest package or full runner invocation is claimed here.
Individual commands above were run locally, with complete logs retained.

Native config provisioning/controller integration and ADR-0019 activation gates
remain open. No production transport startup, trust/pairing mutation, Actions,
commit, push, merge, Mac PASS or M8 completion is claimed.
