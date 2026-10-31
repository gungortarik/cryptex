# macOS synthetic update compatibility and rollback evidence

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Status:** PASS / VERIFIED for the test fixture

**Date:** 2026-10-03

**Source:** `46169eeab0c74817147c13eabd5eff510f378a69`

**Branch:** `m0/p0-2-cursor`

**Host:** macOS 27.0.1 / arm64

## Local validation

`cargo test -p cryptexd --locked` passed with 33 tests: 22 library tests,
7 lifecycle harness tests, 2 Mode B supervisor tests and 2 update/rollback
tests. Main and doc-test targets each contained 0 tests. No tests were
ignored or failed in the successful host run; command exit code was 0.
`cargo build -p cryptexd --locked` also exited 0.

The initial sandbox run had four daemon readiness/clean-shutdown failures in
the lifecycle harness. Repeating the identical test command with host socket
and process access passed all targets. No implementation or tests were edited.

## Physical-host fixture measurement

The repository-built `target/debug/cryptexd` ran `lifecycle update-proof`
with itself as `--base-bin`, experiment `macos-update-compat-20261003`, and
the fresh runtime root below. The command and measurement capture exited 0.
The retained KV was read back and matched all ten required acceptance fields.

```text
experiment_id=macos-update-compat-20261003
runtime_root=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/compare/macos-update-compat-20261003
supervisor_mode=desktop_supervisor
initial_identity=v1
initial_pid=26562
initial_marker_digest=d914619dfbb07c95
incompatible_pair_rejected=true
active_identity_after_incompatible_pair=v1
compatible_pair_accepted=true
compatible_identity=v2
compatible_pid=26563
compatible_restart_ms=118
compatible_marker_digest=d914619dfbb07c95
compatible_marker_unchanged=true
failed_identity=v_bad
failed_detected=true
failed_marker_digest=d914619dfbb07c95
failed_marker_unchanged=true
rollback_identity=v2
rollback_pid=26565
rollback_marker_digest=d914619dfbb07c95
rollback_marker_unchanged=true
final_instance_count=0
scope=isolated_p0_2_update_rollback_proof
```

Retained measurement:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/compare/macos-update-compat-20261003/measurements/macos-update-compat-20261003.kv`

## Scope

This verifies synthetic protocol-pair rejection before staging, preservation
of active `v1`, matching-pair acceptance, failed-candidate detection, rollback
to `v2`, marker preservation and final instance cleanup on the physical Mac.
The protocol pair is test metadata; this fixture does not replace or launch
an actual Desktop package or prove production canonical-data preservation.
Runtime data remained outside Git. ADR-0012 remains unresolved.
