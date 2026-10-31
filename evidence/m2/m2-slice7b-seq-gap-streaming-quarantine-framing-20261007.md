# M2 Slice 7b — Sequence Gaps, Streaming Sealed Quarantine, Framing Reasons

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `93cea75dbaeecd784016f680aa6e4edbf888efe3`

## Scope

1. **Sequence semantics (ADR-0015):** `scan_pack_events` requires valid `u64` sequences that are unique and strictly increasing. Contiguity is not required (durable allocate-before-append may leave gaps). Test: allocate without append, retry append, accept later event with unbroken hash chain.
2. **Sealed quarantine:** `quarantine_sealed_pack` streams exact full sealed bytes via `quarantine_file_range` (no `fs::read` of the whole pack) before returning the validation failure.
3. **`.open` recovery reason codes:**
   - incomplete header/body → `truncated_tail` (tests: short header, short body)
   - complete invalid magic/version/length header → `corrupt_frame_header` (tests: invalid magic, unsupported `frame_ver`, `body_len` above ADR max)

## Local verification (Windows)

- `cargo test --locked -p cryptex-events -p cryptex-storage` — PASS (`cryptex-events` 38; `cryptex-storage` 29 including Windows seal placement)
- `cargo test -p cryptexd --lib canonical_writer --locked` — PASS (10)
- `pnpm --filter @cryptex/schemas check:drift` — PASS
- Local Clippy blocked by Smart App Control (4551); CI Clippy remains the gate

## Platform-sensitive coverage (this host + CI)

- Windows: `windows_seal_moves_open_to_sealed_pack` (MoveFileEx path); quarantine `MoveFileExW` exercised by quarantine suite
- macOS (CI / Mac host): `unix_seal_leaves_readable_sealed_pack_after_open_removal`; `unix_sync_directory_fails_closed_for_missing_path`
- Shared: divergent `.open`/`.seg` fail-closed; directory-sync failure injection

## Non-goals

- GitHub Actions / final M2 CI gate (workflow updated but not dispatched)
- Merge or M2 completion claim

## Next

- Candidate review, then single final dual-OS CI readiness run; Mac-host commands below for any local Unix confirmation.
