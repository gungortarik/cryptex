# M8 owned-process lifecycle — implementation boundary

**Date:** 2026-10-20  
**Scope:** Windows process-ownership foundation for the managed transport.  
**Authority:** M8 authorization, ADR-0004 backend-owned supervision and ADR-0019
activation gates. This is not a trust protocol or a complete transport controller.

## Role and launch boundary

`owned_process::LaunchSpec::syncthing` prepares the fixed native role: explicit
absolute executable and profile paths, `serve --home`, no browser, no native
restart, no automatic upgrade, no port probing and a profile-local native log.
It does not search PATH or invoke a shell. It creates no profile or process.
It rejects relative/traversal paths and repository working directories.

The environment starts from a small system-variable allowlist, not ambient
Syncthing, proxy, loader, PATH or Rust overrides. Add only the selected profile
and no-default-folder settings. Arguments/environment are bounded and Windows
UTF-16 argument quoting preserves spaces, quotes, trailing backslashes, Unicode
and empty arguments. Embedded NUL, duplicate environment names and non-EXE
Windows shell roles are rejected before execution.

The future controller MUST separately verify package/version/provenance,
profile contents/permissions, API secret state, endpoint ownership and user
authorization before spawning. This helper is not proof of those properties.
It is not wired to daemon startup, Desktop/CLI IPC, transport pairing or content
admission. Preparing a native role cannot install peer trust.

## Windows ownership and failure model

Use a fresh unnamed Windows job with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
CreateProcess receives `PROC_THREAD_ATTRIBUTE_JOB_LIST`, so job assignment
happens as part of process creation, before child code runs. Do not spawn first
and attach afterward; do not fall back to an unowned process if creation fails.
This avoids the suspended-but-unowned crash gap.

The job handle is not inheritable. The extended handle list permits only a
valid NUL stdio handle to be inherited. No parent credentials, job handles,
console handles or arbitrary inheritable handles are passed. All temporary
thread, attribute and stdio resources have explicit RAII ownership.

All process observation and termination use retained OS handles; no adoption
API, process-name matching or arbitrary PID kill is provided. `try_exit` is an
owned-TREE completion observation: an exited root with live descendants is
still incomplete. `None` must not be interpreted as healthy replication or even
proof that the root is still running. A future health controller must separately
distinguish root death from a live owned tree.

Waiting has a monotonic deadline; timeout itself does not kill the tree or
claim cancellation. Forced cleanup targets only this object's job, remains
explicit in the exit observation and is never relabeled clean even after a
racing zero root exit. A clean observation requires root exit 0, no force and
zero active processes in the job. Root exit code 259 is handled using the
signaled process handle, not mistaken for the `STILL_ACTIVE` numeric sentinel.

Dropping/losing the owner closes its job handle; the OS terminates remaining
owned members. Tests separately exercise normal Drop, abrupt `exit(97)` without
Rust destructors, and external termination of the owning process alone while a
nested child job exists. An unrelated owned job stays alive during cleanup.
These are process-death checks, not power-loss or arbitrary hostile-child proofs.

## Platform boundary

Win32 handles/job APIs are confined to `owned_process/windows.rs` with the
existing exact windows-sys 0.61.2 pin plus its JobObjects feature. There is no
dependency version upgrade. Other platforms currently refuse spawn with
`UnsupportedPlatform`; no unowned fallback is provided. Implement and verify the
Mac adapter in the later consolidated Mac stage before M8 completion. Existing
Mac behavior is not silently replaced by a claimed managed supervisor.

## Native synthetic proof

The explicit native integration test is excluded from ordinary runs until an
isolated fixture is supplied, then selected separately with `--ignored --exact`.
It verifies the retained unmodified Windows Syncthing 2.1.5 binary's exact
SHA-256, not merely its filename. Its provenance is the previously recorded M0
native package, not a new publisher-signature or distribution acceptance claim.

`scripts/m8/prepare-owned-transport-fixture.ps1` is a Windows TEST PREPARER, not
a product profile provisioner. It refuses existing/non-app-data fixture roots,
protects only its newly created directory with a current-user-only inherited
DACL before generating native keys, and configures the generated synthetic
profile through XML with DTD/external resolution disabled. All native API and
transport listeners are loopback-only; discovery, relay, NAT, automatic upgrade
and usage reporting are off; no folder is shared. It creates no user-global
profile and does not launch a server.

The Rust test launches the real native process through the new owned job,
checks expected native version/identity via the bounded REST adapter, confirms
wrong-key rejection, inspects live native isolation/folder settings and requests
shutdown through a test-only REST call. It verifies clean job completion, no
native database-stop failure and absence of API keys in generation/native logs.
Repeat using the retained synthetic profile to exercise same-version restart.

The fixture uses FakeSecureStore for its synthetic API key. This is NOT product
OS-secret provisioning, profile bootstrap or a production authenticated shutdown
capability. No canonical memory, trusted peer, internet transport or personal
profile participates. Retain private fixture artifacts for review.

## Still required

Product package validation/notices/provenance, app-owned protected profile
provisioning, OS secret storage, endpoint/process binding, authenticated lifecycle
control and daemon integration remain mandatory before the full controller can
be reported implemented. Pairing, durable security events, issuer authority,
revocation and incoming canonical admission remain separately gated. Do not
change the current diagnostic to Active/Verified merely because these fixtures
pass. M7 write-back stays disabled; M8 remains open; Actions stay off.

## Primary API references

- [Windows process attributes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute)
  documents the job and explicit inherited-handle lists.
- [Windows job objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
  documents descendant membership and last-job-handle-close termination.
- Existing ADR-0004 and retained M0 native evidence establish the scoped pinned
  process/REST direction; they do not establish the remaining product gates.
