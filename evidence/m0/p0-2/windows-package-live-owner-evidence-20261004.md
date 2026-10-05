# Windows live-updater recovery owner guard — scoped PASS

**Starting source:** `69e585b`  
**Scope:** M0 P0-2 isolated package transaction candidate

Recovery previously restored any valid pending intent, including an active
updater's transaction. New journals use schema 2 with owner PID and transaction
UUID. External recovery requires OS `ESRCH` for the recorded PID; live or
ambiguous processes are refused. Legacy schema-1 intents lack ownership and
are retained with `RECOVERY_OWNER_UNKNOWN`. They are not silently migrated.
Internal automatic rollback requires the exact journal of its invocation.

## Seven new regressions

- At each of `intent`, `selected`, and `healthy`, a separate recovery process
  exits 2 with `RECOVERY_OWNER_ACTIVE`. Journal and selection bytes are untouched.
  After terminating and awaiting only the owned updater, another recovery
  process exits 0 and restores baseline; repeated recovery is idempotent.
- A refused recovery leaves the live updater able to resume and commit normally.
- A legacy ownerless journal is retained; automatic recovery is refused.
- A malformed owner PID is rejected with `CORRUPT_JOURNAL`.
- A changed owner token prevents internal automatic rollback with
  `RECOVERY_OWNER_CHANGED`, preserving the journal.

Two previous failed-rollback tests now explicitly require external recovery
refusal while their test-runner owner PID is still alive. This is the new
conservative owner rule; four recovery-I/O tests still retry successfully after
their owned worker has exited. Ordinary automatic rollback continues to pass.

## Verification

Combined Node transaction and health adapter suite: **48 + 4 = 52/52 PASS**,
zero failures/skips. Engine, process harness, contender and test syntax checks
passed; `git diff --check` passed. Synthetic inventory assertions and owned
subprocess cleanup passed. Existing process races and five SIGKILL boundaries
passed. No GUI, installed runtime, startup registration or Actions was used.

## Limits

PID liveness is conservative: PID reuse blocks recovery, and an update call
that ended with failed rollback still blocks external retry until its process
exits. Permission/ambiguous liveness does not grant recovery. This is not an
OS-level lock or authentication against same-user metadata tampering.
Simultaneous recovery callers and broad stress remain unverified. Native
handover, publisher signing, canonical-data acceptance and power-loss durability
remain open. Mac parity is pending; ADR-0012 remains PROPOSED.
