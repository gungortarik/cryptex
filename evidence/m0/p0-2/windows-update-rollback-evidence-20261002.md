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
