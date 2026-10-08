# M3 Slice 6 follow-up - Exact UTC instant search precision

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Finding:** `search_events` used `rfc3339_unix_seconds()`, which discards fractional seconds accepted by the M2 RFC3339 parser. Inclusive same-second range bounds and deterministic ordering were therefore incorrect.

## Fix

- Added shared `cryptex_core::validate::Rfc3339UtcInstant` + `rfc3339_utc_instant()`.
- Reuses the existing M2 RFC3339 grammar and leap-second clamp (`:60` → `:59` for the Unix timeline).
- Stores normalized whole Unix seconds plus fractional digits (trailing zeros stripped) so `.1` / `.10` / `.100` compare equal without numeric overflow or a new SQLite column.
- `rfc3339_unix_seconds()` now delegates to the exact helper and still discards fractions for callers that only need whole seconds.
- `search_events` range filtering and ordering use exact instants; `event_id` is the tie-break only when instants are exactly equal.

## Tests

- Core: fractional ordering, offset-equivalent fractions, trailing-zero equivalence, whole-second vs fraction.
- Projection search: inclusive fractional lower/upper bounds, same-second distinct timestamps, offset spellings with fractions, ordering, limit-after-ordering.

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3-s6-frac`

- `cargo fmt --all -- --check` - PASS
- `cargo test -p cryptex-core --lib validate::` - PASS (9)
- `cargo test -p cryptex-projection --lib` - PASS (35)
- `cargo test -p cryptexd --lib projection::` - PASS (11)
- `cargo clippy -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

macOS FTS5/feed/rebuild/search smoke remains **pending** until actually run on a Mac.

## Continuation

Do **not** start Slice 7 until this tip is reviewed. Slice 7 remains next after review of the corrected Slice 6 tip.
