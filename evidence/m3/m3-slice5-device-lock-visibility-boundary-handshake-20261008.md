# M3 Slice 5 follow-up - Lock visibility, boundary-before-mutate, deterministic handshake

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before follow-up:** `c760a047a7d3191128438f02ca7e84c95a13fb91`  
**Parent:** `evidence/m3/m3-slice5-device-canonical-lock-reader-20261008.md`

## Changes

1. **Boundary before mutation** — `acquire_device_canonical_lock` calls `assert_outside_repository` before `create_dir_all` / lock-file creation. Rejected roots leave no device directory (covered for append, for_each, and direct acquire).

2. **Crate-private lock API** — `DeviceCanonicalLock`, `acquire_device_canonical_lock`, `append_memory_note_with_lock`, and `for_each_verified_event_with_lock` are `pub(crate)`. The lock stores `device_id`; locked helpers reject device A lock + device B target.

3. **Deterministic reader-at-lock handshake** — Test hook `set_before_device_lock_exclusive_hook(thread_id, hook)` runs after opening `writer.lock` and before `lock_exclusive`, scoped to the reader thread. The regression barriers prove: writer holds lock with incomplete trailing bytes → reader reaches the shared lock site → probe `try_lock_exclusive` fails and open length is unchanged → writer completes append under the lock → two events verify.

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3`

- `cargo fmt --all -- --check` - PASS
- `cargo test -p cryptexd --lib canonical_writer::` - PASS (18)
- `cargo test -p cryptexd --lib projection::` - PASS (11)
- `cargo clippy -p cryptex-projection -p cryptexd --all-targets -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

Pending until actually run on a Mac.

## Continuation

Do not start Slice 6 until this follow-up tip is reviewed.
