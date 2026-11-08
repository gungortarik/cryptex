# M8 instance-bound runtime admission — 2026-11-08

## Scope and source

The owner requested safe live-transfer read/write coordination. Implementation
commit: `a5429ea150748b64ce7ce25c618327671cdf62d2`, based on the reviewed
`f072ae2` identity/runtime-proof corrections and `989c713` handoff.

This implements the process-local coordination layer required before connecting
the managed transport controller to publication. **Normal incoming publication
still has no external CLI, IPC, watcher or controller activation path.** This
is not an end-to-end ordinary cross-device replication or M8 completion claim.

Authority remains the Master Spec, accepted ADRs and the approved Windows
ADR-0019 publication slice. No canonical event format, immutable byte, trust
grant, transport permission or secure-store namespace was changed.

## Admission and lock lifetime

Each actual `InstanceLock` owns one `RuntimeAdmission`. IPC constructors now
require that matching instance, rather than creating an unrelated gate from a
path. `SearchContext` clones share its admission state. Idle contexts retain a
weak file reference; active operation and exclusive leases pin the same real
OS lock handle. There is no global path-to-gate registry.

Ordinary operations take an owned, cloneable lease **before** canonical work:
search, timeline, event detail, capture/resolve, enrollment, vault generation,
bootstrap exchange/sign/complete, owner-security operations and background
derived catch-up. Health and redacted transport-worker status remain available.

The actual IPC connection retains its lease through serialization and bounded
response writing. Bounded search/timeline/detail/capture workers retain cloned
contexts until their work actually exits, even after the request times out.
A timeout is not worker cancellation or proof that a write did not commit.
Nested derived catch-up reuses its operation's lease, avoiding a second admission
attempt while an exclusive import is pending.

An exclusive import first blocks new ordinary leases, then drains existing
ones. Its admission wait is bounded at ten seconds; timeout/competition returns
typed `AdmissionError::Busy` before staging or canonical mutation. The mutex is
released during the condition-variable wait and before all disk/network work.
This coordinator is distinct from the existing staging reservation: that
reservation is still released before inventory/device locks are acquired.
Canonical inventory then sorted-device lock ordering remains unchanged.

The same exclusive lease covers existing recovery, staging, local-policy and
post-import validation, replay pruning, manifest preparation, no-replace moves,
marker verification/receipt cleanup and derived rebuild. There is no supported
ordinary IPC/derived path admitted between the individual moves or before the
final rebuild outcome is classified.

Bootstrap and owner review sessions release ordinary admission **after** writing
the prepared snapshot and **before** waiting for owner input. They reacquire it
after the complete follow-up frame arrives, before touching journal/canonical
state. An intervening active import/hold refuses confirmation; existing frozen
snapshot, process-binding, one-time capability and canonical revalidation rules
are preserved. Slow or idle framed input does not occupy runtime admission.

Ordinary operations continue to use their existing canonical locks and safety
checks against each other. This layer is exclusive-import coordination, not a
replacement for retrieval/trust verification or a new canonical writer.

## Failure and shutdown outcomes

| Outcome | Runtime admission |
|---|---|
| Successful import plus cleanup/rebuild | Reopens |
| Clean rejection before preparation, including successful staging discard | Reopens; existing retrieval-safety checks still apply |
| Preparation/publication/recovery ambiguity | `RecoveryRequired` hold |
| Canonical commit exists but derived refresh fails | `DerivedRebuildRequired` hold |
| Abandoned exclusive lease or a panicking ordinary worker | Recovery hold, including when a healthy response thread retains another lease clone |
| Poisoned coordination mutex | Refuses ordinary and recovery admission; never silently clears poison |
| Shutdown | Stops new admission and drains actual operations/exclusive work |

The internal `recover_live_with_instance` path may acquire an exclusive repair
lease while held. It reuses existing local-state recovery and the verified,
cutoff-aware M3/M4 import rebuild. A failed repair retains a hold; only successful
recovery plus rebuild reopens admission. No unchecked reset is exposed to the
renderer, CLI or autonomous tools.

The lifecycle hold path now drains for at most 40 seconds before stopping the
transport/releasing its instance. A timeout produces a typed attention error,
not a clean-release claim. Dropping the instance stops future leases, while
orphaned work retains the OS file lock until it ends. File pins are released
before logical drain completion is advertised, and the lifecycle checks the
observed lock is no longer held before writing its clean-release report.

