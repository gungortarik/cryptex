# Incoming validation ownership and interruption checks

Date: 2026-11-07. Base revision: `f5b3c8f`.

## Problem and change

`validate_incoming` previously opened an import staging session before trying
the runtime instance lock. Even when a normal daemon owned that runtime, the
command could create import directories and temporarily occupy its staging
slot before refusing. That order also let an import-slot conflict mask a live
daemon conflict.

The command now checks the existing absolute, outside-repository root, retains
the staging module's platform directory handles, and acquires instance ownership
before `ImportStagingSession::begin`. Ownership survives validation and discard.
Missing roots are rejected without creation. This reuses existing platform
checks; it does not claim new filesystem-hardening parity on macOS.

## Verified on the Windows development host

- `cargo test --locked -p cryptexd --lib incoming_validation::tests -- --test-threads=1`:
  9 passed, 0 failed (includes the subprocess worker entry).
- Existing running-daemon test now asserts no import journal/staging directories
  were created. Another test holds both instance and staging locks and checks
  that the instance refusal wins.
- Missing/relative root rejection does not create a runtime.
- A child process exits without destructors after complete staging, then in a
  second run after successful policy verification while canonical locks remain
  held. Both children report the exact expected boundary and exit code.
- In each case retry refuses the leftover staging with `RecoveryConflict`,
  preserving evidence. Recovery runs twice under a newly acquired instance lock.
  Canonical sealed segment paths/bytes and incoming source bytes stay identical;
  staging becomes empty and the import journal contains only its lock.
- `cargo clippy --locked -p cryptexd --lib --bin cryptexd --tests -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` passed.

The first test-harness run failed because the snapshot helper treated the
inventory lock file as a device directory. The helper now enumerates device
directories before reading sealed segments; the complete targeted suite was
rerun successfully. No product assertion was relaxed.

## Remaining boundary

This covers unprepared validation-only interruptions, using a fake secure store
and synthetic local-author events. It does not activate canonical publication,
prove power-loss durability, exercise the VM/OS credential store, or validate
macOS. No release package or full daemon suite was run in this change.

The next recovery implementation is the existing ADR-0019 section 5.1.1 branch
for unmarked prepared imports with visible destinations: retain full policy and
canonical locks across remaining no-replace moves and committed marker creation,
with exact-manifest checks and repeatable interruption tests. The current code
continues to hold that state; publication activation and independent review
remain open. The completed one-VM/two-profile pairing and repeat checks should
not be repeated for this work.
