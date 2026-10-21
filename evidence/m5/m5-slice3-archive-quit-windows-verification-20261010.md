# M5 Slice 3 -- archive resolve + fail-safe Quit Windows verification

**Date:** 2026-10-10
**Branch:** `codex/m5-capture-timeline`
**Baseline tip:** `ec0e064863adb087037c0472f2cc3d8cc299f229`
**Tested product SHA:** `e874c066ba011cf3c46516f3f4c9125d252abf80`
**CARGO_TARGET_DIR:** `C:\Users\Tarik\AppData\Local\Temp\cryptex-m5-s3-fix-target`
**Host:** Windows 10.0.26300. No macOS PASS is claimed from this host.
**Scope:** Remaining Slice 3 review fixes only. No Actions, app launch, merge, Slice 4, or M5-complete claim.

## Findings addressed

1. **Archived unresolved submissions** -- Deliberate-new Save archives the previous
   pending snapshot for resolve-only tracking (never silently abandoned). Inbox
   lists each archived `capture_id` with Resolve. Archives participate in draft/Quit
   protection via `sessionNeedsProtection` / `shouldPromptBeforeNativeQuit`.
   Late archive responses (including late committed for A while B is pending) call
   `syncUiFromPolicy` so React state stores the updated archive without clearing B's draft.
2. **Native Quit fail-safe** -- Guard starts unsynced (`capture_guard_sync_ok=false` =>
   Unknown). Renderer sync failures mark syncFailed and fail closed. User-initiated
   Quit (`p0_2_quit_clean`, tray Quit) always confirms via shared
   `decide_user_initiated_quit` / `perform_user_initiated_quit` with injectable confirm.
   Cancel never stops the daemon or exits. Confirm allows clean shutdown.
   Close-to-tray remains hide-only (session preserved). Forced kill / OS shutdown out of scope.

## Commands / results

| Check | Result |
|---|---|
| Desktop `test:capture-policy` | **16/16 PASS** (incl. A->B->B-commit A tracked; late A updates archive) |
| Desktop `capture_quit_guard_tests` | **5/5 PASS** (cancel/confirm/unknown/unresolved-after-newer-commit + redact) |
| Desktop `test:search-policy` | **2/2 PASS** |
| Desktop `npm run build` | PASS |
| `cargo fmt` desktop src-tauri `--check` | PASS |
| `clippy -D warnings` desktop bin | PASS |

## Remaining native interactive verification (Windows + macOS)

These are **not** claimed PASS from unit tests alone:

- Tray **Quit** with dirty/unresolved/archived capture: Cancel keeps app + draft + pending/archive; Confirm discards session-only input and exits.
- `p0_2_quit_clean` / any in-app Quit entry (same Cancel/Confirm).
- Window **close-to-tray** with dirty/unresolved/archived capture: window hides; reopening shows the same draft and unresolved ids (no exit).
- Forced process kill / OS shutdown: explicitly unprotected.

## Mac verification checklist (exact commands)

```bash
git fetch origin
git checkout codex/m5-capture-timeline
git pull --ff-only origin codex/m5-capture-timeline
git rev-parse HEAD   # tip may be docs-only after product SHA
git merge-base --is-ancestor e874c066ba011cf3c46516f3f4c9125d252abf80 HEAD && echo PRODUCT_OK
git merge-base --is-ancestor ec0e064863adb087037c0472f2cc3d8cc299f229 HEAD && echo BASELINE_OK
git merge-base --is-ancestor a6c32bd30f883e546e2eb976787a59057fae211b HEAD && echo M4_OK

export CARGO_TARGET_DIR="${TMPDIR%/}/cryptex-m5-s3-archive-quit-target"
mkdir -p "$CARGO_TARGET_DIR"

cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --bin cryptex-desktop -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --bin cryptex-desktop capture_quit_guard_tests

cd apps/desktop
npm run test:capture-policy
npm run test:search-policy
npm run build
cd ../..

# Interactive (not claimed from Windows):
# 1) Uncertain A -> deliberate B -> resolve B / commit B; confirm A remains listed and Quit still confirms.
# 2) Tray Quit Cancel: app stays; draft + archive preserved. Confirm: clean exit.
# 3) Close-to-tray with archive: hide only; reopen shows same session.
```

## Explicit non-claims

- No GitHub Actions run.
- No app launch / packaged visual PASS on this host.
- No Slice 4 work.
- No M5-complete claim.
