# M8 managed-worker incoming bridge and completed source review — 2026-11-08

## Reviewed source and activation boundary

Implementation: `67c3f8bca2e32e73e25958bde517285a7cb94bc2`.
Final code/test revision: `141ff83cdbc0aee8215c742b42f278decd31ccbc`.
The second commit corrects a test's staging-parent expectation after adding a
genuine unapproved-input case; it does not change production behavior.

The owner requested completion of the technical review and connection to the
transport controller. A bounded daemon-side `IncomingController` is now called
through `ManagedTransportWorker::process_incoming`, using its actual status
reader and the caller's existing mutable `InstanceLock`.

**This is a dormant internal connection, not automatic publication activation.**
There is still no normal startup, watcher, CLI or renderer hint feeder/caller.
The native Windows/Mac publication, filesystem/durability and end-to-end gates
are not waived by this source review or by synthetic transport status in tests.
No owner runtime, credential, VM or native transport process was accessed.

## Controller behavior

- One pending batch, at most the existing 16-file import bound. Hints must be
  bounded two-component relative `.seg` paths; absolute/parent-traversal paths
  are refused. Exact queued duplicates are idempotent; another batch is busy.
  Existing staging still checks exact names, identities, per-file and total
  byte budgets, signatures, chains, policy and no-replace destinations.
- No canonical inspection/publication occurs for an idle, stopped, starting
  or unavailable worker. The worker must report authenticated local readiness.
  Readiness alone never grants peer trust or says replication is protected.
- The controller checks the matching runtime instance before filesystem work.
  Under an ordinary admission lease it reconstructs the canonical local approval
  roster and compares its local transport association with the worker identity.
  It rechecks worker readiness and releases ordinary admission **before** calling
  the existing exclusive publisher. No ordinary/exclusive nesting is introduced.
- The publisher revalidates complete canonical, local-trust, cutoff and M3/M4
  safety under its own retained locks. A status snapshot is not a policy proof;
  a readiness change cannot authorize unapproved data.
- Busy admission defers without staging. Held/ambiguous publication retains
  hints and the existing runtime hold. Only existing verified recovery/rebuild
  can reopen the runtime. Success consumes the batch; a later explicit identical
  hint is an immutable replay, not a duplicate append.
- Binding refusal and clean untrusted-input rejection consume only the in-memory
  hint batch. Incoming bytes stay untouched for inspection; an explicit new
  backend hint is needed to retry. Invalid large batches are not repeatedly
  copied/verified on every transport heartbeat.

## Prerequisites completed with the bridge

**Peer-aware derived refresh:** background IPC catch-up previously rebuilt only
the local device's M4 view. It now uses the existing complete local-anchor,
cutoff-aware M3/M4 rebuild, so a later refresh does not erase imported peers from
the verified union. This retains that helper's current v1-content scope: v2 or
unsupported retrieval semantics are not newly enabled, accepted writers and
canonical formats are not widened, and verification/rebuild failure is explicit.

**Local allocator boundary:** equality-pruned, already-canonical local segments
remain idempotent. Any remaining new segment for this installation's own
DeviceId is rejected before manifest preparation. Ordinary replication cannot
pretend to restore a local stream without reconciling its sequence allocator.
The regression creates a genuinely signed contiguous tail in a copied synthetic
runtime and verifies receiver canonical bytes and allocator remain unchanged.
This is refusal of broader restore scope, not implementation of device restore.

## Completed separate technical source review

The authorized independent agent completed a read-only review of the earlier
`a5429ea` coordination/lifecycle implementation. It found no confirmed product
coordination defect, but found a **P2 test-coverage defect**: two lease-lifetime
probes used already-expired exclusive deadlines. Those probes would return busy
even if no operation lease existed. Both now use future 40 ms deadlines while
the worker/actual pipe write remains deterministically blocked; a missing lease
would instead allow exclusive admission and fail the tests.

The reviewer closed that finding by inspecting the correction and reviewed the
new controller, queue/error lifecycle, local-tail refusal and peer refresh. It
bound the reviewed production blobs byte-for-byte to final revision `141ff83`;
the unchanged admission/instance/main/lib blobs retain their `a5429ea` source
binding. It also inspected the final unapproved fixture and staging assertion.
Final incoming test-file SHA-256:
`761D78360D82E24AC76A32CB6535BF1CD83ACAA668B1DF9C451ECFD8B7DAFE4D`.

**Bounded source verdict: no remaining confirmed correctness findings at
`141ff83cdbc0aee8215c742b42f278decd31ccbc`.** No reviewer executable was retried,
security setting changed or tracked file edited by the reviewer. Executed tests
below are author results. This is separate-agent technical source review, not
an external human audit, certification, native verification, activation approval
or M8 completion. Required human review remains open.

## Executed verification

Locked offline Rust 1.98.1 on the development Windows host:

- Final frozen-source daemon library run: **571 passed, 0 failed, 4 ignored,
  8 explicitly filtered owned-HTTP tests**, 81.12 s. Log: `daemon-lib-final.log`.
- Complete IPC group: **66 passed**, including both corrected future-deadline
  ownership probes. The stronger actual instance-shared import/IPC case also
  verifies the imported note remains present after a full background refresh.
- Controller/worker bridge group: **4 passed**. Actual product bridge methods
  run with explicitly synthetic worker observations, genuinely signed paired
  canonical fixtures and fake secure storage. This is not native socket/process
  attestation. Cases cover readiness, stop, busy/defer, foreign runtime/adapter,
  unapproved input, queue limits, rejected-input retry suppression, holds,
  verified repair, publication, replay and empty-queue idle behavior.
- New local replay/restore-tail case: **passed**, including unchanged allocator
  and canonical bytes after rejection.
- Daemon binary: **15 passed**. Strict all-target daemon Clippy, fmt and diff
  whitespace checks passed.

These are overlapping selections, not additive counts. The known host
`owned_process::tests::http::` group remains explicitly excluded from the broad
library run; its earlier VM pass does not diagnose/fix the host case. Native
Mac results from earlier commits do not cover this change.

During validation, relinking once collided with a still-running broad test
executable (LNK1104); later sequential linking resolved it. One intermediate
test binary was blocked by Application Control (4551), without execution or
protection changes. After the genuine test-fixture cleanup, the ordinary build
and all four controller cases ran. An initial added assertion incorrectly
expected the staging parent not to exist after an earlier rejected import;
the final assertion proves the existing parent remains empty before/after stop.
No product check or failing security assertion was weakened.

Logs are outside Git under
`%LOCALAPPDATA%\Cryptex-Dev\m8-controller-bridge-20261108`.

## Continuation

The in-process coordination review and dormant controller connection are now
implemented. Next activation work must establish native publication/transport
behavior on the exact candidate, supported-filesystem durability evidence and
ordinary cross-device offline/restriction/failure/restart coverage. Mac
ancestor-handle/no-replace/owned-endpoint parity, the host compatibility case,
required CI and human review remain open. A live feeder must keep backend path
selection bounded, preserve this instance/admission ownership and use verified
canonical policy rather than transport completion hints. It must not enable
broader local-device restore implicitly. M8 remains incomplete.
