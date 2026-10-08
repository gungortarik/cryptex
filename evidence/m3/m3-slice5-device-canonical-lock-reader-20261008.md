# M3 Slice 5 blocking fix - Shared device canonical lock for verified reader

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before fix:** `576e4d5c16021d665f808ec8c6cc69e928135768`

## Problem

`append_memory_note` held per-device `writer.lock`, but `for_each_verified_event` did not. The verified reader performs active `.open` recovery (truncate-after-quarantine) and could race a concurrent canonical append, corrupting or truncating in-progress writer data.

## Fix

- Added shared `acquire_device_canonical_lock` / `DeviceCanonicalLock` (still the `writer.lock` file; canonical pack formats unchanged).
- Always `create_dir_all` the device directory before opening the lock file so the reader cannot skip locking when packs do not exist yet (first-create race).
- `append_memory_note` acquires the lock; `append_memory_note_with_lock` runs the writer body under an already-held lock.
- `for_each_verified_event` acquires the lock for discovery → recovery → full scan/callback; `for_each_verified_event_with_lock` is used when the lock is already held.
- Projection catch-up/rebuild acquires the device lock **before** opening the SQLite transaction (lock order: device → SQLite) and feeds via `for_each_verified_event_with_lock`.

## Tests

- `verified_reader_waits_for_device_lock_and_does_not_truncate_in_progress_append` — deterministic 3-way barrier: writer holds lock with incomplete trailing bytes; probe `try_lock_exclusive` fails; open length unchanged; writer completes append under the same lock; reader proceeds; two events verify.
- `verified_reader_acquires_lock_file_even_when_device_dir_missing` — empty device creates dir + `writer.lock`.
- Existing `canonical_writer::` (16) and `projection::` (11) suites remain green.

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3`

- `cargo fmt --all -- --check` - PASS
- `cargo test -p cryptexd --lib canonical_writer::` - PASS (16)
- `cargo test -p cryptexd --lib projection::` - PASS (11)
- `cargo clippy -p cryptex-projection -p cryptexd --all-targets -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

macOS projection/feed/rebuild/lock smoke remains **pending** until actually run on a Mac.

## Continuation

Do not start Slice 6 until this blocking fix is reviewed.
