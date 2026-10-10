# M5 Slice 3 -- Windows verification (capture IPC + Quick Capture Inbox)

**Date:** 2026-10-10
**Branch:** `codex/m5-capture-timeline`
**Tested product SHA:** `c5c75afcca978e4937c1063aecc76c85a10bf4f5`
**IPC commit:** `a31e052834cbc669f30ca950bcada32c318f5301`
**Base tip before Slice 3:** `3e98871f9296d38653b765fbce5ef140a284913f` (ancestor)
**M4 merge ancestor:** `a6c32bd30f883e546e2eb976787a59057fae211b` (ancestor)
**CARGO_TARGET_DIR:** `C:\Users\Tarik\AppData\Local\Temp\cryptex-m5-s3-target`
**Host:** Windows 10.0.26300 (x86_64). No macOS PASS is claimed from this host.

## Scope delivered

- Authenticated bounded Desktop IPC ops `memory.capture` / `memory.capture_resolve`
  with `content.capture`, OS peer authentication before large frames, capture-specific
  frame bound (raw 512 KiB text kept independent of serialized size), single capture
  worker slot, 60s capture response deadline returning **Uncertain** (not cancel).
- Redacted typed outcomes (`committed` / `rejected`+`rejectCode` / `uncertain`+`uncertainReason`).
- Minimal Quick Capture Inbox in the existing Desktop shell: explicit classification
  (`public|internal|private|restricted`), UTF-8 byte limit without truncation,
  capture_id + immutable request snapshot, no double-submit/stale-response draft loss,
  Resolve for uncertain outcomes, no auto-resubmit on `receipt_missing`, no
  localStorage/disk drafts, "Saved" means canonical durability only.
- No v1 schema/bytes/signature/recovery changes; production v2 writing remains OFF;
  no timeline/projection catch-up/AI/Obsidian/attachments/pairing/purge/backup/updater.

## Commands executed (Windows)

```text
cargo fmt --all -- --check
cargo test --locked -p cryptexd ipc::tests::
cargo test --locked -p cryptexd capture::
cargo test --locked -p cryptexd capture_
cargo test --locked -p cryptexd restart
cargo test --locked -p cryptexd --lib canonical_writer::
cargo test --locked -p cryptexd --lib secure_store:: -- --test-threads=1
cargo test --locked -p cryptex-projection
cargo clippy --locked -p cryptexd --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --all-targets -- -D warnings
npm run test:capture-policy   # apps/desktop
npm run test:search-policy    # apps/desktop
npm run build                 # apps/desktop (tsc --noEmit && vite build)
```

## Results

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `ipc::tests::` | **29/29 PASS** (includes capture auth/bounds/commit/resolve/conflict + capacity/deadline) |
| added `desktop_capture_capacity_is_bounded_and_keeps_health_slots` | PASS |
| added `bounded_capture_deadline_returns_uncertain_not_cancel` | PASS |
| `capture::` | **20/20 PASS** |
| `capture_` filter (capture + capture_restart + related) | **26/26 PASS** |
| `restart` filter | **18 lib + 1 lifecycle + 2 supervisor PASS** |
| `canonical_writer::` | **22/22 PASS** |
| `secure_store::` | **17/17 PASS** |
| `cryptex-projection` | **48/48 PASS** |
| `clippy -D warnings` cryptexd | PASS |
| `clippy -D warnings` desktop src-tauri | PASS |
| Desktop `test:capture-policy` | **7/7 PASS** |
| Desktop `test:search-policy` | **2/2 PASS** |
| Desktop `npm run build` | PASS |

## Security findings (Slice 3)

- Caller-supplied role/Brain/device/path never establishes capture authority; Desktop
  peer process authentication is required before capture-sized frames.
- Unauthorized / wrong-capability capture and resolve cannot write and do not return
  memory bodies or internal paths.
- Transport timeout/disconnect surfaces as Uncertain; worker slot remains held until
  work exits — timeout is **not** a cancel/commit-absent proof.
- Test-only SecureStore (`FileBackedSecureStore`) remains `#[cfg(test)]`.
- Inbox renders capture status as escaped React text; no renderer FS/shell/SQLite access.

## Remaining risks / blockers

- **macOS not verified on this host.** Owner must run the Mac checklist below on the
  pushed candidate before any dual-OS PASS claim.
- No interactive Desktop launch/manual Inbox UX pass was performed (per coordination rule).
- Projection catch-up after capture is intentionally out of Slice 3; search may lag.
- Abrupt power-loss durability remains out of scope (Slice 2 note still applies).

## Mac verification checklist (exact commands)

Run on macOS at the **final pushed product SHA** (do not claim PASS from Windows):

```bash
git fetch origin
git checkout codex/m5-capture-timeline
git pull --ff-only origin codex/m5-capture-timeline
git rev-parse HEAD   # expect c5c75afcca978e4937c1063aecc76c85a10bf4f5 or later docs-only tip that contains it
git merge-base --is-ancestor c5c75afcca978e4937c1063aecc76c85a10bf4f5 HEAD && echo PRODUCT_ANCESTOR_OK
git merge-base --is-ancestor a6c32bd30f883e546e2eb976787a59057fae211b HEAD && echo M4_OK

export CARGO_TARGET_DIR="${TMPDIR%/}/cryptex-m5-s3-target"
mkdir -p "$CARGO_TARGET_DIR"

cargo fmt --all -- --check
cargo test --locked -p cryptexd ipc::tests::
cargo test --locked -p cryptexd capture::
cargo test --locked -p cryptexd capture_
cargo test --locked -p cryptexd restart
cargo test --locked -p cryptexd --lib canonical_writer::
cargo test --locked -p cryptexd --lib secure_store:: -- --test-threads=1
cargo test --locked -p cryptex-projection
cargo clippy --locked -p cryptexd --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --all-targets -- -D warnings

cd apps/desktop
npm run test:capture-policy
npm run test:search-policy
npm run build
```

Record OS version / arch, exact HEAD, and pass/fail counts in a new
`evidence/m5/m5-slice3-macos-verification-YYYYMMDD.md`. Do not launch the Desktop app
without coordination. Do not dispatch Actions, merge, mark M5 complete, or start Slice 4.
