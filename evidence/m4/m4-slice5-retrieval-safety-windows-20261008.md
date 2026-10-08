# M4 Slice 5 — Retrieval safety gate (Windows local)

**Status:** backend/library gate verified on Windows and macOS; Desktop retrieval integration remains open.

## Change

- Added a daemon-issued `RetrievalSafetyState` and fail-closed evaluator for
  verified canonical history, supported semantics, pending dependencies,
  M4 materialization watermarks, and exact M3 projection contents.
- Added a guarded M3 search wrapper. It pins the Brain ID regardless of caller
  filters and holds canonical locks through the content query. Unknown,
  unverifiable, unsupported, unintegrated-v2, stale, or inconsistent state
  withholds the whole Brain.
- Added `canonical/events/inventory.lock`. First-time daemon device-stream
  creation takes it before publishing a directory; full-history scans hold it
  while enumerating and scanning, and acquire existing per-device locks in
  stable order. This closes the daemon-owned first-device inventory race.
- Regressions cover missing/stale safety metadata, tampered M3 text, a new
  canonical event after materialization, and a first-device writer blocked by
  an inventory scan.

## Windows verification

Environment: Windows, isolated target directory under `%LOCALAPPDATA%`.

- `cargo fmt --all -- --check` — PASS
- `cargo test --locked -p cryptexd --lib -- --test-threads=16` — **160/160 PASS**
- `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` — PASS
- `git diff --check` — PASS

## macOS verification

Owner-reported exact-source run:

- SHA: `5d5ad11af8de3996cab831207b343c48431755f2`
- Host: macOS 27.0.1 (26A434), arm64; branch `m4/knowledge-engine`, clean
- `cargo fmt --all -- --check` — exit 0
- `cargo test --locked -p cryptexd --lib` — **155/155 PASS**
- `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` — exit 0

The lower macOS test count is due to platform-specific test gating; Windows-only
tests account for the difference from the 160-test Windows run.

## Boundaries / remaining work

- Production v2 writing remains disabled.
- The current Desktop has no memory-search/result command; `cryptexd` IPC still
  exposes only M0 `health.ping`. Therefore this record does not claim Desktop
  retrieval enforcement or completion of the full Slice 5 acceptance gate.
  Any Desktop route must remain daemon-owned and satisfy the accepted IPC
  authorization boundary; no new IPC operation was introduced here.
- The inventory lock serializes daemon-owned first-device creation. A future
  filesystem sync/import actor must join the same lock protocol; inventory is
  also rechecked after locks and after scans, but arbitrary external filesystem
  mutation cannot be serialized by this daemon lock alone.
- No Actions were run; this local dual-OS verification does not replace M4's
  final reviewed-candidate CI gate.

