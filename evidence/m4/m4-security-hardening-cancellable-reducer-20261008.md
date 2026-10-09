# M4 security hardening — cancellable reducer and bounded retrieval

**Code SHA:** `c2cf07bf0662589a9932c201f5e9e749e6a1013a`
**Branch:** `m4/knowledge-engine`
**Windows verification:** PASS
**macOS verification:** PASS on this SHA
**GitHub Actions:** not run; retain the single required run for the final reviewed candidate.

## Changes

- Desktop search setup and reduction run in a bounded worker. A deadline returns a Brain-wide hold with no partial or late hits; the search reservation remains held until the worker exits.
- Every unauthenticated connection uses the capped worker pool, including health-only daemon mode. Health capacity remains separate from the two-slot Desktop search pool.
- Reducer sorting, event-history scans, merge/revoke processing, alias grouping, and correction dependency graph traversal now check cancellation. Entity equivalence uses deterministic union-find with the lexicographically smallest entity ID as representative.
- Merge/revoke joins and alias groups avoid repeated full-history scans.
- Production v2 writing remains disabled.

## Windows checks on this code

- `cargo fmt --all -- --check` — PASS
- `cargo test --locked -p cryptex-knowledge` — **34/34 PASS**
- `cargo test --locked -p cryptexd --lib ipc::` — **14/14 PASS**
- `cargo test --locked -p cryptexd` — **199 PASS** (lib 183, bin 2, lifecycle 9, Mode B supervisor 3, update rollback 2)
- `cargo clippy --locked -p cryptex-knowledge -p cryptexd --all-targets -- -D warnings` — PASS
- `cargo check --locked -p cryptex-desktop` — PASS
- `git diff --check` — PASS

## Security review

The read-only security re-review found no remaining P1/P2 algorithmic issue, reducer semantic regression, retrieval fail-open, or Windows/macOS portability issue in the changed reducer and IPC code. It confirmed that timed-out work cannot return hits and that its worker slot remains capped until the synchronous operation exits.

Residual availability limitation: a synchronous OS or SQLite call that does not return can retain one of two search reservations until it returns. The daemon then responds `search_busy`; this does not expose results or consume the separate health-worker capacity.

## Exact-SHA macOS verification

Owner-reported verification at exact SHA `c2cf07bf0662589a9932c201f5e9e749e6a1013a`:

- Host: macOS 27.0.1 (26A434), arm64; working tree clean before and after.
- `cargo fmt --all -- --check` — PASS.
- `cargo test --locked -p cryptex-knowledge` — **34/34 PASS**.
- `cargo test --locked -p cryptexd` — **194 PASS** (lib 178, bin 2, lifecycle 9, Mode B 3, update rollback 2).
- Strict Clippy for `cryptex-knowledge` and `cryptexd` — PASS.
- `cargo check --locked -p cryptex-desktop` — PASS.
- `git diff --check` — PASS.

The earlier macOS report at `b6d652d` did not cover these hardening changes; this exact-SHA run closes that cross-platform verification gap.

Physical visual review, final DoD/human review, and the single required dual-OS Actions gate remain open. M4 is not complete; no merge or production v2 activation is claimed.
