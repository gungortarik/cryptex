# Prepared import recovery finisher candidate

Date: 2026-11-07. Base revision: `059dc94`.

## Scope and behavior

This implements the unmarked, already-visible prepared-import completion branch
from ADR-0019 section 5.1.1 as a private candidate. It is not connected to daemon
startup, CLI or transport. Existing startup still returns its explicit hold for
these transactions. No publication activation gate is waived.

`finish_visible_prepared_recovery` requires a matching live runtime instance
lock. It pins the existing runtime/staging/journal directories using current
platform helpers and requires staging and canonical events on the same volume.
It reads the exact prepared manifest, loads the local secure-store key binding,
opens the existing projection read-only and invokes the existing recovery
verifier. That verifier reconstructs pre-import history excluding only
manifest-bound destinations, verifies signatures and contiguous chains, and
reruns post-import trust/cutoff and retrieval policy.

The resulting policy token retains the canonical inventory/device locks. Only
then does the finisher take the existing import slot. It rejects extra journal
entries/markers, changed manifests or transaction directories, and unexpected
staging entries. Each segment must exist in exactly one location and match its
manifest; active-file conflicts and staged aliases of canonical files reject.
Before each missing no-replace move and before marker creation, the entire
batch and journal are rechecked. Existing canonical files are never overwritten.
The existing platform placement/sync/read-back primitive writes the marker
while the locks remain held. Cleanup remains the separate existing committed
recovery path after the finisher returns.

Transactions with no visible destination are refused by this finisher and must
use the existing abort path. Marker-bearing transactions also remain the
responsibility of the existing marker recovery branch.

## Verification on the Windows development host

`cargo test --locked -p cryptexd --lib import_staging::tests -- --test-threads=1`
passed **57 tests**, including existing tests and subprocess worker entries.
New tests cover:

- A two-segment batch with one or both destinations already visible; successful
  completion followed by two cleanup passes preserves exact segment digests
  and the original pre-import segment bytes.
- Eight refusals with evidence preserved and no remaining-segment publication:
  nothing visible, corrupt destination, missing staged file, duplicate
  staged/destination copies, active segment conflict, unknown journal entry,
  corrupted signature even with a matching recomputed manifest digest, and an
  instance lock belonging to another runtime.
- A three-segment batch interrupted in real child processes after the first
  recovery move, after the final recovery move, after marker temporary sync,
  and after final marker sync/read-back. Each exact handshake and abrupt exit
  is checked. A fresh lock owner finishes recovery and repeats cleanup; exact
  destinations remain, staging is empty, and only the journal lock remains.

The subprocess tests pass synthetic fixture credentials through child stdin
into a fake secure store; they do not use or export OS credentials.

Strict daemon library/binary/tests Clippy (`-D warnings`), Rust format check,
and diff whitespace checks passed. The earlier prepared-policy verification
regression also passed before running the full import-staging suite.

## Remaining activation gates

These tests cover Windows process interruption with synthetic local-author
streams. They do not prove power-loss durability, macOS parity, mixed-device
native import, independent security review, or full milestone completion. No
new Windows pilot package, full daemon suite or CI run is claimed.

Before connecting the finisher to startup and activating normal publication,
audit all branches together. In particular, ADR-0019 requires policy
revalidation for temporary-marker completion and repeatable cleanup across its
own interruptions; the existing marker path must not be assumed to satisfy
those requirements merely because this candidate passes. Review the complete
ordering with startup readers, instance ownership and import/canonical locks.
Then obtain the remaining platform/durability and independent-review evidence.
