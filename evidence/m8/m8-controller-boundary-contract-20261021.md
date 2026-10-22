# M8 Windows isolated transport controller boundary

Date: 2026-10-21. Implemented backend scope; not milestone completion.

`PreparedNativeProfile::supervise` runs on a dedicated blocking backend worker.
It wraps the existing prepared profile, pinned package, OS credentials, owned
job and same-socket REST verification. It does not provision automatically or
register a production daemon startup hook/IPC command. No peer/folder/trust
configuration, remote admission or canonical security writer is enabled.

## Lifecycle

- Before every start, existing native code revalidates config, identity, OS
  secret and idle/exclusion state. Start failures are terminal, not retryable
  initialization or permission to replace keys/profile files.
- A returned native session has passed owned local API readiness. The controller
  polls whole-job exit, then authenticated identity/version health, then the
  trusted backend cancellation callback. Poll interval is 250ms plus operation
  duration. Existing bounded native readiness/REST/shutdown budgets apply.
- Only observed full owned-job exit permits replacement. Session destruction
  precedes replacement/backoff. No external PID is adopted. Native unresolved
  cleanup retains pins and poisons the prepared profile; the next start refuses.
- Maximum four starts per invocation (one initial, three replacements).
  Backoffs are 1/2/4 seconds in 100ms cancellation slices; callback/operation
  time can extend wall duration. Healthy observations do not reset the budget.
- Health, ownership, credential, start and shutdown errors are terminal. A
  crash racing with health may therefore fail terminally rather than restart;
  safety is preferred to guessing that an authentication error was a crash.
- Stop while live requests authenticated native shutdown and requires owned
  tree exit plus existing log checks. Stop before start or during backoff means
  `Cancelled`, never `CleanShutdown`. Unrequested native exit, even code zero,
  is not an owner-requested clean shutdown.
- Session Drop uses existing bounded owned cleanup on errors/unwind. Forced
  cleanup is never represented as graceful. The backend callback must return
  promptly; this is not an untrusted renderer/plugin extension point.

## Status

`Starting`, `LocalReady`, `RestartWaiting`, `Stopped`, `Failed` contain only
attempt count, current advertised PID and typed error. No path, credential,
memory body, raw native response or transport identity is exposed. `LocalReady`
is local engine health only, not trusted-peer connectivity, canonical admission
or protected replication. `Failed` with no advertised PID is not evidence that
all OS cleanup succeeded; unresolved native cleanup separately retains locks
and emits the existing structured diagnostic.

Status callbacks precede launches, follow authenticated health and run during
backoff; one terminal callback follows session scope release. Its return value
cannot override a terminal error. No durable/UI status file is created here.

## Verification boundary

Eight deterministic driver tests cover cancellation, bounded replacement,
release-before-restart, clean stop, terminal start/health/shutdown failures and
restart exhaustion. The real Windows native provisioning fixture additionally
runs two controller start/clean-stop cycles on the same protected profile.
Those native cycles prove adapter/start/health/stop integration, not actual
crash-driven replacement, long-running health or production startup integration.

Mac adapter, real crash/restart matrix, daemon startup/worker cancellation,
shutdown ownership and authenticated status IPC integration remain open.
ADR-0019 protocol/security review still gates trust activation. M7 write-back
remains off; Actions remain unauthorized.
