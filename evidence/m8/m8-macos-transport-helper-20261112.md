# Disabled Mac transport helper — implementation checkpoint

ADR-0021 records the owner's delegated selection of the proposed helper role.
This checkpoint adds a feature-only entry in the existing signed daemon before
ordinary lifecycle/canonical initialization. A purpose/root/release/epoch-bound
native channel authenticates the actual parent and captured helper child.

The helper's private session/group owns the unmodified pinned Syncthing 2.1.5
monitor/server tree. Existing profile/XML/API-credential and owned-socket HTTP
logic is reused through Mac adapters. New task-generation and established-socket
FD checks use public SDK declarations; no application bytes precede ownership
validation. A read-only peer-generation duplicate carries no owner authority.

The canonical daemon's private reader submits a fresh bounded helper observation
for each identity request. A ready cache is scheduling information only. Native
read/offer/proof dispatch uses this reader only in explicit transport fixture
create/resume modes; all ordinary Mac product gates remain closed.

An active helper marker precedes spawn. The parent retains the helper unreaped
through group cleanup; unresolved cleanup leaves the marker and prevents another
start. Clean and forced results remain distinct. The helper watches its parent
generation during engine waits, and a forced group cleanup terminates the helper
too; its daemon parent owns cleanup verification. Combined supervisor-loss
acceptance remains a separate required test, not a claimed Windows Job equivalent.

Local checks: Rust format, all-target daemon strict clippy and Python tooling
(76 tests, 5 platform skips) pass on Windows. No Mac execution is inferred from
those checks. Mac private-profile/protocol unit tests and the actual signed
helper/engine startup, foreign-socket refusal, reopen and helper-loss driver are
added to the scoped CI path. Their execution result is pending at this checkpoint.

The new source has not had independent review or owner-Mac execution. Native
Desktop positive pairing and remaining loss/credential cases still require
evidence before this source is described as completing the identity connection.

## Execution attempts and corrections

