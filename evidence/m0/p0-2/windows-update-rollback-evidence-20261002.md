# P0-2 — Windows synthetic update/rollback evidence

**Date (UTC):** 2026-10-02  
**Host:** Windows physical machine / x86_64  
**Branch:** `m0/p0-2-cursor`  
**Commit under test:** `484181f`  
**Experiment ID:** `windows-update-rollback-20261002T`  
**Status:** VERIFIED for the isolated comparison harness behavior only

## Runtime isolation

All runtime files and synthetic fixtures were kept outside the repository:

```text
%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\windows-update-rollback-20261002\update-rollback
```

The retained KV measurement is at:

```text
%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\windows-update-rollback-20261002\update-rollback\measurements\windows-update-rollback-20261002T.kv
```

## Results

| Stage | Result |
|---|---|
| Initial candidate | `v1`, PID 3416 |
| Compatible candidate | `v2`, PID 30824; restart 80 ms |
| Incompatible candidate | `v_bad` detected/rejected |
| Rollback | returned to `v2`, PID 31516 |
| Synthetic canonical marker digest | `d914619dfbb07c95` at every stage; unchanged flags true |
| Final instance count | 0 |

The fixture demonstrates that the harness preserved the synthetic canonical
marker across compatible activation, rejected activation and rollback. It does
not constitute a signed production package/update test, prove power-loss
atomicity, or authorize a production update architecture. ADR-0012 remains
unresolved.

## Windows Desktop/daemon protocol-pair rejection follow-up — 2026-10-03

The `cryptexd lifecycle update-proof` fixture was extended to check a synthetic
Desktop/daemon protocol pair before staging the daemon candidate. On the
Windows development host, the updated proof completed with:

```text
experiment_id=windows-update-compat-20261003
incompatible_pair_rejected=true
active_identity_after_incompatible_pair=v1
compatible_pair_accepted=true
compatible_identity=v2
compatible_restart_ms=157
compatible_marker_unchanged=true
failed_detected=true
failed_marker_unchanged=true
rollback_identity=v2
rollback_marker_unchanged=true
final_instance_count=0
```

The full measurement was retained outside the repository under
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\windows-update-compat-20261003`.
`cargo test -p cryptexd --locked` also passed on the Windows host (44 tests).

This is still synthetic protocol metadata and daemon-fixture evidence. It does
not replace or execute a Desktop package and does not prove production package
atomicity. A matching macOS run is requested in
`macos-update-compat-test-plan-20261003.md`.
