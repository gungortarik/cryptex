# M4 Slice 5 — Daemon Verified Rebuild Path

**Status:** Windows locally verified; macOS verification pending.

## Change

Added `cryptexd::knowledge_materialization::rebuild_knowledge_from_canonical`.
The caller supplies an explicit trusted-device set and trusted-daemon key
resolver; trust is never inferred from event contents. The function acquires
device canonical locks in stable `DeviceId` order and holds them through frame
collection, cross-version chain verification, deterministic reduction, and the
M4 SQLite transaction.

The explicit trust set must cover every on-disk canonical device directory;
unlisted or malformed directories stop rebuild instead of silently omitting a
stream. Devices with no canonical directory yet are allowed.

The scan is read-only. It reports original frame offsets, rejects conflicting
pack layouts, requires sealed packs to be fully framed, and treats an
incomplete active `.open` tail as an uncommitted suffix. It does not quarantine,
truncate, seal, or rewrite canonical bytes. Unknown/unverifiable envelopes stop
the whole rebuild before any M4 database write. Verified-unsupported events
advance integrity progress only; safe progress stops before the first such
event. Pending dependencies likewise block the source device's safe watermark.

No v2 production writer or retrieval endpoint was enabled by this change.
Shared `RetrievalSafetyState` enforcement remains open in Slice 5.

## Verification

Using `CARGO_TARGET_DIR=%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m4`:

- `cargo fmt -p cryptexd`: PASS
- `cargo check --locked -p cryptexd --all-targets`: PASS (including the new tests)
- `cargo clippy --locked -p cryptexd --all-targets -- -D warnings`: PASS
- `git diff --check`: PASS
- An initial focused test launch was blocked by Smart App Control (4551). A
  subsequent launch succeeded without policy changes; all focused tests passed.
- `cargo test --locked -p cryptexd --lib -- --test-threads=16`: PASS — 157 tests
- `cargo test --locked -p cryptex-knowledge`: PASS — 29 tests
- `cargo test --locked -p cryptex-storage`: PASS — 41 tests

## Security review follow-up

A read-only audit found two issues in the first candidate: an incomplete active
tail could be omitted, and malformed pack-like names could be ignored. Both
are fixed. An active `.open` pack whose verified frame length differs from its
file length now blocks before migration or M4 writes; malformed pack filenames
and unexpected directory entries also fail closed. Regressions prove the
canonical bytes and preexisting SQLite state remain unchanged. Focused daemon
materialization tests: 7/7 PASS; full daemon library suite: 157/157 PASS.
- macOS focused execution: pending.
- GitHub Actions: not run.

New regression tests cover verified v1 rebuild with byte-identical canonical
input, unchanged SQLite state on an unverifiable envelope, and a pending
dependency holding the safe watermark before its frame.

## Follow-up

Run on macOS:

```bash
cargo fmt --all -- --check
cargo test --locked -p cryptexd --lib knowledge_materialization:: -- --test-threads=16
cargo test --locked -p cryptex-storage
cargo clippy --locked -p cryptexd --all-targets -- -D warnings
```

Then continue Slice 5 with shared retrieval-safety enforcement and late-arrival
full-rebuild parity. M4 is not complete; production v2 writing remains disabled.
