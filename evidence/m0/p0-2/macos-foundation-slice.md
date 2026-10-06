# P0-2 macOS foundation slice

**Date (UTC):** 2026-09-30T23:32:09Z  
**Host:** macOS 27.0.1 (Build 26A434)  
**Branch:** `m0/p0-2-cursor`  
**Commit:** `01bbda6b8f0c8e7880b1a088eee8cdc8f4d1531d`  
**Toolchain:** rustc 1.98.1 (48a229cea 2026-09-01)  
**Runtime root used:** `~/Library/Application Support/Cryptex-Dev/p0-2/harness-20260930T233209Z`

P0-1 isolation check: `syncthing-p0-1` and `syncthing-p0-1-runs` remained present and were not modified by this probe. No runtime files were created inside the repository checkout.

## Automated checks (VERIFIED)

Commands:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked -p cryptexd
cargo test --locked -p m0-storage-bench
cargo run --locked -p cryptexd --quiet
```

Results:

| Check | Result |
|---|---|
| `cargo fmt --check` | VERIFIED pass |
| `cargo clippy -D warnings` | VERIFIED pass |
| `cargo test -p cryptexd` | VERIFIED 12 passed (6 unit + 6 harness) |
| `cargo test -p m0-storage-bench` | VERIFIED 3 passed (unchanged neighbor) |
| `cargo run -p cryptexd` | VERIFIED prints `cryptexd M0 foundation` |

Harness coverage exercised on macOS:

| Behavior | Status |
|---|---|
| second daemon instance rejected | VERIFIED |
| first daemon remains healthy while second is rejected | VERIFIED |
| clean shutdown releases instance lock | VERIFIED |
| forced crash detectable; restart path acquires lock again | VERIFIED |
| lifecycle state reflects runtime state | VERIFIED |
| no runtime files created inside repository | VERIFIED |
| incompatible Desktop/daemon protocol rejected | VERIFIED |
| failed-update/rollback fixture preserves synthetic marker | VERIFIED |

## Physical macOS process probe (VERIFIED for foundation slice only)

```bash
cargo run --locked -p cryptexd -- lifecycle probe --runtime-root "$RUNTIME"
cargo run --locked -p cryptexd -- lifecycle hold ... --stop-file ...
# concurrent probe -> exit 1 "already running"
cargo run --locked -p cryptexd -- lifecycle compat --desktop-protocol 1 --daemon-protocol 1 --runtime-root "$RUNTIME"
```

Observed:

- probe acquired lock (`instance=held`, `health=active`, `supervision=unresolved`)
- concurrent second instance exited `1` with `daemon instance already running`
- holder remained `health=active` during rejection
- clean stop released lock (`instance=stale`, `health=attention`)
- compat fixture reported `canonical_marker_safe=true`
- registration remained `not_configured` (no LaunchAgent installed)

## Explicitly NOT YET VERIFIED

| Requirement | Status |
|---|---|
| main-window close behavior (Desktop + tray) | NOT YET VERIFIED |
| reboot/login startup (LaunchAgent or other) | NOT YET VERIFIED |
| production crash supervisor restart policy | NOT YET VERIFIED |
| Desktop reconnect after daemon restart | NOT YET VERIFIED |
| real Desktop + daemon package update orchestration | NOT YET VERIFIED |
| real updater rollback path | PARTIAL (synthetic marker fixture only) |
| Windows Mode B AUTOMATED process-supervisor foundation | VERIFIED — see `windows-desktop-supervision-evidence.md` (GUI close / login-reboot / full Desktop IPC reconnect still NOT YET VERIFIED) |
| ADR-0012 selection among A/B/hybrid | OPEN / NOT ACCEPTED |

## Notes

- Supervision strategy is intentionally `unresolved`.
- This evidence does **not** mark P0-2 complete.
- Windows behavior must not be inferred from macOS results.
