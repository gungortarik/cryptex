# M8 Windows post-import trust and cutoff gate

Date: 2026-10-29
Scope: validation of a staged import against the authenticated union of the
locked canonical history and staged records. No transport admission or
production publication was enabled.

## Change

- Rebuilt the complete security-history input from exact canonical frame bodies
  and exact, already verified staged record bodies while the canonical
  inventory/device locks remain held.
- Re-ran local-anchor authentication and the ADR trust reducer over that union.
- Required every current canonical device and staged candidate to remain in the
  resulting trusted-device set and have an effective cutoff.
- Accepted staged v1 content only when its exact `(device, sequence, event ID)`
  appears in the post-import cutoff view. Other content versions remain held
  until their retrieval semantics are integrated.
- Reduced cutoff-filtered content and held unresolved or unmaterialized
  assertions. V3 trust controls are policy metadata and are not materialized as
  content.
- Applied the same verifier to ordinary staged batches and prepared-recovery
  batches. The returned token retains canonical locks and the computed
  post-import trust/content state; it is not a publication authorization.

## Regression

Synthetic local history contains a v1 note at sequence 1 and an imported
self-retirement control at sequence 2 with cutoff 1. Both normal batch
verification and prepared recovery derive cutoff 1, and the control event does
not appear in materialized content. Recovery was exercised with an existing
canonical destination and with a read-only projection database.

## Verification

- Focused prepared-recovery regression: **1/1 passed**.
- `cargo test -p cryptexd --lib --bins`: library **513 passed, 0 failed, 4
  ignored**; binary **12 passed, 0 failed**.
- `cargo clippy -p cryptexd --all-targets --release -- -D warnings`: **PASS**.
- `cargo fmt --all -- --check`: **PASS**.
- `git diff --check`: **PASS**.

## Limits

This does not establish production transport-to-staging wiring, Windows
power-loss/directory-durability behavior, macOS parity, or independent security
review. Publication/replay is not authorized by this validation result. M8 is
not complete. No commit, push, or Actions were run.
