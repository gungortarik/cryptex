# M2 Quarantine Staging Ownership / Cleanup Fix (2026-10-07)

**Branch:** `m2/immutable-event-store`  
**Follows:** `evidence/m2/m2-closeout-l1-l2-f4-f6-20261007.md` (F6 staging Drop-guard)

## Bug

`StagingCleanup` was armed with the intended staging path **before** `q_open_create_new_write` (`create_new`) succeeded. On `AlreadyExists`, `Drop` could delete a staging file this call did not own.

Cleanup failures after an owned staging create were also discarded (`let _ = q_remove_file` / best-effort Drop only), losing the primary failure context when remove failed.

## Fix

1. Keep `StagingCleanup` **unarmed** until exclusive create returns `Ok`.
2. On owned-path failure paths: `cleanup_now()` then `with_staging_cleanup(primary, cleanup)` → `StorageError::QuarantineCleanupFailed { primary, cleanup }`.
3. `Drop` remains best-effort only for unwind / forgotten paths.
4. `place_staging_as_object` no longer silently removes staging on rename failure; the caller owns cleanup reporting.
5. Exclusive `create_new` behavior unchanged.

## Test hooks (cfg(test))

- `force_next_staging_leaf` — deterministic collision path
- `fail_next_staging_removes` — injectable remove failure

## Regressions

- `staging_create_collision_preserves_foreign_bytes` — foreign staging preserved byte-for-byte; `AlreadyExists`; no cleanup-combined error
- `staging_cleanup_failure_preserves_primary_error` — `QuarantineCleanupFailed` retains short-read primary + injected remove cleanup in typed error and Display

## Windows verification

```
cargo test --locked -p cryptex-storage quarantine
→ ok. 24 passed

cargo test --locked -p cryptexd --lib
→ ok. 126 passed

cargo fmt --all -- --check
→ PASS

cargo clippy --locked -p cryptex-storage -p cryptexd --all-targets -- -D warnings
→ PASS
```

## macOS / final CI

Shared-code tip requires the same four checks on macOS, then **one** dual-OS Actions gate. M2 remains IN PROGRESS; do not merge.

### macOS four-check status (2026-10-07)

- Local agent host: Windows_NT — cannot run Darwin checks here.
- Cloud verify attempt at tip `1a26493`: **Linux** (`MAC_CHECKS: SKIPPED_NOT_MACOS`).
- Dual-OS GitHub Actions gate: **not started** (blocked until real macOS four-check PASS).

Please run on a Mac at `1a26493` (or later tip containing `b5c9d69`):

```
cargo test --locked -p cryptex-storage quarantine
cargo test --locked -p cryptexd --lib
cargo fmt --all -- --check
cargo clippy --locked -p cryptex-storage -p cryptexd --all-targets -- -D warnings
```

After Mac PASS, run the single dual-OS Actions workflow on this tip.
