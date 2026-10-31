# Windows concurrent recovery exclusion — scoped PASS

**Starting source:** `28c4ea0`  
**Scope:** M0 P0-2 isolated filesystem package candidate

The PID owner guard alone allowed two recovery callers to modify the same dead
updater's intent. Recovery now exclusively creates `recovery-claim.json` before
reading intent. Existing claim causes `RECOVERY_BUSY`; updater checks the claim
before staging and after securing intent, before activation. Normal recovery
checks its own claim contents before removing it. The claim is test-only metadata.

Three new separate-process cases passed:

1. Recovery pauses before selection rename while holding claim. A second
   recovery and new updater are refused. Journal/selection bytes remain intact.
   First recovery finishes, releases claim, and a successor updater commits.
2. An updater prevalidates before another updater leaves a pending transaction.
   Recovery restores baseline and pauses after deleting the old intent. The
   prevalidated updater creates its own intent but refuses activation while
   recovery owns the claim. Old recovery completes without deleting that newer
   intent. After its owner exits, subsequent recovery completes idempotently.
3. Recovery is force-stopped before selection rename. Claim and journal bytes
   remain unchanged; another recovery and updater are refused. The test does
   not remove the claim to manufacture a recovery success.

## Checks

Combined transaction/health adapter suite: **51 + 4 = 55/55 PASS**, zero failures
or skips. Four syntax checks (engine, process harness, contender, tests) and
diff check passed. Prior I/O faults, owner guards, process races and five updater
SIGKILL boundaries passed. Synthetic fixture inventories and owned child cleanup
passed. No GUI, installer, startup registration, canonical data or Actions used.

## Limits

Interrupted recovery claims require inspection and are never automatically
reclaimed, including when their PID has exited. This is a safe refusal with an
availability limitation, not native process locking or automatic crash recovery.
Claim-release I/O failure and broad scheduler stress are not measured here.
Same-user adversarial tampering, native handover, signing, power-loss durability
and canonical-data acceptance remain open. Mac parity is pending; ADR-0012
remains PROPOSED. No retained experiment is rewritten or cleaned by these tests.
