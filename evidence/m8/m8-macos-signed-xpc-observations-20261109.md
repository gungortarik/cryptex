# M8 synthetic signed XPC observation package

Status: native harness CI passed; owner certificate run NOT RUN; full matrix INCOMPLETE.
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
