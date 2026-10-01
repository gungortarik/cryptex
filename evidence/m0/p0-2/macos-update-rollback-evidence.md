# macOS Update / Rollback Evidence (P0-2)

**Classification:** VERIFIED (synthetic/test-only macOS proof)  
**Branch:** `m0/p0-2-cursor`  
**Harness commits:** `fd19f6f` (implementation), evidence under this file  
**Latest physical experiment ID:** `macos-update-rollback-20261001T003240Z`  
**ADR-0012:** UNRESOLVED (not accepted by this evidence)  
**Supervisor mode used for restart ownership:** `desktop_supervisor` (proof-only)  
**Runtime root:** `~/Library/Application Support/Cryptex-Dev/p0-2/compare/update-rollback`

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

## Claim classification (post-proof)

| Claim | Class |
|---|---|
| Compatible staged binary replacement under supervised restart | **VERIFIED** |
| Supervisor detects/restarts into compatible `v2` | **VERIFIED** |
| Single-instance protection remains intact during active holds | **VERIFIED** |
| Synthetic marker unchanged across compatible update | **VERIFIED** |
| Incompatible/failing replacement detected | **VERIFIED** |
| Failed update does not damage synthetic marker | **VERIFIED** |
| Rollback to last-known-good (`v2`) succeeds | **VERIFIED** |
| Final runtime state truthful | **VERIFIED** |
| No runtime files stored in repository | **VERIFIED** |
| Production installer / updater UX | **NOT YET VERIFIED** |
| Windows update/rollback | **NOT YET VERIFIED** |
| Login / reboot persistence of update path | **NOT YET VERIFIED** |
| ADR-0012 supervision selection | **UNRESOLVED** |

## Latest physical measurements

```
experiment_id=macos-update-rollback-20261001T003240Z
runtime_root=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/compare/update-rollback
supervisor_mode=desktop_supervisor
initial_identity=v1
initial_pid=26444
initial_marker_digest=d914619dfbb07c95
compatible_identity=v2
compatible_pid=26445
compatible_restart_ms=112
compatible_marker_digest=d914619dfbb07c95
compatible_marker_unchanged=true
failed_identity=v_bad
failed_detected=true
failed_marker_digest=d914619dfbb07c95
failed_marker_unchanged=true
rollback_identity=v2
rollback_pid=26447
rollback_marker_digest=d914619dfbb07c95
rollback_marker_unchanged=true
final_instance_count=0
scope=isolated_p0_2_update_rollback_proof
```

| Stage | Identity | PID | Marker digest | Instance |
|---|---|---|---|---|
| INITIAL | v1 | 26444 | d914619dfbb07c95 | 1 (held) |
| COMPATIBLE UPDATE | v2 | 26445 | d914619dfbb07c95 (unchanged) | 1 (held) |
| FAILED UPDATE | v_bad | n/a (start failed) | d914619dfbb07c95 (unchanged) | 0 |
| ROLLBACK | v2 | 26447 | d914619dfbb07c95 (unchanged) | 1 then clean stop → 0 |

Compatible restart latency: **112 ms**.  
Active identity after proof: `v2`.  
Marker contents after proof: `canonical-v1`.

## Limitations (explicit)

- Synthetic / test-only staged binaries and a failing shell fixture — **not** a production installer.
- Uses Mode B process supervisor only as restart owner for the proof — **does not** select ADR-0012.
- macOS only in this evidence file.
- No login / reboot.
- Synthetic marker proves fixture non-corruption only; not production canonical-data safety beyond M0 fixture semantics.

## Decision readiness

This evidence supports P0-2 acceptance of the **compatible update + rollback/error path** using synthetic fixtures.

It does **not**:

- select Mode A / Mode B / hybrid
- accept ADR-0012
- claim Windows or reboot coverage
