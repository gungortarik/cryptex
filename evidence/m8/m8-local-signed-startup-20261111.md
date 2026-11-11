# M8 local signed real-binary startup

Implementation source: `3d0b6c244396efb708f4c2a29fbb12033b4d5296`.
Scope: ADR-0020 default-off, headless startup experiment.
M8 and product activation remain open.

## Implemented boundary

Both actual executables are built with `m8-local-startup-check` and an independently
selected certificate DER embedded before signing. The CI identity is disposable,
isolated in a temporary keychain and never added to OS trust settings.
The external CMS inventory is generated after both role images are signed.
Its release, architecture, paths, content hashes and role CodeDirectory hashes
are checked against the compiled pin and policy.

The real Desktop entry uses the shared lifecycle-hold command constructor to
launch the real daemon. The feature-only child hook validates its actual parent
before any ordinary runtime initialization. Every startup frame remains bound
to native kernel-attributed identity, the pinned process instance, role, epoch
and the same inventory digest. A separate authenticated runtime-ready frame
follows ordinary lifecycle initialization and ready-file publication.
Readiness alone never completes this check.

The Desktop retains the exact child, verified snapshot and channel in one
cleanup owner. It forcibly ends that first daemon, launches a fresh instance
and refuses the old retained channel before clean shutdown. Native handshake,
data and readiness waits retain their caller's absolute deadline. Synchronous
platform identity/filesystem calls cannot be forcibly preempted; the adapter
rechecks elapsed time before sending or publishing.

## Verification

[Exact-source scoped CI](https://github.com/gungortarik/cryptex/actions/runs/43067144663)
passed on both Mac and Windows.

The actual signed pair passed startup, forced child exit/restart, old live
channel refusal, authenticated runtime-ready and clean shutdown. All eight
negative fixtures below reported their expected refusal stage. The feature-on
library suite passed, including exact named assertions for the startup-frame
test and closed product factory/route. The runner reported both native exchange
and complete fixture cleanup success, without OS trust writes.

Scoped regression totals:

| Suite | macOS | Windows |
| --- | --- | --- |
| Actual child startup/restart | 11 | 10 |
| Desktop Rust | 39 | 46 |
| Shared owner-channel | 21 + 2 explicitly invoked helpers | 13 |
| IPC | 71, 4 pre-existing ignores | 80 |
| Approval sessions | 17 | 17 |

Native C ownership checks, feature/default all-target lint, formatting and the
selected readiness/admission checks passed. Both Windows actual standard-user
runs confirmed removal of their temporary account/profile. This is the selected
startup scope, not full-workspace or interactive release acceptance.

The real-binary matrix requires a successful signed startup/restart control,
then each negative must exit with the expected fixed stage code, report no
success marker and leave no initialized runtime:

| Fixture | Required refusal |
| --- | --- |
| Changed listed resource | Inventory |
| Signed wrong release | Inventory |
| Signed wrong expected daemon CDHash | Image |
| Daemon signed under wrong role identifier | Image |
| Daemon with no certificate leaf | Image |
| No live rendezvous | Rendezvous |
| Actual Desktop signed under wrong role identifier | Child Image |
| Signed wrong expected parent CDHash | Child Image |

The feature-enabled library suite also requires the startup-frame test and
the unchanged closed product factory/route tests to pass by their exact names.
Native signed subprocess checks exercise unanswered HELLO/ACK budgets and
already-expired connection, together with the prior identity/replay/exit cases.
Metadata input tests refuse oversize files, hardlinks, symlinks and FIFOs.
All temporary signing material must be removed before the fixture reports
cleanup success.

Local Windows validation passed: startup 9 with one explicit disposable-account
ignore, Desktop 46, shared owner-channel 13, all-target Clippy and formatting.
Three Python runner regressions passed. These local checks do not substitute
for Mac execution.

Independent source review through `3d0b6c2` has no actionable finding. Earlier
findings corrected the reset handshake budget and replaced a misleading
foreign-parent claim with distinct missing-rendezvous and actual parent-image
checks. Run43066179175 passed the native certificate exchange but stopped at
Clippy plugin discovery; it is not passing real-startup evidence. The runner
now resolves the tool and its nested Cargo executable explicitly, preserving
strict lint flags and restoring the environment on failure.

## Limits and continuation

This verifies the tested headless live-process startup binding. The immutable
content snapshot does not grant an atomic executable path or establish bytes
already consumed by Tauri/webview. No owner request, pending approval capability
or domain authorization is constructed by this startup protocol.
Ordinary product builds have no feature or configured image factory, and all
Mac owner-operation gates remain closed.

Next: protected Tauri resource consumption and authenticated owner-request/
approval-session integration, followed by the actual signed interactive product
pair and the remaining reviewed activation gates. Windows transport and Unix
ordinary APIs retain their existing policy.
