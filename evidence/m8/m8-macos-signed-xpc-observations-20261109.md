# M8 synthetic signed XPC observation package

Status: owner certificate partial run received2026-11-10; signer-only insufficiency
observed; full matrix INCOMPLETE and exact-instance binding NOT PROVEN.
This continues the authorized XPC feasibility investigation only. Accepted
ADR-0007 production Unix-socket IPC, canonical data, trust and transport are unchanged.

The personal Mac prerequisite receipt on source59411433 found one existing valid
file-Keychain certificate. MAC-IMZA.command selects that single certificate locally,
compiles fixed public-API C source, signs synthetic client/server fixtures with no
timestamp network request, verifies signatures and runs only benign nonce echoes.
Multiple certificates refuse automatic selection. No certificate/key is created,
imported or exported; existing Keychain approval may be requested for codesign.
The user may allow once or cancel; no Always Allow/security-setting change is needed.

## Experiment and acceptance boundaries

- The guarded listener configures the expected client code requirement before
  peer resume. Requirement = fixed synthetic identifier plus exact certificate leaf.
  The client pins the corresponding synthetic server requirement before resume.
- Every delivered dictionary derives SecCode from that exact XPC message and calls
  SecCodeCheckValidity before the nonce operation. The separate observer listener
  deliberately isolates message validation without a connection-level client pin.
- Normal expected clients run before/after negatives. Wrong-identifier, ad-hoc and
  signature-removed fixtures are separate observations. An OS launch failure,
  timeout, connection error or absent log never counts as authorization rejection.
  Guard rejection needs an explicit peer-requirement error, successful positive
  controls and the paired observer's delivered, nonce-matched invalid-code result.
- An expected client obtains the guarded endpoint and forwards it through a
  synthetic courier to a separately executed same-requirement client, after origin
  exit/reap. If accepted, record signer-only insufficiency. This is an endpoint
  lifetime observation, not stale actual connection reuse or an exact-instance fix.
- No second certified signer is selected; ad-hoc is not a substitute. The full
  matrix outcome stays INCOMPLETE, actual minimum product OS is unconfirmed,
  exact-instance binding is unproven and M8 remains OPEN.

## Runtime, receipts and cleanup

All files stay in a fresh owned0700 /private/tmp/cryptex-mac-imza.* directory.
A random exact current-user launchd job advertises three synthetic Mach services;
its plist is never installed in LaunchAgents. Bootstrap requires its exact service
target to be absent. Cleanup bootouts only that owned target and requires print113
to report registration_removed. It does not separately prove server process exit;
the receipt says so. The server also has a90s lifetime watchdog and128-record cap.
INT/TERM use cleanup; launcher retries if the ownership marker remains. SIGKILL
cannot promise cleanup. An unconfirmed removal is a failed cleanup receipt.

The existing bounded child-process runner supplies deadlines, output caps and
owned-process-group cleanup. Key-use attempted state is recorded before codesign,
even if a later operation fails or the user cancels. Exported ZIP members are
only result.json and fixed launcher/cleanup diagnostics; private inventory, raw
requirements, launchctl/compiler logs, key selectors and real certificate names
stay out. Receipts carry fixed synthetic identifiers, source/binary hashes and
per-case observations. They are local owner-supplied evidence, not attestation.

## Validation

Windows authoring:10 new unit tests pass, including reject-everyone/paired-observer
controls, no false negatives from launch/transport failure, attempted-key-use
failure/interruption, exact owned-job cleanup, bounded log parsing and tampered
package refusal. Existing prerequisite tests:8 pass,4 POSIX checks skipped locally.
Independent source-only review found and corrected two receipt false-pass/truth
issues; final precision corrections separate origin/recipient messages and
registration absence from process death. No production Rust or frontend changed.

Mac CI must compile with warnings denied, run the anonymous ad-hoc same-process
smoke and the shared cross-process/courier harness using CI-only ad-hoc codehash
requirements. Those are harness tests, not the owner's certificate or complete
signed feasibility. Package only committed source after the native checks pass.

