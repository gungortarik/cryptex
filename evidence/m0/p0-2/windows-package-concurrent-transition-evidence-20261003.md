# Windows package-candidate concurrent transition regressions

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Result:** PASS, scoped to the test-only filesystem selection candidate  
**Host:** Windows, Node `v24.19.0`  
**Starting source:** `854ac16`  
**Workflow:** none

## Change

The candidate previously captured `before`, staged a private slot, and then
created `pending.json` without checking whether another updater had committed
in the meantime. A stale process could therefore replace a newer selection.

The transition now:

1. compares the active selection with its captured `before` after staging;
2. exclusively creates the pending intent (`wx`), classifying an occupied
   intent as `CONCURRENT_TRANSITION`;
3. rereads selection after intent creation to cover a competing commit between
   the first comparison and intent creation; and
4. removes the intent only when its bytes still decode to the exact journal this
   process created. Unexpected intent data remains for diagnosis.

## Verification

Command:

```powershell
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
```

Result: **31 passed, 0 failed, 0 skipped**. The suite includes deterministic
cases where another candidate commits (a) while the first candidate is staged
and (b) after selection validation but before its intent is created. Both stale
transitions fail with `CONCURRENT_TRANSITION`; the winner remains selected and
no pending journal is left behind. Five existing actual worker-kill boundaries,
recovery checks and synthetic fixture inventory checks also passed.

Additional checks:

- `node --check tools/m0-lifecycle-compare/scripts/package-transaction.mjs` — PASS
- `node --check tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs` — PASS
- `git diff --check` — PASS

No runtime install, canonical store, retained P0-1 corpus, GUI, or GitHub
workflow was accessed. Fixture payloads were synthetic and their before/after
inventory assertion passed.

## Limits

This is not an OS-level process lock or stress test. Native updater handover,
publisher authenticity, recovery-I/O failure behavior, native executable locks,
power-loss durability, production canonical-data preservation, and Mac parity
remain open. ADR-0012 remains PROPOSED.
