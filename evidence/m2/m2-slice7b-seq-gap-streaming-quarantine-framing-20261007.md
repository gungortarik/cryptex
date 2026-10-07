# M2 Slice 7b — Sequence Gaps, Streaming Sealed Quarantine, Framing Reasons

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `05caec01bc35ce9837718b3694d9fc8fe4b4ef37`

## Scope

1. **Sequence semantics (ADR-0015):** `scan_pack_events` requires valid `u64` sequences that are unique and strictly increasing. Contiguity is not required (durable allocate-before-append may leave gaps). Test: allocate without append, retry append, accept later event with unbroken hash chain.
2. **Sealed quarantine:** `quarantine_sealed_pack` streams exact full sealed bytes via `quarantine_file_range` (no `fs::read` of the whole pack) before returning the validation failure.
3. **`.open` recovery reason codes:** incomplete header/body → `truncated_tail`; complete invalid magic/version/length header → `corrupt_frame_header`. Reason-code tests for short header, short body, and invalid magic.

## Local verification (Windows)

- `cargo test -p cryptex-storage --lib recovery --locked` — PASS (9)
- `cargo test -p cryptexd --lib canonical_writer --locked` — PASS (10)

## Non-goals

- GitHub Actions / final M2 CI gate
- Merge or M2 completion claim

## Next

- Remaining cross-platform failure matrix; final CI once at candidate.
