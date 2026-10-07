# M2 Slice 8 — Cross-Platform Matrix Coverage + CI Wiring

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `93cea75dbaeecd784016f680aa6e4edbf888efe3`

## Scope

- Reconciled slice 7b framing evidence with full `corrupt_frame_header` coverage (magic, version, length) plus `truncated_tail` incomplete header/body.
- Added platform-sensitive M2 placement/sync tests (`#[cfg(windows)]` / `#[cfg(unix)]`) without expanding milestone scope.
- Updated `.github/workflows/ci.yml` readiness jobs to run:
  - `cargo test --locked -p cryptex-events -p cryptex-storage`
  - `pnpm --filter @cryptex/schemas check:drift`
- Did **not** dispatch Actions (minutes reserved for final candidate).

## Local verification (Windows host)

- `cargo test --locked -p cryptex-events -p cryptex-storage` — PASS
- `cargo test -p cryptexd --lib canonical_writer --locked` — PASS
- `pnpm --filter @cryptex/schemas check:drift` — PASS
- `cargo fmt -p cryptex-events -p cryptex-storage -p cryptexd` — applied
- Clippy: blocked locally by SAC 4551 (unchanged historical constraint)

## Mac-host commands still needed (Unix-only assertions / parity)

```bash
cd /path/to/cryptex
git checkout m2/immutable-event-store
git pull
cargo test --locked -p cryptex-events -p cryptex-storage
cargo test -p cryptexd --lib canonical_writer --locked
# Explicitly exercises #[cfg(unix)] seal hard-link + sync_directory missing-path tests:
cargo test --locked -p cryptex-storage --lib segment_io::tests::unix_seal_leaves_readable_sealed_pack_after_open_removal
cargo test --locked -p cryptex-storage --lib durability::tests::unix_sync_directory_fails_closed_for_missing_path
pnpm install --frozen-lockfile
pnpm --filter @cryptex/schemas check:drift
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

## Final CI command (after candidate review only — do not run yet)

Dispatch the existing workflow once on the reviewed tip (both readiness OS matrix cells):

```bash
gh workflow run ci.yml --ref m2/immutable-event-store
# or open a PR into the agreed base and rely on pull_request readiness jobs
```

Confirm the readiness job for **windows-latest** and **macos-latest** each execute the new M2 crate tests and schema drift steps. Record the Actions run URL in handoff only after green.

## Non-goals

- Running Actions now
- Merge / M2 complete claim
