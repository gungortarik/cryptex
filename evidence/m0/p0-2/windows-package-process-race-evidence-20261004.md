# Windows separate-process package race proof — scoped PASS

**Starting source:** `1e80cbf`  
**Scope:** M0 P0-2 isolated filesystem candidate, synthetic health/payloads

Added an IPC-controlled test contender and bounded parent harness. Each child
executes the existing transaction engine in its own Node process; a checkpoint
message identifies the exact owned PID. Parent messages release that checkpoint.
Workers launch with `windowsHide`, return structured results, and are awaited
or force-stopped by their owned child handles before fixture removal.

Four separate-process tests passed:

1. First process pauses after staging; second commits. First resumes and exits
   2 with `CONCURRENT_TRANSITION`; second exits 0 and its selection is retained.
2. First pauses after selection validation; second commits. First resumes and
   detects stale state after securing intent. Winner is preserved, intent removed.
3. First owns pending intent; second exits 2 with `RECOVERY_REQUIRED`, leaving
   intent bytes untouched. First resumes and commits successfully.
4. Both processes pause after validation and are released together. Exactly
   one exits 0, one exits 2 with `CONCURRENT_TRANSITION`; the complete winner
   is selected with baseline as previous.

Post-exit recovery reports no pending transaction. Synthetic data inventory
checks pass, and owned temporary roots are removed by test cleanup. Actual
transaction engine behavior was unchanged in this slice.

## Checks

```powershell
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction-process.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction-contender.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
git diff --check
```

**45/45 PASS**: 41 transaction + 4 health adapter tests, no failures/skips.
Includes prior recovery faults, controlled in-process races and five SIGKILL
boundaries. Syntax and diff checks passed.

## Limits

This measures four controlled races, not broad scheduler stress or OS-level
process locking. Recovery running concurrently with an active transaction is
unverified. Health is injected and package payloads are synthetic. Native
Desktop/daemon handover, signatures, canonical-data preservation and power-loss
durability remain open. Mac parity is pending; ADR-0012 remains PROPOSED.
No GUI, installer, startup registration, installed runtime or Actions was used.
