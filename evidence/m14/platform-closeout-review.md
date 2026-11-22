# M14 platform acceptance review

Review target [`18b9b04`](https://github.com/gungortarik/cryptex/compare/d4b621b...18b9b04). The product sources are unchanged from the Windows console observation build. The last candidate change adds a signed public note to the installed-updater rehearsal.

On November 22, 2026, the candidate entered final review for the disposable-account scope. M14 remains open. Resolve or disposition the findings below against the accepted ADR and obtain human diff review before closure.

| M14 requirement | Evidence and limits |
| --- | --- |
| Per-user login and removal | The [Mac login receipt](macos-interactive-login.json) reports one launch in a fresh session over 60 seconds and removal of the startup entry. The [Windows VM receipt](windows-interactive-login-9a5035d-v2.json) reports both calibration controls, an observer armed 5.28 seconds before launch, zero console shows or console process-start events through ten seconds after readiness, and removal of the startup entry and observer. The [Windows session count](../../docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md#m14-login-startup) was independently read in the VM. |
| Tray, window and background | The native window rehearsal minimizes, restores, hides and reopens the window. The installed-package smoke checks the Desktop and daemon process identities through close-to-background and restore. Earlier physical menu and tray Open/Quit evidence is indexed in [ADR-0012](../../docs/adr/ADR-0012-background-runtime-implementation-proposal.md). |
| Scaling, appearance and keyboard | The native WebView exercises zoom. The built renderer exercises navigation, focus and search in two appearances at three densities. These are automated viewport and keyboard checks. Hardware monitor changes and assistive-technology sessions were outside these runs. |
| Installation and package recovery | The ordinary release smoke installs NSIS on Windows and an app bundle on macOS. In a separate disposable-account rehearsal, the local updater uses three instrumented packages to exercise a compatible handover, explicit protocol rejection, and recovery after the updater process is killed. The signed-note check verifies the signature and compares the canonical pack set and bytes before and after those transitions, including the interrupted selection. |

## Native results

The candidate's [full CI](https://github.com/gungortarik/cryptex/actions/runs/45044676425) passed Windows and macOS readiness and the Windows cross-account IPC check. The [separate native startup suite](https://github.com/gungortarik/cryptex/actions/runs/45045335417) passed on both platforms, including the Mac restricted-peer check that failed on the earlier run. Only the installed-updater test script changed between those runs. The retry leaves the cause of the first failure open. Fourteen local Windows M14 harness tests also passed.

The [installed update and recovery run](https://github.com/gungortarik/cryptex/actions/runs/45044444538) passed on Windows and macOS. On each platform the `updated`, `rolled_back` and `recovered` receipts report a verified canonical note and a reconnected Desktop on protocol 2. The interrupted transaction refused a second updater while the surviving probe held its session lock. The [ordinary package smoke](https://github.com/gungortarik/cryptex/actions/runs/45044444522) and [login registration](https://github.com/gungortarik/cryptex/actions/runs/45044444529) also passed on the candidate.

The [native shell and display run](https://github.com/gungortarik/cryptex/actions/runs/45044188743) used the same product, workflow and UI-test sources at `d8b7326`. Both platforms passed all six density/appearance cases. The rendered screenshots show the focused search control and submit button within the visible width, including at 200%.

## Security and recovery review

- The [first Mac startup regression](https://github.com/gungortarik/cryptex/actions/runs/45044243171/job/137275224639) returned `existing_profile_settings_restricted_peer`. After a confirmed device restriction, the settings read returned `Attention / Start`, one device and the saved revision. The test requires `Paused` and an empty device list. This is evidence of an incorrect settings response. The test did not observe an unauthorized transfer. The same symptom is recorded in the earlier [window restoration handoff](../../docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md#m14-window-restoration). The follow-up correction and its verification are recorded below.
- The original candidate allowed the filesystem start request and the environment to select the service executable. The [bundled service selection check](#bundled-service-selection) below covers the correction required by [ADR-0012](../../docs/adr/ADR-0012-background-runtime-implementation-proposal.md#mandatory-production-gates-retained).
- The [local updater](../../apps/updater/src/main.rs) reports a 30-second probe timeout and asks for Desktop to quit. It does not terminate or reap the timed-out probe on that branch. A probe that remains alive with its session lock can delay recovery. The interruption rehearsal releases its held probe before retrying. It establishes recovery after that exit. Automatic recovery from an indefinitely hung probe is unverified.
- A process kill after package selection exercises the on-disk journal and the next invocation. It does not model a filesystem losing unflushed changes in a power failure. Signed distribution and notarization belong to M17 under the [M14 contract](../../docs/milestones/M14_PLATFORM_POLISH_UPDATES.md).

## Restricted device list after a transport error

At `81dd2d1` the controller checked the roster generation before asking the worker for its identity. A restriction could invalidate that generation during the identity read. On the resulting transport error, shutdown discarded the generation guard while the response still contained the old device rows. Later settings reads had neither a running worker nor a guard that would trigger verification of the roster.

[`c6e2fc6`](https://github.com/gungortarik/cryptex/commit/c6e2fc6) keeps the roster guard across a clean worker stop, checks it after observing identity and withholds invalidated rows until it has verified the current bindings. An ordinary transport error still reports its cause. A failed rescan cannot republish previously displayed rows.

The original controller retained the row in the local regression. The corrected test forces both orderings of transport failure and generation change. It also checks that a current roster and its original error remain available after an unrelated transport failure. This test and the offline cutoff test passed in the [temporary Windows standard account](https://github.com/gungortarik/cryptex/actions/runs/45048652205/job/137288704619) and the [disposable macOS runner account](https://github.com/gungortarik/cryptex/actions/runs/45048652205/job/137288704665).

The [original Mac native scenario](https://github.com/gungortarik/cryptex/actions/runs/45048652138/job/137288704461) confirmed the signed restriction and read back paused sync, an empty roster and the same settings revision. The [Windows native exchange](https://github.com/gungortarik/cryptex/actions/runs/45048652205/job/137288704464) checked removal of the last peer, refusal of its stale settings and preservation of the restriction after reopening. All profiles used for these checks were disposable.

## Bundled service selection

Desktop derives the daemon path from its own executable directory. The renderer, environment and filesystem start control may name that packaged file. A different file is rejected before a lifecycle command can run. Status checks and supervisor restarts resolve the package path again.

The installed release check supplies an inert file with the daemon's name in a separate directory. On cold launch the environment selection must return `SIDECAR_REQUEST_MISMATCH`, with no daemon PID, readiness marker or held lock. With the packaged daemon running, the filesystem start request must return the same refusal. Hiding and restoring the window must preserve the live daemon PID, and quitting must release its lock.

At `0f52961` the Windows NSIS installation in a temporary standard account and the macOS app in the disposable runner account both passed these checks. [Native release receipts](https://github.com/gungortarik/cryptex/actions/runs/45053165024)
