# Disposable public Mach bootstrap mechanism: reviewed phase-one evidence

Status: **phase-one mechanism evidence only; no product custody acceptance**.
Owner authorization: bounded replacement design and isolated experiment, keeping
the accepted Unix IPC transport and all Mac owner mutation gates unchanged.
[Design and explicit remaining boundaries](../../docs/proposals/m8-macos-message-bootstrap-experiment.md).

## Reviewed continuation: exact final source passed

[CI42948812911](https://github.com/gungortarik/cryptex/actions/runs/42948812911)
**SUCCESS** on `8816c04a1b0b6444bd1b9b7e80b5ac315c0f971b`, mechanism job
`130287981566`, macOS26.7.2 arm64. Native C SHA256:
`4d364c49ef0a6327b7641dfb8240327988be6d19b8bded249d704e2e6e833074`.
Collector SHA256:
`d0f4f2ed7cf41e6dd7385f3c1bf7e21c0ba0ddb8af3f7a1fb73ee6318ab9507b`.

All **thirteen** native rows pass: the twelve below plus required
`owned_right_cleanup=cleanup_calls_succeeded`, stage cleanup/code0. This means
tracked normal cleanup calls succeeded, not a general proof against all leaks.
The receipt's exact source hash, closed inventory, all13 booleans, native_exit0,
forced signal9 and independent namespace removal were revalidated from the actual
final job JSON. No positive product/certified-bundle/session flag was set.

Independent source review found two experiment defects in the initial source:
unconfirmed kill/wait could claim supervisor completion, and failed Mach-send
cleanup assumed unchanged port names despite possible pseudo-receive. Corrections:

- Require successful exact-child terminal reaping and actual SIGKILL status before
  supervisor63. Kill/wait failures and raced normal exits remain distinct.
- Eight native injected kill/wait paths exercise EPERM, ESRCH/ECHILD, unexpected
  PID, nonterminal status, raced normal exit and EINTR retry. No real PID is used
  by those injection callbacks; the ordinary timeout case separately kills and
  reaps an actual owned child.
- A failed send immediately exits this disposable fixture with typed exit67
  (timeout) or59 (other), allowing process teardown to dispose the entire IPC
  space. The existing Python supervisor then stops its still-owned process group.
  No stale-name deallocation or blind destruction of outgoing borrowed rights.
- Normal right-release errors aggregate into failure; worker/lookup cleanup
  failures propagate too. A separate actual wrong-right cleanup control returns68
  only after exact KERN_INVALID_RIGHT and a successful real receive-right close.
  Setup62 can no longer impersonate that negative observation.

Four required native controls passed: reaper-controls exit0; wrong-right cleanup
exit68; actual full-queue COPY_SEND timeout exit67; genuine send-once right with a
deliberately invalid identical destination/reply header exit59. Each control's
stage is child_exit. Actual setup errors preserve a closed stage/code diagnostic.

Two intermediate CIs (42948364515/57cec11 and42948682904/aa3da56) correctly refused
completion at the original dead-send-once error fixture. No setup diagnostic was
emitted: that fixture's send actually returned success and reached its final
failure return. This is consistent with supported kernel behavior for inactive
send-once destinations. The corrected fixture uses the public validator's invalid
header condition; no send assertion or authentication test was relaxed.
[Mach send/pseudo-receive contract](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/man/mach_msg.html),
[public kernel header validator](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/ipc/ipc_kmsg.c).

Final source-only review found the P2/P3 findings and control ambiguity closed,
with no further actionable issue in this bounded delta. Reviewer executed none;
native evidence comes from the exact CI above. Eleven new Python tests passed on
Mac and Windows; final local complete suite52:47passed/five native-Mac skips.
Existing scoped prerequisite/raw-Mach/XPC regressions also passed in final CI.

## Exact first successful execution

- [CI42947465303](https://github.com/gungortarik/cryptex/actions/runs/42947465303),
  source `6bfa9304c9d95b2ca5f5a68f77cc28cc206e82e2`: **SUCCESS**.
- Mechanism job `130283606819`, disposable `macos-26-arm64` runner.
- Native source SHA256:
  `af56bc9e413e25f22999eba7a900695ea7b61d0aa06e44309c127d893a3b8263`.
- Public C compilation with `-Wall -Wextra -Werror`, availability errors and
  deployment target12 succeeded. This does not establish runtime support on12.
- Ad-hoc CI fixture only: fixed identifier and same-image hash, not a certified
  organization, sealed Cryptex bundle, publisher or notarized distribution.
- The actual JSON was independently parsed from the completed job log: exact
  source/hash match, closed typed case inventory and all acceptance flags checked.

All twelve native observations passed:

| Case | Observed stage / code |
| --- | --- |
| Active-name collision refused | check_in / 1103 |
| Same-image other process with forged claimed PID refused | validate / 52 |
| Actual child authenticates both directions after intruder | validate / 0 |
| Same-sender replay refused | validate / 56 |
| Wrong session refused | validate / 51 |
| Short frame refused | validate / 50 |
| Complex frame carrying a Mach right refused | validate / 50 |
| Same-PID exec generation refused | validate / 53 |
| Queued frame after child exit refused | validate / 54 |
| Fresh child and fresh session authenticated | validate / 0 |
| Supervisor deadline distinct from child receive deadline | supervisor / 63 |
| All nine normally closed names independently absent | independent_lookup / 1102 |

A separate forced-exit fixture actually terminated by SIGKILL (child_signal9).
A fresh helper then confirmed `BOOTSTRAP_UNKNOWN_SERVICE` for its exact name;
`forced_exit_namespace_removed=true`. Normal-close lookup helpers ran while the
parent was still alive. No name was unregistered or taken over after collision.

The public dynamic check_in/lookup path therefore executed on this disposable
host; this is not merely a declaration check or an assumed inherited Mach right.
No secret/token FD, inherited task-registration window, service plist, private
header, deprecated registration API or installation was used.

## Report and regression verification

Eight new Python tests passed on both the native Mac job and local Windows.
Local complete Mac-script suite:49 total,44 passed/five native-Mac skips.
Regressions check actual stage/code consistency, duplicates/unknown/privacy
fields, nonliteral booleans, partial spawn/registration failures, missing cases,
nonzero native exit, bounded-run failure and failed forced-exit cleanup. Partial
typed failures survive collector rejection; they cannot retain completion.
The existing scoped prerequisite/raw-Mach/XPC checks also passed in this CI.

The receipt always records `ci_only=true`,
`identity_policy=fixed_identifier_same_image_adhoc_ci_fixture_only`,
`certified_bundle_acceptance=false`, `production_changed=false`,
`owner_session_acceptance=false`, `m8_complete=false`,
`product_minimum_os=unchanged`, `owner_package=not_produced`.

This initial source/run is historical and superseded by the reviewed exact-source
continuation above; its unchecked reaping/failed-send cleanup is not the final
accepted experiment implementation.

## Next boundary

The phase-one mechanism experiment and its source review are complete within the
explicit CI-only boundary. Next extend the experiment to an actively substituted
rendezvous/reply endpoint and transferred
rights, and integrate the certified two-binary/self-bundle policy in isolation.
Only after those prerequisites and review should an owner package be requested.

No owner Mac/VM action is required now. The original direct descriptor candidate
remains stopped. This scoped mechanism success does not close either original
custody finding, concurrent bundle replacement, steady-state Unix forwarded-FD
authorization, real Desktop/daemon lifetime/session acceptance or M8.
