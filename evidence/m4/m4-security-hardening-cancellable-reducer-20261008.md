# M4 security hardening — cancellable reducer and bounded retrieval

**Code SHA:** `c2cf07bf0662589a9932c201f5e9e749e6a1013a`
**Branch:** `m4/knowledge-engine`
**Windows verification:** PASS
**macOS verification:** WAITING for this SHA
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

## macOS verification required

The owner-reported macOS PASS at `b6d652da07c175d086c625bfb498cf370dfc5fe2` is valid for that earlier code SHA, which is an ancestor of this commit. It does not cover the reducer cancellation, union-find, or bounded worker changes in `c2cf07b`.

On macOS, check out this exact SHA and run:

```sh
git checkout c2cf07bf0662589a9932c201f5e9e749e6a1013a
git rev-parse HEAD
cargo fmt --all -- --check
cargo test --locked -p cryptex-knowledge
cargo test --locked -p cryptexd
cargo clippy --locked -p cryptex-knowledge -p cryptexd --all-targets -- -D warnings
cargo check --locked -p cryptex-desktop
git diff --check
```

Physical visual review, the final DoD review, human review, and the single required dual-OS Actions gate remain open. M4 is not complete; no merge or production v2 activation is claimed.
