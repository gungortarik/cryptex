# M2 Slice 8c — macOS Clippy unused `std::io` fix

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `PENDING`

## Prior CI (partial green)

- GitHub Actions run: [37650566598](https://github.com/gungortarik/cryptex/actions/runs/37650566598)
- **PASS:** Windows readiness; Windows IPC ACL
- **FAIL:** macOS readiness at strict workspace Clippy — unused import `std::io::self` in `services/cryptexd/src/event_signing_key.rs` (only referenced inside `#[cfg(windows)]` placement)

## Fix (no lint allows)

- Drop module-level `use std::io::{self, ...}`; keep `ErrorKind` and `Write`.
- Use `std::io::Error::last_os_error()` inside the Windows `place_new_file_no_replace` path only.
- Unix hard-link path unchanged.

## Local verification (Windows host)

- `cargo fmt -p cryptexd`
- `cargo clippy --locked --workspace --all-targets -- -D warnings` — PASS
- `cargo test --locked -p cryptex-events -p cryptex-storage` — PASS
- `cargo test -p cryptexd --lib canonical_writer --locked` — PASS (10)
- `pnpm --filter @cryptex/schemas check:drift` — PASS
- Unix-only seal/sync tests not executable on this Windows host (see Mac commands below)

## Mac-host commands (required for Unix-only assertions)

```bash
cd /path/to/cryptex
git fetch && git checkout m2/immutable-event-store && git pull
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked -p cryptex-events -p cryptex-storage
cargo test -p cryptexd --lib canonical_writer --locked
cargo test --locked -p cryptex-storage --lib segment_io::tests::unix_seal_leaves_readable_sealed_pack_after_open_removal
cargo test --locked -p cryptex-storage --lib durability::tests::unix_sync_directory_fails_closed_for_missing_path
pnpm install --frozen-lockfile && pnpm --filter @cryptex/schemas check:drift
```

## Non-goals

- Dispatching Actions from this agent

## Next

- Owner review of this tip, then **one** more dual-OS `ci.yml` readiness run for the required green final gate.