Admission state itself is process-local, not a new durable recovery journal.
Process abort/power interruption still relies on the existing immutable import
journal/marker/receipt recovery and canonical/derived safety checks at restart.
The existing startup sequence is preserved: single-instance ownership, import
recovery and local approval reconciliation precede transport and IPC startup.
Dropping/recreating an in-process gate is not an authorized repair operation.

## Verification

Executed on the development Windows host with locked, offline Rust 1.98.1:

| Check | Result |
|---|---|
| Daemon library excluding the known host owned-HTTP compatibility group | 566 passed, 0 failed, 4 ignored, 8 filtered; 74.34 s |
| Daemon binary | 15 passed, 0 failed |
| Runtime coordination cases included above | 5 passed |
| Publication cases included above | 13 passed |
| IPC cases included above | 66 passed |
| Strict daemon all-target Clippy (`-D warnings`) | Passed |
| Workspace rustfmt / diff whitespace check | Passed |

Counts overlap; do not add them. The eight explicit exclusions are
`owned_process::tests::http::`; their known host TCP discrepancy is not fixed
or hidden. This is not a full-suite PASS. Original assertions in that group
were unchanged. The broad-run log is outside Git at
`%LOCALAPPDATA%\Cryptex-Dev\m8-runtime-admission-20261108\daemon-lib.log`.
An additional clean-rejection/reopening assertion was subsequently added to
the live recovery case; that case was rerun successfully, followed by strict
Clippy/fmt. Platform cfg scoping below does not change Windows behavior.

New checks cover:

- pending exclusive work blocks newcomers, drains every lease clone, refuses
  competing/foreign-runtime admission, times out without mutation, and retains
  holds on failed/abandoned repair, panic and poison;
- actual instance release remains blocked by an orphaned active lease;
- real IPC refuses all supported content/mutation/enrollment operations while
  held, but health and transport status respond; background catch-up also refuses;
- a real large event-detail pipe response is left under OS backpressure after
  one response byte arrives: exclusive admission remains blocked until the
  actual write completes;
- a timed-out bounded worker retains its runtime lease until it actually exits;
- an owner-review session permits an intervening exclusive operation during
  think-time and refuses confirmation while that operation is active;
- a genuine signed internal import and real IPC endpoint share the same actual
  `InstanceLock`: after canonical publication, while cleanup/rebuild is paused,
  IPC returns `runtime_busy` with no hits; after successful refresh, retrieval
  returns to its verified normal path;
- committed refresh failure and recovery-evidence failure latch the proper
  live holds; retries cannot clear them, failed repairs stay held, successful
  verified repair/rebuild reopens, and canonical/incoming bytes remain unchanged;
- the existing process-interruption/cleanup/immutable replay regression groups
  still pass through the coordinated wrapper.

A metadata-only `rustc` check of the coordinator's production reader/shutdown
path for `aarch64-apple-darwin` passed with warnings denied. Dormant writer/repair
APIs remain `cfg(any(windows, test))`: Mac production publication is still gated,
while common primitive tests compile that code on both targets. No warning
allowance or security setting was added. This narrow cross-target metadata
check is **not** a full Mac build, native execution, lint receipt or publication
parity result. Those native checks remain pending for this new source.

## Review status and next boundary

The separately authorized reviewer inspected the in-progress coordination diff
and reported no confirmed admission bypass/lock inversion in that source pass.
Its shutdown observation concern was corrected with bounded draining, file-pin
ordering and the observed-lock check. However, reviewer test execution was
blocked before running by Windows Application Control (OS 4551), and its later
agent turn ended with an automatic possible-cybersecurity-content rejection.
No protection was disabled or modified, and no unchanged blocked binary was
repeatedly retried. **There is no completed independent verdict for the final
commit.** Root-authored executions above are not attributed to the reviewer.

Next: complete review of this exact coordinator, then connect only a gated
managed-controller caller using its existing instance and the shared barrier.
Before activation, also address peer-aware long-lived derived refresh and the
self-device tail/allocator scope, ordinary end-to-end offline/restriction and
failure/restart coverage, native Mac publication/ancestor/endpoint parity,
supported-filesystem physical durability, the host compatibility case, required
CI and human review. Earlier Mac/VM receipts retain their original source scope.
No new owner VM exercise is needed merely to reconfirm earlier checkpoints.
