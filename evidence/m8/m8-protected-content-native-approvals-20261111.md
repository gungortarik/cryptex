# M8 protected Tauri content and native approval candidate

Source: `efdb877719cc64f34c612d92f6bfce364a167f62`.
Scope: ADR-0020 default-off local-certificate experiment. Product activation and
M8 completion remain open.

## Implemented boundary

The actual signed Desktop installs a Tauri Assets provider from the fully
verified retained inventory snapshot. It drops the embedded provider, clears
the development URL and uses only listed frontend bytes. Tauri's index routing
can still select the verified index. The candidate's identifier, app directories
and incognito WebView storage are isolated from ordinary product data.

After installing that Context, Desktop and daemon independently bind the same
content catalog to their existing authenticated native connection. Only the
private per-message receiver can mint a consumed-once request proof. Wire
capabilities stay in the native owner thread; the renderer receives opaque
local handles. The three registered owner-security commands prepare, confirm
and cancel through the existing approval domain and journal. Other product UI
commands are not enabled by the candidate builder.

Approval sessions retain their original root/store/admission context. Actual
reply sends retain admission; owner think-time does not. Native process or
endpoint loss, malformed/replayed followups and abandoned prepared responses
burn ephemeral grants. They do not invent durable cancellation or roll back an
uncertain append. A worker-held delivery lease observes cancellation both
during work and after send but before asynchronous consumption. Normal receipt
is a distinct completed state; it does not invalidate the successful review.

Only a fresh, owned, marked TEMP runtime in this explicitly authenticated mode
selects a process-local FakeSecureStore and synthetic seed. It omits ordinary
IPC. No ordinary runtime changes store merely because the feature is enabled.
The actual daemon is the sole canonical writer in this fixture.

## Verification and limits

