# M5 Slice 3 -- explicit app-menu Quit fix (Windows verification)

**Date:** 2026-10-11
**Branch:** `codex/m5-capture-timeline`
**Failed Mac recheck product:** `3b0c6d4d0410ed963498b906c5da3e3967a6f82c` (evidence: `m5-slice3-macos-app-menu-quit-recheck-fail-20261011.md`)
**Product baseline (archive/Quit):** `e874c066ba011cf3c46516f3f4c9125d252abf80`
**Tested product SHA:** `bc3463bf2829be42d7bda41fdc3898ed4c759f1f`
**CARGO_TARGET_DIR:** `C:\Users\Tarik\AppData\Local\Temp\cryptex-m5-s3-fix-target`
**Host:** Windows 10.0.26300. **No macOS native PASS is claimed from this host.**

## Cause

Repository-pinned muda macOS maps `PredefinedMenuItem::quit` to `terminate:`, bypassing `ExitRequested`. The prior ExitRequested-only fix at `3b0c6d4d0410ed963498b906c5da3e3967a6f82c` therefore could not guard app-menu / Cmd+Q.

## Fix

- Install a Cryptex app menu (replacing Tauri's default menu factory) that keeps standard macOS items (About, Services, Hide, Hide Others, Edit/View/Window/Help) but uses an explicit `MenuItem` id `cryptex-app-menu-quit` with accelerator `CmdOrCtrl+Q`.
- Route that id (and tray `quit`) through `run_guarded_user_quit` → shared confirm, re-entry gate, clean daemon stop, then intentional `app.exit(0)`.
- Do not include `PredefinedMenuItem::quit` (forbidden plan entry covered by unit tests).
- For `ExitRequested` that do occur: call `prevent_exit` **before** blocking confirm/shutdown; intentional `code: Some(_)` exits without another prompt.

## Commands / results (Windows)

| Check | Result |
|---|---|
| `exit_lifecycle_routing_tests` (menu plan + routing + Cancel/Confirm/repeat/failure/intentional) | **9/9 PASS** |
| `capture_quit_guard_tests` | **5/5 PASS** |
| Desktop `test:capture-policy` | **16/16 PASS** |
| Desktop `test:search-policy` | **2/2 PASS** |
| Desktop `npm run build` | PASS |
| `cargo fmt` desktop `--check` | PASS |
| `clippy -D warnings` desktop bin | PASS |

## Explicit non-claims

- No macOS native PASS from this Windows host.
- No forced-kill / OS-shutdown protection.
- No Actions, merge, Slice 4, production v2 activation, or M5-complete.

## Mac native recheck instructions

```bash
git fetch origin
git checkout codex/m5-capture-timeline
git pull --ff-only origin codex/m5-capture-timeline
git merge-base --is-ancestor bc3463bf2829be42d7bda41fdc3898ed4c759f1f HEAD && echo PRODUCT_OK
git merge-base --is-ancestor 3b0c6d4d0410ed963498b906c5da3e3967a6f82c HEAD && echo FAILED_ANCESTOR_OK

export CARGO_TARGET_DIR="${TMPDIR%/}/cryptex-m5-s3-app-menu-quit-target"
mkdir -p "$CARGO_TARGET_DIR"

cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --bin cryptex-desktop -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --bin cryptex-desktop exit_lifecycle
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --bin cryptex-desktop capture_quit_guard

cd apps/desktop
npm run test:capture-policy && npm run test:search-policy && npm run build
npm run tauri:package -- --bundles app
cd ../..

# Native interactive (required):
# 1) App menu Quit / Cmd+Q + draft: Cancel keeps Desktop+daemon+draft/archive.
# 2) Confirm: daemon stops cleanly; Desktop exits (note clean_quit_from_app_menu).
# 3) Rapid repeated Quit: single dialog; no double shutdown.
# 4) Tray Quit Cancel/Confirm still PASS; close-to-tray + Dock reopen still PASS.
# 5) Confirm menu shows Quit Cryptex with Cmd+Q and is not a silent terminate bypass.
```
