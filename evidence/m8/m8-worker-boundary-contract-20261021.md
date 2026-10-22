# M8 Windows managed transport worker boundary

Date: 2026-10-21. Explicit daemon-backend worker, not default startup activation.

`ManagedTransportWorker::spawn_existing` accepts backend-selected existing
profile/package paths, environment and fixed loopback ports. It spawns one
named blocking thread, which opens the pinned package and exclusive private
profile, uses Windows Credential Manager, validates ready configuration and
runs the existing owned controller. The thread owns all resources through
native shutdown/unwind; no native process handle crosses to the renderer.

## Reopen-only boundary

`ManagedProfile::reopen_native` has a distinct read/validate path. It requires
ready state, initialization intent, both generation markers, completed native
receipt, matching ports/schema, protected existing home, unchanged certificate,
key/config and existing bound OS credential. It never calls provisioning or
the generation/staging branch, even if a receipt disappears between reads.
Missing/corrupt/incomplete state refuses without repair or key replacement.
Profile ownership locks may be opened by the existing profile adapter.

No profile discovery, automatic creation, transport peer/folder configuration,
security-event writer or trust activation is added. Explicit backend selection
is still required; daemon command/startup and IPC do not call this API yet.

## Cancellation, ownership and joining

- Cancellation is a monotonic atomic latch; subsequent status cannot clear it.
- The existing controller checks the latch before launch, after bounded health
  observations and during backoff. Startup/REST/shutdown remain blocking and
  use their existing deadlines; cancellation is not instantaneous.
- `shutdown` accepts a nonzero budget up to 45 seconds, requests stop and waits
  for thread completion using monotonic time. It joins only after
  `JoinHandle::is_finished`. Invalid budgets refuse before cancellation.
- A deadline error retains the thread handle for a later wait. It does not
  signal a released profile or clean shutdown. Status-lock failure still
  requests cancellation and attempts the join.
- Drop requests cancellation and waits at most three seconds. If unfinished,
  the detached stopping thread continues owning profile/package/job resources.
  It emits a typed unresolved diagnostic; no replacement is started. This is
  not evidence of immediate process cleanup or graceful daemon exit.
- A daemon owner must explicitly await successful shutdown before claiming a
  clean runtime shutdown/releasing its lifecycle ownership. This call-site
  integration is pending. Whole-daemon abrupt death still relies on the owned
  job's existing kill-on-close behavior, not a new recovery mechanism here.
- Unwind is caught as `WorkerError::Panicked` after task-owned resources unwind.
  Panic payloads are not copied into worker status; Rust's existing panic hook
  is unchanged. No global hook or secret-bearing error fields are introduced.

## Bounded in-process status

One mutex-protected fixed-size snapshot carries Opening/Active/Stopping/
Finished/Failed, the stop latch, latest local controller status, outcome and
typed error. There is no unbounded queue, persistent status file or polling of
untrusted JSON. Poisoned status returns `StatusUnavailable`; it cannot convert
an error to success. A terminal worker does not prove native cleanup when the
native result reported unresolved cleanup; existing exclusion pins apply.

LocalReady remains local owned-engine readiness only. No field claims trusted
peers, content convergence or protected replication. Snapshot is an internal
diagnostic and is not an authorization input or authenticated IPC endpoint.

## Remaining scope

Authenticated read-only status IPC, owner lifecycle hookup and explicit
configuration selection, daemon-death native coverage, later macOS adapter,
and ADR-0019 exact trust-protocol/security review remain open. M7 write-back
stays disabled. No Actions/CI or milestone-complete claim.