The [exact-source CI run](https://github.com/gungortarik/cryptex/actions/runs/43095500335)
ended in `startup_failure` on initial dispatch and after requeue. At 20:47 UTC
on November 11 the final response listed zero jobs and zero check runs. Its
annotation reports an unexpected GitHub error, request ID
`23DB:165871:D52F90:29E2356:6AF4D39C`. At 20:48 UTC the
[GitHub incident](https://www.githubstatus.com/incidents/dvy0t3mvh1lg)
reported degraded Actions availability. The workflow is active and matches
the workflow from passing run `43090075267`. The old run failed again when
requeued during service recovery. A [new dispatch](https://github.com/gungortarik/cryptex/actions/runs/43101602427)
at the same full SHA completed successfully. Both Desktop startup matrix jobs
passed. The Mac log includes all four content/session markers, all eight
negative startup cases and disposable signing fixture cleanup. Platform counts
include Mac/Windows startup 11/10, Desktop 39/46, approval 17/17, IPC 71/80
and owner channel 22/14. Four existing Mac IPC tests were ignored.

The [interactive fixture build](https://github.com/gungortarik/cryptex/actions/runs/43104045850)
exported the tested arm64 bundle at the same product SHA. Its disposable signing
certificate expires on 2026-11-12 at 21:34:20 UTC. The delivered ZIP has SHA-256
`e33e15b054e060137f64bd884eaa4c0e6c6975cf53e28151c7931a84daa684cc`.
Only the external result collector changed after export. Every signed bundle
member matched the downloaded artifact byte for byte. On this revision the
interactive entry calls Tauri's `run`. The macOS Tao loop exits the process
before control can return to `owner.close()` on ordinary quit. The collector
reports the journal independently when the shutdown receipt is absent and
leaves event-prefix verification incomplete in that case.

The owner returned `MAC-ARAYUZ-SONUC-272703d4.zip` (SHA-256
`5e556a53140576014a89d0f914140942c83bc09cbfe05e3dde167a1dcc86c4b2`).
The single `result.json` identifies source `efdb877719cc64f34c612d92f6bfce364a167f62`.
The owner reported Active after relaunch. The journal check returned true for
one cancellation with zero append attempts and one anchored confirmation with
one attempt. Desktop exited with code 0. The outcome was
`journal_verified_shutdown_receipt_missing` and
`two_events_original_prefix_preserved` was null.

The actual signed headless fixture requires all four result markers for actual
Context/provider installation, synthetic prepare/cancel/confirm, lost native
session/old local-handle refusal and failed delivery cleanup. It verifies the
initial canonical event prefix, exactly one additional confirmed event, and
the entire journal state multiset with exactly one anchored append attempt.

| Synthetic runtime | Exact journal outcome | Append attempts |
| --- | --- | --- |
| Normal cancel, bad digest, confirm | cancelled + owner_pending + anchored | 0 + 0 + 1 |
| Lost connection | owner_pending | 0 |
| Missing snapshot / failed handle allocation | owner_pending per runtime | 0 |
| Lost receiver / in-flight or late cancellation | owner_pending per runtime | 0 |
| Successful receipt then actual cancel | cancelled | 0 |
| Already-expired queue | no journal created | none |

Fault fixtures include missing accepted snapshot, failed local-handle
allocation, already-expired queued work, lost reply receiver, cancellation
during an in-flight prepare and cancellation after worker delivery. A separate
gated successful receipt must still permit an accepted real cancellation.
Native subprocess tests cover concurrent process/endpoint loss against a long
receive budget. Their delayed loss is scheduling-based, not a deterministic
observation of entry into the kernel receive syscall.

Independent source review through `3da1ac9` closed the actor cancellation,
cleanup and WebView isolation findings. The test-runner correction in
`c603bfd` separately passed source review. Neither review executed binaries.
An earlier native run passed all four actual content/session markers but the
runner failed an ordering-dependent journal assertion; that run is not passing
acceptance. The correction checks exact complete outcomes regardless of the
journal's random transaction-ID ordering, without changing domain behavior.

Later exact-source Mac runs hit Deadline before/inside the content check.
Fixed-stage diagnostics in `df1d39b` identified native_accept at 2638 ms: the
native 2.5-second HELLO window was starting while the child still verified its
full independent snapshot. `4794f04` separates that preparation using an exact,
public stdout progress hint followed by EOF, read under the unchanged outer
startup deadline. The child restores stdout to a writable discard sink before
lifecycle output. The hint is never authentication, readiness or approval.
Existing actual wrong-image fixtures must still refuse after that hint.
The native 2.5-second and outer 10/15-second caps have not been increased.
An actual unsigned cat subprocess regression covers exact/spoofable, malformed,
partial, extra, EOF, absent and already-expired hints; its RAII cleanup in
`efdb877` reaps children even when an assertion fails. Independent source review
of that exact source found no actionable issue. The retained Release also
caches its immutable catalog once, without sharing caches across fresh release
observations or removing independent parent/child verification.

Local Windows checks passed all-target lint, formatting, 46 Desktop tests and
9 startup tests with one explicit disposable-account ignore. Four Python
runner regressions passed. A later local daemon test executable was refused by
Windows Application Control (4551), before execution; security settings were
not changed and that attempted run is not reported as a pass.

The [scoped CI run for the exit fix](https://github.com/gungortarik/cryptex/actions/runs/43147508139)
passed on macOS and Windows at `a7b41a22f7d178030c8ad8f1fdf86d1ef9550007`.
The Mac driver launched the signed interactive binary and requested normal
AppKit termination of that PID. It exited successfully and the daemon reported
one event with the original prefix preserved (`native_ui_quit_preserved_seed=passed`).
The exported arm64 archive SHA-256 is
`17cca4f1dde137d0974cad67a353b1781b49913cae306272750ac53ed3539c3d`.
Its fixture signing certificate expires 2026-11-13 03:22:54 UTC.

The revised owner result `MAC-ARAYUZ-SONUC-bf755756.zip` has SHA-256
`72016f4f5638d67d18754b7159e5044f3d1dc69980fb42135374d6958e92f385`.
Its single `result.json` identifies source
`a7b41a22f7d178030c8ad8f1fdf86d1ef9550007`, exit code 0 and
`outcome=expected_local_events`. Both outcome checks are true: exactly two
verified events with the original prefix preserved, and the exact journal
multiset of one cancelled operation with zero append attempts and one anchored
operation with one attempt. This closes the owner interactive cancellation,
confirmation and normal-quit receipt check for this isolated fixture.

Open gates: same-profile durable restart with the intended platform secure store, product
publisher/OS-floor decisions and explicit activation. The fresh-runtime lost
handle test does not establish same-profile restart or old wire-capability
replay across new daemons. This is selected candidate evidence, not a full
workspace, production distribution or M8 acceptance claim.
