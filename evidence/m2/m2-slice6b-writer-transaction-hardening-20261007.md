# M2 Slice 6b — Canonical Writer Transaction Hardening

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`

## Scope

- Exclusive per-device `writer.lock` across reconcile → recover → allocate → append/seal → tip persist.
- `writer_tip.json` treated as rebuildable derived cache; chain tip / active segment reconstructed from verified `.seg`/`.open`.
- Stale/corrupt tip healed by rebuild; unverifiable sealed packs quarantined and fail closed.
- Crash injection: durable append before tip, and seal before tip — retry does not fork the hash chain.
- Concurrent same-device writers: unique strictly-increasing sequences and unbroken chain links.

## Local verification (Windows)

- `cargo test -p cryptexd --lib canonical_writer --locked` — PASS (6)
- `cargo test -p cryptex-storage --locked` — PASS
