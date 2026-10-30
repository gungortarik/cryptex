# M8 Windows cutoff-to-reducer bridge — 2026-10-29

## Scope

This slice connects authenticated `VerifiedSecurityHistory` and locally
reduced cutoffs to the M4 content reducer, then uses that policy snapshot in
`RetrievalSafetyState`. The local key resolver supplies only the local public
identity anchor. Under the canonical inventory lock, retrieval enumerates the
complete device inventory, locks every stream, verifies peer keys from local
approval transcripts, and derives cutoffs before selecting content. V3
security policy records are excluded from materialization. The snapshot keeps
all inventory/device locks through SQLite freshness checks and the final
content query.

The legacy generic M4 rebuild/import-tip scanner is unchanged. V2 content
semantics remain held by the current retrieval gate; no transport or canonical
publication was enabled. This is not M8 completion or independent security
approval.

## Checks

- `cargo fmt --all` — PASS
- `cargo fmt --all -- --check` — PASS
- `git diff --check` — PASS
- `cargo test -p cryptex-events --lib` — **80/80 PASS**
- `cargo test -p cryptex-knowledge --lib` — **36/36 PASS**
- Focused knowledge cutoff regression — PASS
- Focused daemon local-anchor guarded-search regression — PASS
- `cargo test -p cryptexd` — library **485 passed / 0 failed / 4 ignored**;
  binary **12/12**, lifecycle **9/9**, supervisor **3/3**, rollback **2/2**
- `cargo clippy -p cryptex-events -p cryptex-knowledge -p cryptexd --lib --tests --release -- -D warnings` — PASS

## Coverage

- Mixed v1/v2/v3 history converts to v1/v2 content records only.
- Device cutoff boundary and missing-cutoff exclusion remain enforced.
- The knowledge reducer materializes an assertion when its record is within
  cutoff and excludes it when the device has no admitted cutoff.
- The lock-scoped policy snapshot passes its own reducer-produced cutoff map
  into content reduction.
- The local-anchor retrieval regression proves a local v1 note remains
  retrievable only after the filtered canonical snapshot, M3 rows, and M4
  materialization state agree; projection tampering and later canonical
  append continue to hold retrieval.
- The cutoff conversion and knowledge reduction expose cancellation paths;
  regression tests confirm cancellation returns no partial result.

## Remaining gates

- Preserve fail-closed behavior for v2 privacy semantics, incomplete history,
  unresolved dependencies, and stale projections.
- Run the full daemon test suite and native Windows retrieval exercise.
- Windows native execution, independent review, and macOS parity remain open.
