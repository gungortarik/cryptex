# Windows package recovery I/O faults — scoped PASS

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Starting source:** `56919da`  
**Host:** Windows, Node `v24.19.0`  
**Scope:** M0 P0-2 test-only filesystem candidate

Recovery previously exposed raw mutation errors without a recovery phase.
It now reports `RECOVERY_IO_FAILED`, retains the original exception as `cause`,
and attaches `diagnostics.phase`, `diagnostics.io_code`, and
`diagnostics.intent_present`. Automatic health rollback uses the same handler.
Defaults still use native fs; optional I/O adapters enable isolated fault tests.

## Six new cases

| Injected fault | State after failure | Retry |
| --- | --- | --- |
| Temporary selection write, ENOSPC | Candidate selected, journal retained | Baseline recovered |
| Temporary selection file fsync, EIO | Candidate selected, journal retained | Baseline recovered |
| Selection rename, EACCES | Candidate selected, journal retained | Baseline recovered |
| Intent unlink, EACCES | Baseline selected, journal retained | Baseline recovery completed |
| Rejected health then rollback rename, EACCES | Candidate selected, journal retained; no restoration claim | Baseline recovered |
| Throwing health then rollback rename, EACCES | Candidate selected, journal retained; no restoration claim | Baseline recovered |

The four direct cases start after an actual owned-worker SIGKILL at `selected`.
Their journal bytes and both complete slot inventories remain unchanged during
the fault. Repeated normal recovery is idempotent. All tests compare untouched
synthetic data fixture membership/hashes and clean owned temporary roots.

## Checks

```powershell
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
git diff --check
```

**41/41 passed**: 37 transaction tests and 4 health adapter tests, zero failures
or skips. Syntax and diff checks passed. No Rust behavior changed.

## Limits

Fault codes are injected at filesystem call boundaries; this is not actual
disk exhaustion, native ACL denial or a physical device error. Write failure
occurs before payload write, not after a measured partial write. POSIX directory
flush failures after successful intent unlink may leave no journal; the error
reports completion as unestablished and exposes observed intent presence.
Windows directory fsync remains unimplemented. Native durability, power loss,
process-lock/concurrency safety, publisher authenticity, installed Desktop
handover, and canonical-data acceptance remain open. ADR-0012 is PROPOSED.
No GUI, installer, login registration, retained P0-1 corpus or Actions was used.
