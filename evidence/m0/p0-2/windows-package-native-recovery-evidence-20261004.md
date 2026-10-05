# Windows native interrupted recovery cleanup — functional PASS

**Starting source:** `335a0b7`  
**Scope:** isolated M0 P0-2 candidate, not installed updater integration  
**Remaining verification:** Rust formatting and Mac parity

Added `clear-interrupted-recovery-claim` to the lifecycle tool, using the existing
locked fs2 0.4.3 version. Cargo.lock adds only a direct dependency edge; no
version upgrade. An exclusive OS lock serializes stale-claim inspection/removal.
The lock file remains in the owned root, never unlinked. Root/marker/ordinary
file checks precede mutations; links, junctions and repository paths are refused.

A bounded Node owner probe must return ESRCH; live/ambiguous owners are refused.
Claim bytes are rechecked while the native lock is held before unlink. The Node
adapter then calls existing guarded recovery. A later new owner can win the claim
and cause safe refusal; no forced recovery or installed-app operation is added.

## Verification

- Rust lifecycle tool tests: **13/13 PASS** (three new validation tests).
- Strict Clippy and locked helper build: PASS.
- Combined Node suite: **61/61 PASS** (51 transaction, 4 health adapter, 6 native
  recovery), zero failures/skips.
- New Node adapter/test syntax checks and diff check: PASS.
- `cargo fmt --all`: BLOCKED by Application Control, os error 4551. No policy
  change or alternate blocked-binary execution was attempted. Format remains
  unverified; Mac plan includes an authorized formatting-only repair if needed.

Six native cases verify live claim preservation; actual killed-recovery claim
reclamation followed by baseline/idempotent recovery; competing cleaners removing
exactly one old claim and subsequently refusing a new live claim; malformed
claim preservation; actual SIGKILL of an OS-lock holder and later reacquisition;
and repository-root refusal. Fixtures remain synthetic, inventory assertions
pass, and only owned subprocesses are stopped. No GUI, installed runtime or
GitHub Actions was used.

## Limits

The native lock serializes stale-claim cleanup only. The helper's `--hold-ms`
option is a test checkpoint, not runtime policy. Default recover still refuses
interrupted claims; the new explicit adapter enables cleanup plus retry.
PID reuse or ambiguous owner state conservatively blocks cleanup. Broad stress,
same-user adversarial tampering, installed updater handover, publisher signing,
canonical-data acceptance and machine power loss remain open. Windows directory
fsync remains unimplemented. ADR-0012 stays PROPOSED.
