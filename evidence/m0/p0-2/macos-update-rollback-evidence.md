# macOS Update / Rollback Evidence (P0-2 test-only)

**Classification:** VERIFIED (synthetic/test-only macOS proof)  
**Date:** 2026-09-30  
**Branch:** `m0/p0-2-cursor`  
**Experiment ID:** `macos-update-rollback-1`  
**ADR-0012:** UNRESOLVED (not accepted by this evidence)

## Audit classification (pre-implementation)

| Area | Status |
|---|---|
| version/compat fixtures (`compat.rs`) | PARTIAL — protocol + marker only |
| lifecycle supervisor (Mode B) | VERIFIED earlier — reused as restart owner |
| synthetic marker fixture | PARTIAL → extended by update proof |
| binary stage / activate helpers | NOT STARTED → implemented test-only |
| rollback helpers | NOT STARTED → implemented test-only |
| docs/26 production updater | UNRESOLVED — out of scope |
| ADR-0012 | UNRESOLVED |

## What was proven

Physical run of `m0-lifecycle-compare run-update-rollback` under Cryptex-Dev:

`~/Library/Application Support/Cryptex-Dev/p0-2/compare/update-rollback`

| Stage | Result |
|---|---|
| INITIAL `v1` under DesktopSupervisor | VERIFIED — PID `26561`, identity `v1` |
| COMPATIBLE stage+activate `v2` + restart | VERIFIED — PID `26562`, restart `111ms`, identity `v2`, instance=1 |
| FAILED stage+activate `v_bad` | VERIFIED — start failure detected, no duplicate daemon |
| ROLLBACK to last-known-good `v2` | VERIFIED — PID `26564`, identity `v2`, instance=1 |
| Synthetic marker digest | VERIFIED — `d914619dfbb07c95` unchanged across all stages |
| Runtime outside repository | VERIFIED |
| Final clean stop / lock not held | VERIFIED |

## Measurement KV (verbatim)

```
experiment_id=macos-update-rollback-1
runtime_root=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/compare/update-rollback
supervisor_mode=desktop_supervisor
initial_identity=v1
initial_pid=26561
initial_marker_digest=d914619dfbb07c95
compatible_identity=v2
compatible_pid=26562
compatible_restart_ms=111
compatible_marker_digest=d914619dfbb07c95
compatible_marker_unchanged=true
failed_identity=v_bad
failed_detected=true
failed_marker_digest=d914619dfbb07c95
failed_marker_unchanged=true
rollback_identity=v2
rollback_pid=26564
rollback_marker_digest=d914619dfbb07c95
rollback_marker_unchanged=true
final_instance_count=0
scope=isolated_p0_2_update_rollback_proof
```

Runtime copy: `.../update-rollback/measurements/macos-update-rollback-1.kv`

## Post-run layout identities

| Slot | Identity |
|---|---|
| `update-bin/active` | `v2` (last-known-good restored) |
| `update-bin/previous` | `v2` |
| `update-bin/failed` | `v_bad` (retained bad fixture) |
| `update-bin/staging` | empty |
| `synthetic-canonical.marker` | `canonical-v1` |

## Automated gates

| Command | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS (incl. `update_rollback` integration tests) |
| `pnpm --filter @cryptex/desktop build` | PASS |

## Explicit limitations

- VERIFIED: synthetic/test-only staged-binary update/rollback on macOS
- NOT YET VERIFIED: production installer / packaging / updater UX
- NOT YET VERIFIED: Windows update/rollback proof
- NOT YET VERIFIED: login/reboot persistence of update state
- UNRESOLVED: ADR-0012 final supervision / updater orchestration decision
- Mode B DesktopSupervisor used only as the restart owner for this proof; does not select production supervision
