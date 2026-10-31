# M5 Slice 3 -- review-fix Windows verification

**Date:** 2026-10-10
**Branch:** `codex/m5-capture-timeline`
**Baseline tip:** `23dcc0e4777e2ad927c07d2ad1de6a35aab0522e`
**Tested product SHA:** `ec0e064863adb087037c0472f2cc3d8cc299f229`
**CARGO_TARGET_DIR:** `C:\Users\Tarik\AppData\Local\Temp\cryptex-m5-s3-fix-target`
**Host:** Windows 10.0.26300. No macOS PASS is claimed from this host.

## Findings addressed

1. **Unresolved submission preservation** — Ordinary Save cannot overwrite an
   unresolved `pendingSnapshot` / `capture_id`. Deliberate new Save requires
   explicit duplication acknowledgment and archives the previous unresolved
   snapshot. ReceiptMissing never auto-resubmits. Policy covered by
   `capture-policy` state-transition tests (timeout→Save, receipt_missing→Save,
   edits while unresolved, late committed, repeated clicks, rejected retry).
2. **Native Quit draft protection** — User-initiated Quit (`p0_2_quit_clean`,
   tray Quit) consults `capture_session_dirty` and shows a native confirm.
   Cancel retains app/draft/pending. Close-to-tray still hides only (session
   preserved). Forced kill / OS shutdown are out of scope.
3. **IPC response write deadlines** — `handle_connection_for_peer` uses
   operation-appropriate write deadlines (`IPC_RESPONSE_DEADLINE` for
   health/search/enrollment; `IPC_CAPTURE_RESPONSE_DEADLINE` only for capture).
   Capture timeout remains Uncertain (not cancel).
4. **Redacted transport errors** — Tauri capture/resolve map `io::Error` to
   stable codes; renderer shows redacted copy only (no bodies/paths/OS strings).

## Commands / results

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `ipc::tests::` | **32/32 PASS** |
| `response_write_deadline_is_operation_appropriate` | PASS |
| `capture::` | **20/20 PASS** |
| `capture_` filter | **28/28 PASS** |
| `restart` filter | **18 lib + 1 lifecycle + 2 supervisor PASS** |
| Desktop `capture_quit_guard_tests` | **3/3 PASS** |
| Desktop `test:capture-policy` | **14/14 PASS** |
| Desktop `test:search-policy` | **2/2 PASS** |
| Desktop `npm run build` | PASS |
| `clippy -D warnings` cryptexd | PASS |
| `clippy -D warnings` desktop src-tauri | PASS |

## Remaining native interactive verification (Windows + macOS)

These are **not** claimed PASS from unit tests alone:

- Tray **Quit** with dirty/unresolved capture: Cancel keeps app + draft + pending;
  Confirm discards session-only input and exits.
- `p0_2_quit_clean` / any in-app Quit entry with dirty session (same Cancel/Confirm).
- Window **close-to-tray** with dirty/unresolved capture: window hides; reopening
  shows the same draft and pending capture id (no exit).
- Forced process kill / OS shutdown: explicitly unprotected.

## Mac verification checklist (exact commands)

```bash
git fetch origin
git checkout codex/m5-capture-timeline
git pull --ff-only origin codex/m5-capture-timeline
git rev-parse HEAD   # expect ec0e064863adb087037c0472f2cc3d8cc299f229 or later docs-only tip containing it
git merge-base --is-ancestor ec0e064863adb087037c0472f2cc3d8cc299f229 HEAD && echo PRODUCT_OK
git merge-base --is-ancestor a6c32bd30f883e546e2eb976787a59057fae211b HEAD && echo M4_OK

export CARGO_TARGET_DIR="${TMPDIR%/}/cryptex-m5-s3-fix-target"
mkdir -p "$CARGO_TARGET_DIR"

cargo fmt --all -- --check
cargo test --locked -p cryptexd ipc::tests::
cargo test --locked -p cryptexd capture::
cargo test --locked -p cryptexd capture_
cargo test --locked -p cryptexd restart
cargo clippy --locked -p cryptexd --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked capture_quit_guard

cd apps/desktop
npm run test:capture-policy
npm run test:search-policy
npm run build
```

Then perform the interactive Quit / close-to-tray checks above on macOS and record
results in `evidence/m5/m5-slice3-macos-verification-YYYYMMDD.md`.

Do not launch Desktop without coordination. Do not dispatch Actions, merge, start
Slice 4, enable production v2, or claim M5 complete.
