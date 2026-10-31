# M5 Slice 4 -- derived catch-up and restart reconstruction (Windows)

**Status:** Windows-focused local verification PASS; macOS and native packaged verification remain open.
**Product SHA:** `7637ebc4bcf8a55bcf1ada9c2834820d4fa8373f` (`feat(m5): catch up derived state after capture and restart`)
**Base:** `d564069ce43cf141f2a14edd21a1398ea691d4d0`
**Host:** Windows 11 x64 workspace
**Authorization:** Owner replied “hadi” to the explicit Slice 4 preparation/continuation request; scope is Slice 4 item 4 in `evidence/m5/m5-kickoff-plan-20261010.md` only.

## Behavior

- Starts one bounded daemon-owned derived-state worker for the authenticated Desktop IPC server. It schedules catch-up at daemon startup so canonical commits can be reconstructed after a daemon restart.
- A `Committed` capture or `capture_resolve` queues derived catch-up without waiting for SQLite or changing the canonical capture outcome. A failed catch-up leaves canonical memory intact and retrieval guarded.
- When guarded search finds missing/stale M3/M4 materialization, it schedules a retry and returns the existing hold response. The UI can retry after catch-up completes.
- The worker opens/replaces only unusable derived projection state, incrementally catches the local device's M3 projection up from verified canonical events, then rebuilds M4 knowledge materialization through the existing verified canonical scan. Canonical files remain read-only to this path.
- Catch-up failures are logged only by stable error category; no memory body, identity value or filesystem path is logged.
- `cryptex-projection` entry points accept the daemon's `dyn SecureStore` context (`?Sized`), retaining the existing single-writer and lock-order behavior.

## Windows checks

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p cryptexd --lib ipc::tests::` | **34/34 PASS** |
| `cargo test --locked -p cryptexd --lib projection::` | **17/17 PASS** |
| `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` | PASS |

The new IPC tests cover a missing projection database, fail-closed first search
and catch-up scheduling, canonical-to-derived rebuild followed by searchable
results, daemon-start catch-up, and the rule that committed capture still
returns its canonical outcome while scheduling indexing work.

## Limits / next verification

- Catch-up is asynchronous. A search arriving before the worker finishes may
  receive the existing retrieval hold; retry after catch-up is required.
- M3 catch-up is checkpointed/incremental. M4 currently uses the existing full
  verified-history rebuild; measure performance before expanding dataset size.
- These are Windows local checks, not macOS verification or a packaged native
  app interaction pass. Run the focused Rust checks on macOS at this product SHA
  before closing Slice 4. No power-loss guarantee is claimed.
- No GitHub Actions, merge, Slice 5 timeline work, or M5-complete claim.