Primary references: [Apple XPC peer requirements](https://developer.apple.com/documentation/xpc/xpc_connection_set_peer_code_signing_requirement(_:_:)),
[message-derived SecCode](https://developer.apple.com/documentation/security/seccodecreatewithxpcmessage(_:_:_:)),
[XPC endpoints](https://developer.apple.com/documentation/xpc/xpc_endpoint_create(_:)),
[launchd jobs](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html).

## Verified candidate and outgoing package

Source `ee5d41764aef396e3064ffb52c1f357e2acca310` passed narrow Mac
[CI42701476626](https://github.com/gungortarik/cryptex/actions/runs/42701476626),
job129447049254,12seconds. XPC10tests and11 applicable prerequisite tests passed;
one non-Mac refusal test skipped. Warning-denied native compile, same-process
ad-hoc smoke, shared real cross-process exercise and launchd registration removal
passed. Expected guarded/observer echoes before/after negatives succeeded;
wrong-ID observer explicitly rejected the delivered message. Wrong-ID guarded
client returned connection_error and remains inconclusive, not a rejection pass.
The same-requirement recipient echoed through the forwarded endpoint after origin
exit, observing signer-only insufficiency in this CI ad-hoc/codehash environment.
It does not establish the personal certificate result or a production custody fix.
General readiness/Windows ACL/installer jobs were intentionally skipped; retain
earlier full product CI evidence separately. A follow-up CI-only receipt correction
moves the unproven-server-exit field before serialization; a fault-injected test
checks that limit and cleanup survive probe failure. Owner-package bytes unchanged.

Outgoing `C:\Users\Tarik\Downloads\cryptex-mac-imza-ee5d4176.zip`,15895bytes,
SHA256 `b659127a06f62ca44aebba766d5d673bfb7a2691ea33d26517a75cb75874ce4d`.
Packaged only exact committed source. Closed six-member inventory, payload hashes,
UTF8/LF and executable launcher mode were verified. Source-only independent review
confirmed the receipt/authorization precision corrections; owner run remains pending.

Owner next: extract on the personal MacBook, double-click MAC-IMZA.command,
allow codesign key access once if asked (or cancel), return MAC-IMZA-SONUC.zip.
No terminal, Windows VM, new certificate or repeated pairing/API preflight needed.

## Personal MacBook signed observations received (2026-11-10)

The owner returned `MAC-IMZA-SONUC.zip`, SHA256
`21d85852f84d59873137c08ec61ca47aabd35b9730dfe061a55c84237f8a5510`.
The sole regular member is result.json,4809bytes, collected at
2026-11-10T20:19:16Z on sourceee5d41764aef396e3064ffb52c1f357e2acca310.
Receipt source hash matches the committed native C source:
`9c47fe03a5c5c54556d01dfdb42c60d406f510085d724ac490af4d897f3349df`.
Duplicate JSON keys, source binding, case consistency and explicit limits were
checked. The supplied receipt reports macOS27.2/build26B123 arm64, SDK27.2,
native_compile=pass, single_existing_certificate and
private_key_use=synthetic_fixture_signing_succeeded. Runner exit0 means the
observation harness finished, not that every authorization case passed.

| Observation | Owner receipt | Conclusion |
| --- | --- | --- |
| Expected guarded/observer client, expected after negatives, fresh process | Echo, exit0, one valid delivered message each | Four positive controls passed |
| Wrong identifier, observer lane | Explicit message_rejected, delivered validation false | Message-level rejection passed |
| Ad-hoc signature, observer lane | Explicit message_rejected, delivered validation false | Message-level rejection passed; not a second certified signer |
| Wrong identifier/ad-hoc, guarded lane | connection_error, no delivered message | Inconclusive; not an attributed connection-level authorization refusal |
| Signature-removed executable, both lanes | client_launch_or_exit_failure, exit-9 | Inconclusive; does not prove unsigned XPC sender rejection or the cause of termination |
| Origin endpoint courier | stored, exit0 | Courier control only, not authorization proof |
| Same-requirement recipient after origin exit | Echo, valid delivered guarded message | Signer-only insufficiency observed |

The synthetic endpoint remained usable by a separately executed same-requirement
client after the origin exited. Thus certificate/identifier requirements and
message-derived SecCode validation alone did not establish the expected original
process instance in this tested courier topology. This is not a bypass of an
implemented parent-launch custody mechanism: none was implemented or tested.
Do not claim production IPC infeasible in every topology, or weaken authorization
to PID/signer-only matching. The already authorized bounded investigation must
next analyze/test exact parent-launch connection custody and stale/cross-connection
refusal before any production architecture decision. Keep production Mac owner
operations fail-closed under accepted ADR-0007.

Full feasibility remains INCOMPLETE: second certified signer not selected/tested,
actual stale-connection reuse not run, minimum product OS unconfirmed, exact-instance
binding not proven. Launchd registration removal is reported; server process exit
was not independently observed. production_changed=false and m8_complete=false.
These are owner-supplied local observations, not remote attestation. The raw
receipt/archive stays outside Git. Do not repeat successful signing/API/Windows
pairing tests; no new owner device batch is requested for this receipt review.
