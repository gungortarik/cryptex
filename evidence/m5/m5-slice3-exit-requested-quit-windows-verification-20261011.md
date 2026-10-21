# M5 Slice 3 -- app-menu ExitRequested Quit fix (Windows verification)

**Date:** 2026-10-11
**Branch:** `codex/m5-capture-timeline`
**Product baseline:** `e874c066ba011cf3c46516f3f4c9125d252abf80`
**Finding evidence commit:** `0b4c3de3261b76e74b6fc71b1063422471762795` (`evidence/m5/m5-slice3-macos-native-interactive-20261011.md`)
**Tested product SHA:** `3b0c6d4d0410ed963498b906c5da3e3967a6f82c`
**CARGO_TARGET_DIR:** `C:\Users\Tarik\AppData\Local\Temp\cryptex-m5-s3-fix-target`
**Host:** Windows 10.0.26300. **No macOS native PASS is claimed from this host.**

## Finding addressed

macOS application-menu Quit / Cmd+Q issued `RunEvent::ExitRequested` with
`code: None`. The Desktop run loop handled only Dock `Reopen`, so the process
exited without confirmation and left the supervised daemon running. Tray Quit
already used the shared confirm path and was not the failure.

## Fix

- Classify `ExitRequested`: `code: None` = user-initiated; `code: Some(_)` =
  intentional programmatic exit after guarded shutdown.
- User-initiated requests call `api.prevent_exit()`, then the shared
  `perform_user_initiated_quit` confirm + clean daemon stop, then `app.exit(0)`.
- Cancel keeps Desktop, daemon, draft and unresolved/archive session state.
- Shutdown failure keeps Desktop and writes a redacted `quit.error` diagnostic.
- `user_quit_in_progress` blocks re-entrant dialogs / duplicate shutdown.
- Intentional `app.exit` is allowed without a second prompt (no blanket veto).
- Close-to-tray, Dock reopen, and tray Quit paths remain unchanged in role.

## Commands / results (Windows)

| Check | Result |
|---|---|
| `exit_lifecycle_routing_tests` | **6/6 PASS** |
| `capture_quit_guard_tests` | **5/5 PASS** |
| Desktop `test:capture-policy` | **16/16 PASS** |
| Desktop `test:search-policy` | **2/2 PASS** |
| Desktop `npm run build` | PASS |
| `cargo fmt` desktop `--check` | PASS |
| `clippy -D warnings` desktop bin | PASS |

## Explicit non-claims

- No macOS native app launch or interactive recheck from this Windows host.
- No forced-kill / OS-shutdown protection.
- No GitHub Actions, merge, Slice 4, production v2 activation, or M5-complete.

## Mac build / native recheck instructions

```bash
git fetch origin
git checkout codex/m5-capture-timeline
git pull --ff-only origin codex/m5-capture-timeline
git rev-parse HEAD
git merge-base --is-ancestor 3b0c6d4d0410ed963498b906c5da3e3967a6f82c HEAD && echo PRODUCT_OK
git merge-base --is-ancestor e874c066ba011cf3c46516f3f4c9125d252abf80 HEAD && echo BASELINE_OK

export CARGO_TARGET_DIR="${TMPDIR%/}/cryptex-m5-s3-exit-requested-target"
mkdir -p "$CARGO_TARGET_DIR"

cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --bin cryptex-desktop -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --bin cryptex-desktop exit_lifecycle
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --bin cryptex-desktop capture_quit_guard

cd apps/desktop
npm run test:capture-policy
npm run test:search-policy
npm run build
npm run tauri:package -- --bundles app
cd ../..

# Native interactive recheck (required; not claimed from Windows):
# 1) App menu Quit Cryptex / Cmd+Q with draft: Cancel keeps Desktop+daemon+draft.
# 2) App menu Quit Confirm: daemon stops cleanly, Desktop exits (note clean_quit_from_app_menu).
# 3) Shutdown-failure simulation if available: Desktop retained; quit.error written.
# 4) Tray Quit Cancel/Confirm still PASS; close-to-tray + Dock reopen still PASS.
# 5) Confirm no double dialog on rapid Quit / after Confirm.
```
