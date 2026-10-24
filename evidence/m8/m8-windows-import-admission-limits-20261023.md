# M8 Windows import admission-limit foundation

**Date:** 2026-10-23
**Platform:** Windows x86_64-pc-windows-msvc
**Scope:** Pin and unit-test bounded import batch accounting; no transport or canonical publication.

## Limits

- 16 sealed segment files per prepared batch.
- 1 GiB per file.
- 4 GiB total per batch.
- One prepared import transaction at a time.

The 1 GiB per-file cap accommodates 1,000 ADR-0016 maximum-size event records
and frame headers. These are importer admission limits, not event-format limits.
The transport folder is not read or changed by this code. The admission budget
is not yet connected to a staging copier or journal.

## Implementation and checks

`services/cryptexd/src/import_limits.rs` provides checked per-file and
aggregate accounting. Rejections leave the budget unchanged. Unit tests cover
zero length, per-file limit, file-count limit, aggregate-byte limit, and integer
overflow.

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test -p cryptexd --lib import_limits::tests` | 5 passed |
| `cargo test -p cryptexd --lib` | 398 passed, 0 failed, 4 ignored |
| `cargo clippy -p cryptexd --all-targets -- -D warnings` | PASS |

## Boundary

No Syncthing transport started, no runtime or canonical data touched, and no
import staging, journal persistence/recovery, publication, Windows native
filesystem race test, or macOS parity is claimed. Trust remains disabled. No
Actions, commit, or push.