At `6def6c4`, [run 43303477995](https://github.com/gungortarik/cryptex/actions/runs/43303477995)
passed the Windows startup job and compiled/linted the Mac feature. The actual
helper refused with `mac_transport_Package`: the transport resources had been
prepared but omitted from the CMS inventory's file list. They are now listed
explicitly. The cleanup path also now excludes a captured child only after
`waitid(WNOWAIT)` proves that exact unreaped child exited; it does not require
Darwin to return BSD information for that zombie. Other unavailable/truncated
membership observations still refuse clean completion.

At `d2624df`, [run 43304848703](https://github.com/gungortarik/cryptex/actions/runs/43304848703)
again passed Windows and compiled/linted the Mac feature. The actual helper
advanced into native generation but returned `native_config_GenerationFailed`.
The sanitizer had supplied Windows system variables and explicit `--home`, but
not HOME. Upstream 2.1.5 resolves the user's home in package initialization,
before command-line home overrides: see its [locations initialization](https://github.com/syncthing/syncthing/blob/v2.1.5/lib/locations/locations.go)
and [home expansion](https://github.com/syncthing/syncthing/blob/v2.1.5/lib/fs/util.go).
Candidate `4254f3e` gives the engine HOME/TMPDIR equal to its private isolated
profile, rather than forwarding the user's real home or ambient loader/ST state.
This diagnoses a concrete missing prerequisite; successful native generation
and end-to-end pairing still require the fresh execution result.

The same candidate adds actual signed Desktop/daemon offer/proof, both role
approvals, canonical completion and byte-identical completion replay after
same-profile restart. API-key deletion must produce a typed credential refusal
without regenerating the key/config/identity. A purpose-bound negative reply
is acknowledged before the helper exits, preserving live-sender validation.
There is no dead-peer exception for queued messages.

A separate bounded driver kills only its captured daemon child after real
engine readiness. Its group numbers are used for read-only `ps` evidence,
never for adopting/killing a process. It requires group exit and an unresolved
active marker, then checks that reopening refuses without changing the native
profile. This is a daemon-only loss fixture, not combined supervisor-loss or
Windows Job equivalence. The runtime reader still uses native generation/socket
proof; `ps` does not authenticate a transport identity or authorize a request.

The private-profile test now rejects an actual extended ACL, the native channel
test exercises an independently retained read-only liveness pin, and the actual
HTTP stream is reobserved for at most 250 ms while Darwin installs the accepted
server FD. The absolute request deadline is unchanged; no reconnect or credential
write precedes exact ownership. Darwin ACL entry success is zero, as documented
in Apple's [acl_get_entry manual](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/acl_get_entry.3.html).

Local format, strict daemon clippy, launcher syntax and 81 Python tests
(5 platform skips) pass. The fresh exact-source run is
[43306764215](https://github.com/gungortarik/cryptex/actions/runs/43306764215),
including execution of the exported one-click launcher when preceding checks
pass. Execution is pending here. The aggregate-only Mac transport package is
prepared in tooling; it is not accepted from packaging alone. Independent source
review, owner-Mac execution, other endpoint families/exec races, and combined
supervisor-loss acceptance remain open. Ordinary product gates remain closed.

## Later execution/debugging checkpoints

Run [43306764215](https://github.com/gungortarik/cryptex/actions/runs/43306764215)
at `4254f3e` passed Windows but stopped in the newly enabled Mac private-profile
test: its positive file check still had the deliberate hard link attached.
The fixture now uses an independent file for the ACL check; hard-link, symlink,
mode and actual ACL refusal assertions are retained. No policy was loosened.

Run [43308584628](https://github.com/gungortarik/cryptex/actions/runs/43308584628)
at `778a427` passed Windows and the Mac adapter tests. The native helper driver
then failed with a channel error, without enough pre-cleanup diagnostics to
identify the operation which ended the helper. This is not a successful
generation/owned-REST/pairing result, nor a confirmed diagnosis of that failure.

Source `171465d` now uses the public SDK's `POSIX_SPAWN_START_SUSPENDED` for
engine launch, validates the exact captured stopped image, and only then sends
SIGCONT. The private-group/parent relationship and independent engine CDHash
remain required. A short-lived generator cannot exit before that capture, and
replacement image code cannot run before the live-image check. The actual
[Apple spawn-flag contract](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man3/posix_spawnattr_setflags.3)
defines this pre-userspace stop and SIGCONT resume. No private numeric flags or
OS structures were copied. A captured-child owner handles partial spawn errors
as well as normal reaping. The native listener now observes child exit with
WNOWAIT instead of reaping it before its owner finishes cleanup; an actual
exited-before-accept fixture asserts the child remains waitable afterwards.

That [run 43311368960](https://github.com/gungortarik/cryptex/actions/runs/43311368960)
stopped at strict Mac compilation because SDK 26 deprecated `addchdir_np` in
favor of `addchdir`. Source `dc24aaf` selects the new public function under an
availability guard and retains the earlier public function for older SDK/OS
combinations. It does not suppress warnings or select a product support floor.
Only the current runner branch is exercised; the older-system branch remains
unverified. The current exact-source run is
[43312391943](https://github.com/gungortarik/cryptex/actions/runs/43312391943),
pending at this checkpoint. New native operation/stage/error diagnostics are
fixed labels or numeric codes, never raw engine logs, keys or configuration.

The earlier `43308456165` dispatch was cancelled before acceptance to consolidate
the typed worker diagnostic's lint fix. It supplies no passing test evidence.
One-click export remains conditional on the native checks passing and also
requires running the exported launcher itself. No new owner-Mac pass, independent
review, cross-device replication, combined-loss cleanup or M8 completion is
claimed by these development checkpoints.

The SDK-26 runtime guard at `dc24aaf` still triggered a deprecation warning for
its older-OS branch. Run [43312391943](https://github.com/gungortarik/cryptex/actions/runs/43312391943)
therefore failed strict feature compilation as well. `abbbfed` leaves the
warning-as-error policy intact: a new-SDK build uses the new public `addchdir`
when available and returns ENOTSUP on an older OS until that compatibility path
is validated. An older SDK can still compile its original public `_np` call.
This candidate limitation is explicit; it is not a selected product support
floor. Current exact-source execution is
[43313882134](https://github.com/gungortarik/cryptex/actions/runs/43313882134),
pending here. The new pre-userspace pin and unreaped-child changes have not yet
established a successful actual engine/pairing run.

Run [43313882134](https://github.com/gungortarik/cryptex/actions/runs/43313882134)
compiled the Mac feature but lost the helper before its Ready message. Pre-drop
diagnostics at `52fa831` then showed in [43315350104](https://github.com/gungortarik/cryptex/actions/runs/43315350104)
that actual owned REST succeeds; the foreign-socket negative fixture returns
InvalidState immediately afterwards. Windows passed that exact source. No key,
engine log, config or memory body was printed to diagnose the failing stage.

Source `5c86cd5` explicitly clears the accepted probe socket's nonblocking mode
before its three-second read timeout. Darwin can inherit that mode from the
listener; immediate WouldBlock is not evidence of EOF. The assertion still
requires both PeerNotOwned and an actual zero-byte EOF, and rejects any received
byte. Apple's [accept contract](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/accept.2.html)
documents that accepted sockets inherit the listening socket's properties.
An actual TCP regression test covers delayed close and a delayed one-byte
write with nonblocking explicitly set before entering the probe. Failure-only
diagnostics distinguish the typed REST refusal from the boolean EOF result.
The exact-source [run 43317681202](https://github.com/gungortarik/cryptex/actions/runs/43317681202)
is pending. The native pairing/exported launcher and remaining evidence gates
above are still unverified for this candidate.

Run [43317681202](https://github.com/gungortarik/cryptex/actions/runs/43317681202)
at `5c86cd5` passed Windows and the foreign-socket/no-credential fixture on Mac,
then returned a helper channel error. Source inspection found a stop lifecycle
race: after sending its reply the helper closed/exited before the parent could
consume it, correctly triggering the native channel's queued-after-exit refusal.
`f586420` requires a purpose-specific parent STOP_ACK before the helper exits.
The parent keeps its endpoint live through ACK consumption and verified group
death/reaping. No dead-peer/closed-endpoint exception is introduced. The actual
driver and exported launcher now require the clean-stop-ACK pass marker as well
as the existing reopen/loss/refusal markers. Existing queued-after-exit native
negative tests are retained. Local format/diff checks and 81 Python tests pass
(5 platform skips). Exact-source [run 43319240925](https://github.com/gungortarik/cryptex/actions/runs/43319240925)
is pending; the full native pairing and package remain unaccepted until executed.

Run [43319240925](https://github.com/gungortarik/cryptex/actions/runs/43319240925)
at `f586420` passed Windows and all actual helper checks: foreign socket before
credential bytes, clean-stop ACK, same-identity reopen, helper-loss cleanup and
missing-key no-regeneration. Native Desktop pairing started both real engines
but then failed with Channel. Source comparison found the domain already emits
`bootstrap.approval.signed` on successful confirm, while LocalDaemon::finish_peer
expected the generic refusal operation `bootstrap.approval.result`.

`fcad5a3` / `5ffdd4b` correct that contract and require a nonempty bounded signed
message for successful signing. Typed generic confirm refusals remain accepted
only when they carry no signed message, snapshot, grant or identity payload and
match the original request. Regression assertions reject missing signed output,
generic-result success, payload-bearing refusals and another request's ID. The
size limit is shared privately within the native adapter, without a public UI
API. Exact-source [run 43320999981](https://github.com/gungortarik/cryptex/actions/runs/43320999981)
is pending. The earlier `43320940326` dispatch was cancelled immediately to
include the shared constant's Rust visibility correction; it is no acceptance
evidence. Native pairing/reopen, daemon-loss and exported launcher still need
their actual successful results.

Run [43320999981](https://github.com/gungortarik/cryptex/actions/runs/43320999981)
at `5ffdd4b` passed Windows and helper checks. The native pairing driver then
reported Cleanup after the signing-contract correction. Its transport-specific
LocalDaemon::close waited for daemon exit after sending Native Close, but the
outer lifecycle hold still required its existing stop file. Source `39c7415`
retains the backend-selected stop sibling of the authenticated ready path and
reuses shutdown for a bound live transport daemon; invalidated/unbound owners
retain the existing termination fallback. The helper drains before the daemon
returns to its lifecycle stop check. No deadline or process identity check is
relaxed. The real driver/tooling now requires native_managed_pairing_clean_shutdown
as well as pairing/reopen markers. Local format, strict Windows daemon clippy
and 81 Python tests pass (5 platform skips). Exact-source
[run 43322732272](https://github.com/gungortarik/cryptex/actions/runs/43322732272)
is pending; no exported launcher or independent/owner-Mac acceptance is inferred.

Before accepting the lifecycle integration, `c2d8e5b` / `04fa52e` replace its
stop-file truncation with exclusive no-follow creation and fsync. Resume can
retain a prior marker: that exact five-byte single-link owned regular file is
verified read-only, never overwritten. Writable, mismatched, linked or nonregular
existing leaves refuse. Regression tests cover new creation, read-only repeated
shutdown, unchanged conflicting bytes and a symlink whose target stays intact.
Dispatches `43322732272` and `43322986345` were cancelled before acceptance to
include these marker protections and resume handling. Current exact-source
[run 43323149190](https://github.com/gungortarik/cryptex/actions/runs/43323149190)
is pending; these cancelled attempts supply no success evidence.

## Exact-source passing CI checkpoint

Source `04fa52eb6b8ba527036a07b9eee5b49a89b80d15` passed both selected
Mac/Windows startup jobs in [run 43323149190](https://github.com/gungortarik/cryptex/actions/runs/43323149190).
This was the scoped manual desktop_startup_only workflow, not all milestone jobs.
The [structured receipt](macos-transport-native-04fa52e-ci.json) records the exact
job IDs and pass markers. All real helper checks, native Desktop pairing, clean
daemon/helper shutdown, idempotent reopen, parent daemon loss and unresolved
replacement refusal passed. Existing content/session/Keychain replay checks
also passed. The exported transport launcher was executed and its matching-source
receipt required all five identity/pairing/loss/credential/runtime booleans true.
This is actual local real-engine/native integration, not cross-device replication
or a human WebView pairing acceptance claim.

Artifact 13990599689 (mac-interactive-ui) contains the transport ZIP (25,290,975
bytes), SHA-256 a5fdc1723b460d61bcd281988b4ffc6161d94a22881a9b7d5393ff8621cabdcc.
Its fixture.json source matches the tested commit, architecture is arm64 and
expiry is 2026-11-13T17:48:45-05:00. The exact launcher and fixed package namespace
were inspected after download. This candidate's new-SDK spawn branch requires
macOS 26 or later; no product OS floor is selected.

Independent read-only source review is in progress at this checkpoint. An owner
Mac run, combined supervisor-loss acceptance, other endpoint/exec races and
product signing/support-floor/activation decisions remain open. Default product
routes stay closed. Continue with the reviewed exact-source one-click package
and the owner's aggregate result; do not repeat the manual JSON transcript loop.

## Independent source findings and correction candidate

The independent read-only review at `04fa52e` found two P2 issues despite passing
CI: live observation omitted revalidation of persisted profile/cert/key/XML and
receipt continuity, and an outer identity-driver timeout/crash could skip Rust's
tail Keychain deletion. No additional concrete blocker was found in helper role,
native purpose binding, captured process/group ownership, exact established
socket attribution, pairing contract or package export. Declared combined-loss,
endpoint/exec, owner-device and product gates were not treated as solved.

Source `14e8839` validates current profile identity, intent/ports, retained receipt,
exact generation/API markers, config policy, cert/key hashes and API binding/OS
credential before and after each owned REST observation. The helper also checks
all independently pinned package bytes before/after each Observe. This default-off
experiment deliberately pays two full bounded package hashes per observation,
including idle polling; it introduces no cached authorization or new product
performance claim. Eleven post-start mutation cases require identity refusal,
verified failed-worker cleanup and unchanged tampered bytes. Only the explicit
fixture restores its own derived files after verified cleanup. No canonical event
is modified by these tests.

Both CI and owner launchers now wrap the identity driver in Python finally cleanup
of its fresh exact metadata-derived API account. Deletion accepts only success or
the exact missing-item exit; a separate bounded find without password output must
prove absence. Failed test outcome/runtime evidence is retained. Mock timeout and
crash-after-key-creation, already-deleted key and unsafe-label tests were added.
Local format/diff/strict daemon clippy and 83 Python tests pass (5 platform skips).
Exact-source [run 43326133084](https://github.com/gungortarik/cryptex/actions/runs/43326133084)
and the focused independent correction review are pending. The older passing ZIP
is superseded and must not be delivered as the reviewed owner candidate.

The first outer cleanup correction did not yet fence a helper's late provisioning
write. Corrections through `dbb3657` use the same exclusive profile lease for a
fixed signed cleanup driver, retain it through deletion/final typed Missing and
recheck a helper's captured parent immediately after acquiring that lease before
provisioning. Raw unresolved driver waits refuse cleanup; no raw CLI fallback
can bypass the fence. Real-lock tests cover earlier and delayed writers and
already-Missing verification under lease. Pairing/parent-loss API cleanup was
changed to the same fence as well.

The unsplit `14e8839` [run 43326133084](https://github.com/gungortarik/cryptex/actions/runs/43326133084)
passed Windows and nine actual mutation refusals but exceeded the old aggregate
identity-driver 180-second watchdog. `b3ee9ba` separates the expanded test-only
matrix into closed 6+5 enum-selected batches with independent fresh roots and
accounts, capped concurrency two, unchanged per-driver/per-request limits and
unchanged exported 360-second absolute watchdog. Both futures and their fenced
finally cleanup settle before the aggregate mutation marker; a failure does not
cancel or abandon the other batch. This is test orchestration, with no helper
wire command or product lifecycle change. Earlier `43327486098` / `43327663341`
dispatches were cancelled before acceptance; they supply no passing evidence.

[Independent read-only review](m8-macos-transport-independent-review-20261112.md)
through `b3ee9bad0f0afb46101160e93eb4b85d3fb30ea4` found no remaining actionable
blocker for the disabled slice. Local checks include 86 Python tests (5 platform
skips), strict Windows daemon clippy, format and diff checks. Exact-source
[run 43328920581](https://github.com/gungortarik/cryptex/actions/runs/43328920581)
is pending. Only its passing reviewed artifact may be sent for owner-Mac execution.

Run [43328920581](https://github.com/gungortarik/cryptex/actions/runs/43328920581)
at `b3ee9ba` passed Windows but its new cleanup CLI correctly refused an alias
fixture path with Arguments: the Python CI temporary root used Mac's /var
spelling while the backend requires the physical /private/var root. `74b95bd`
canonicalizes backend-selected fixture locators before selecting driver/batch
paths and before metadata/cleanup invocation. The daemon validator is unchanged.
A regression requires physical root arguments even for an alias-shaped locator.
The independent source reviewer found no actionable issue in this correction.
Local Python tests now total 87 (5 platform skips). Current exact-source
[run 43330307492](https://github.com/gungortarik/cryptex/actions/runs/43330307492)
is pending; earlier artifact/source checkpoints remain superseded.

The first attempt of `43330307492` passed Windows and all new Mac transport,
eleven live mutation, native pairing, clean restart and daemon-loss checks. It
then hit the existing 30-second `resign_wrong_identifier_deadline` while codesign
was preparing an older startup-refusal fixture. This is not a transport or
request timeout pass. Only failed jobs were rerun at the same full source SHA
(attempt 2); no code or deadline changed. A passing complete Mac job and exported
launcher receipt are still required before delivery.

Attempt 2 reproduced the same codesign fixture deadline after all the new native
transport/mutation/pairing/loss markers passed again. `0d98a16` routes certificate
fixture signatures through an immediate unlock of the exact disposable signing
keychain before codesign. The ad-hoc missing-leaf negative case never uses the
keychain. Unlock failure stops before signing; neither lock settings, ACLs nor
native/signing/exported deadlines change. Three regressions cover repeated
immediate unlock, fail-closed unlock and the unchanged ad-hoc case. All 90 Python
tests pass (5 platform skips). Exact-source
[run 43334196206](https://github.com/gungortarik/cryptex/actions/runs/43334196206)
and focused source review are pending. No owner package from the superseded
passing source has been delivered.

## Reviewed exact-source passing artifact and next owner action

Source `0d98a16d075ca317b208c6e3dd6b0c66b64df707` passed
[CI 43334196206](https://github.com/gungortarik/cryptex/actions/runs/43334196206):
Mac job 131586116554 and Windows job 131586115017 both succeeded. This is the
scoped desktop_startup_only workflow, not all milestone jobs. The codesign
negative fixtures passed after the disposable-keychain correction; this result
does not independently prove the cause of the two earlier hangs. Focused source
review found no actionable finding in the correction. All 90 local Python tests
pass with 5 native platform skips; prior native lint/format/diff checks passed.

Actual Mac execution passed all five core helper controls, the closed 6+5 live
mutation inventories, actual native Desktop pairing, clean stop/reopen and
idempotent canonical state, daemon-loss group exit and unresolved replacement
refusal. Earlier content, native session replay, OS-store and eight signed-startup
refusal controls passed. The exported MAC-AKTARIM launcher ran under its unchanged
360-second absolute watchdog. Its matching-source receipt required passed outcome
and identity, pairing, loss, credential cleanup and runtime cleanup all true.
See the [structured receipt](macos-transport-native-0d98a16-ci.json) for exact
markers, source, job IDs and the explicit execution/evidence limits.

Artifact 13997720218 was downloaded from this passing run. The transport ZIP has
15 unique regular-file members in the fixed Cryptex-Mac-Aktarim namespace; all
signed-inventory member sizes and SHA-256 hashes matched after download. It
contains no fixture private signing key, PKCS#12 identity or Keychain database.
The source, arm64 architecture, executable launcher and unexpired fixture metadata
were checked before copying it to the owner's Downloads directory:

- Filename: `CRYPTEX-MAC-AKTARIM-0d98a16.zip`
- Bytes: 25,307,258
- SHA-256: `747f53476cc23c7ed10da4a4d047b052d9d0bd47f8b3dae861d29aeeb352a154`
- Fixture expiry: 2026-11-14 00:06:25 UTC
- Candidate platform: Apple Silicon, macOS 26 or later; not a product OS-floor decision

The older passing 04fa52e ZIP remains superseded and must not be delivered as this
reviewed candidate. The next action is the owner's Mac: extract the matching ZIP,
double-click MAC-AKTARIM.command, keep the window open and return the generated
MAC-AKTARIM-SONUC-<nonce>.zip. No terminal commands or signed JSON exchange are
needed. If expired, rebuild this reviewed source before another owner run.

Owner-device execution remains pending. This is local real-engine identity and
headless native Desktop pairing evidence, not cross-device replication, human
WebView pairing or production activation. Combined supervisor-loss acceptance,
other endpoint/exec races, product publisher/support floor and ordinary activation
remain open. ADR-0021 and all default-off product gates remain unchanged; M8 is open.

## Subsequent owner-Mac result

The owner returned `MAC-AKTARIM-SONUC-0616b9b2.zip` for the exact reviewed source.
Its single bounded JSON report has passed outcome and all five collector checks
true. The [receipt](m8-macos-transport-owner-receipt-0616b9b2.md) records its hash,
source/schema checks and aggregate-only evidence limit. The owner-device gate
for this candidate is now closed; no unchanged package or JSON-loop rerun is
required. This does not close combined supervisor-loss, other endpoint/exec,
cross-device or product activation gates. Continue with combined daemon/helper
supervisor-loss acceptance within the existing default-off scope.
