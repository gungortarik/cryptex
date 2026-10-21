# M5 Slice 3 macOS automated verification

**Date:** 2026-10-10
**Provenance:** Owner-supplied Mac report, `## M5 Slice 3 macOS — automated verification PASS.txt`. Results were not rerun on this Windows host. Raw command logs were not attached.
**Branch:** `codex/m5-capture-timeline`
**Tested HEAD:** `5a21c1ae8079a4d057f6facba85bc00421f128fa`
**Product ancestor:** `e874c066ba011cf3c46516f3f4c9125d252abf80`, reported confirmed.
**Host:** macOS 27.0.1 (26A434), arm64.
**Target:** `$HOME/Library/Caches/cryptex-cargo-target-m5`
**Working tree:** Reported clean before and after.

## Reported results

Every command below exited 0. Cargo test/checklist commands used locked dependencies; frontend installation used the frozen lockfile. Command names below follow the supplied abbreviated report.

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| cryptexd library IPC | 30/30 |
| capture | 20/20 |
| capture_restart | 15/15 |
| canonical_writer | 22/22 |
| secure_store, single test thread | 14/14 |
| projection | 17/17 |
| cryptex-projection | 48/48; 0 doctests |
| Strict all-target Clippy: cryptexd + cryptex-desktop | PASS |
| Desktop capture_quit_guard | 5/5 |
| pnpm frozen installation | PASS |
| Desktop capture-policy | 16/16 |
| Desktop search-policy | 2/2 |
| Desktop frontend build | PASS |
| git diff --check | PASS |

Platform-specific counts remain distinct: earlier Windows IPC 32 and SecureStore 17 are not substituted for Mac IPC 30 and SecureStore 14.

## Remaining gate

Automated macOS verification passes at the recorded HEAD. Native interactive verification is prepared but was not launched: tray Quit Cancel/Confirm, dirty draft close-to-tray/reopen, and unresolved/archive protection and resolution remain unverified on both platforms in this evidence.

Use the existing isolated synthetic pilot procedure only, coordinate with the owner before app launch or desktop control, and record any scenario that cannot be safely reproduced as unverified. Unit tests do not prove native interaction. Forced kill/OS shutdown protection is not claimed.

No production code/dependency changes, Actions, merge or M5 completion are implied. Slice 4 remains closed pending Slice 3 closeout and separate authorization.
