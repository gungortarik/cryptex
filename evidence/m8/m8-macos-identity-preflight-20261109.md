# M8 Mac identity prerequisites — 2026-11-09

## Scope and continuation

The owner asked to continue the remaining M8 work. The next accepted investigation
is the bounded synthetic Mac connection/instance-identity feasibility probe.
Production ADR-0007 Unix sockets, trust events, publication and normal startup
are unchanged. M8 remains OPEN; the signed native XPC matrix remains NOT RUN.

Earlier Mac receipts established developer tools and selected native primitives,
but did not inventory the code-signing prerequisites. A one-click prerequisite
package now collects that missing information before attempting signed clients,
different signers/identifiers, endpoint forwarding and restart/stale connections.
This package does not implement or claim those authorization tests.

## Package and privacy

Exact committed subset: MAC-KIMLIK.command, mac_identity_preflight.py,
mac_identity_api.c, MAC-KIMLIK-BASLA.txt and a closed SHA256 MANIFEST.json.
The packager reads files from the selected Git commit, verifies names/UTF8/LF/
bytes/Unix modes after ZIP creation and refuses to overwrite an existing output.
No binary, profile, private key, certificate or full checkout is shipped.

On Mac the launcher uses a fresh private /private/tmp directory outside source,
umask077, fixed system PATH and isolated/no-bytecode system Python. Missing
existing developer tools produce a diagnostic ZIP; nothing is installed. Only
result.json (or a fixed launcher.txt fallback) enters MAC-KIMLIK-SONUC.zip.
Result files contain source binding, OS/build/architecture/SDK, valid file-Keychain
identity/certificate/team-label COUNTS, API compile/presence and fixed diagnostics.
Names, certificate hashes/team IDs, home/SDK/scratch paths and raw command output
are not exported. No signing identity is used, imported, generated or unlocked.

`security find-identity -v -p codesigning` is read-only enumeration of valid
file-based Keychain identities. Zero is not proof that every possible identity is
absent: data-protection Keychain identities are not enumerated by that tool. A
nonzero count is not proof of private-key usability or two suitable distinct teams.
Actual selection and key-use/signing remain a later explicit probe step.

The small C program compiles/links the public declarations for peer signing
requirements and SecCode message validation. Its main only checks symbol presence
via dlsym. It creates no listener/connection/service and performs no protected
operation. Probe compile target12.0 is for this API check only; the product's
minimum OS remains unconfirmed and unchanged. No minimum-OS behavior is certified.

## Bounded command lifecycle and review

Collector commands have deadlines and a64KiB retained-output limit. They run in
fresh owned groups; output is never interpreted as shell code. Darwin kqueue
observes exit without reaping, then group cleanup happens before Popen.wait.
The leader PID remains reserved through cleanup, with a lock/reaped flag preventing
late output-reader kills after PID release. The reader owns unbuffered pipe closure;
main performs a bounded join and refuses a retained/broken pipe. Child environment
copies discard compilation/loader injection variables and use private TMPDIR.

Independent source review found and corrected a successful-leader/detached-child
cleanup gap and a late-reader PGID race. The POSIX regression now coordinates
ready/release explicitly, atomically publishes the child's ready PID, and checks
both a post-return continuation marker and actual gone/zombie state. The reviewer
also caught the initial ready-file visibility race; atomic ready publication was
applied. This is technical source review, not human certification or native proof.

Local Windows checks:8 platform-independent tests pass;4 POSIX process tests are
deferred to Mac execution. Bash syntax, non-Mac refusal, package hashes/modes,
duplicate/malformed inventory and manifest rejection, privacy and truthful scope
checks pass. Dedicated manual-only Mac CI will verify those native process bounds
and the API-only launcher; its result is recorded below after execution.

The new standalone workflow was not yet registered on the default branch, so its
first workflow_dispatch returned404 and executed nothing. The existing registered
CI workflow now exposes mac_identity_preflight_only, reusing the narrow Mac job
from this branch and skipping readiness/IPC/installer jobs for that explicit mode.
Normal CI defaults/permissions are unchanged; no main-branch integration needed.

First registered-branch run42686288860 on563a43ad failed the three native process
tests before API compilation: select.kqueue does not implement a context manager.
The wrapper now uses contextlib.closing and a platform-independent queue double
with only close/control catches that API assumption. No test assertion weakened;
rerun is required before delivering a package. That initial outgoing archive
was not delivered to the owner.

Run42686557847 on841dc03 passed deadline/descendant tests but output-cap cleanup
issued a second killpg after its reader had already killed the group; Darwin
reported EPERM. Group stopping is now one-time under the lifecycle lock, before
reaping. A native clean-exit/output regression also checks the successful path;
reaping runs even if cleanup itself errors. Assertions remain unchanged.

Run42686906555 onde92225 passed output/deadline/descendant checks; the added clean
exit regression exposed Darwin's all-zombie killpg EPERM behavior. Apple's XNU
killpg1 implementation filters SZOMB members and returns EPERM when none qualify.
EPERM is now accepted only after a bounded, exact-group ps snapshot contains no
live member (all Z or absent). Live/wrong-group/malformed/error snapshots still
refuse. The leader stays unreaped through that query, preventing PGID recycling.
No general permission-error suppression was added; a parser regression guards it.
Source: [Apple XNU killpg1](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_sig.c).

Run42687272930 on51690e7 passed11 tests (one non-Mac-only check skipped) including
all native process cases, then failed API linking. Disposable-CI-only compiler
diagnostics in run42687524508 identify unresolved CFRelease: CoreFoundation must
be linked explicitly beside Security. Both real collector and diagnostic command
now include that framework; no API check or assertion was removed. Raw compiler
diagnostics stay on the disposable CI host; owner receipts retain fixed codes.

## Verified outgoing candidate and owner continuation

Source `59411433c5c903bb15d2251f135f911c4b295fa7` passed the narrow Mac-only
[CI42687722399](https://github.com/gungortarik/cryptex/actions/runs/42687722399),
job129398931515, 17seconds. All11 Mac-applicable tests passed; the one non-Mac
refusal check was skipped on Mac and passed on the Windows authoring host. Actual
deadline, streaming output cap, clean exit and detached-descendant cleanup passed.
The Darwin EPERM exception was independently source-reviewed with no actionable
safety/receipt issue. Its two-column owned-group ps output is buffered before its
64KiB acceptance check (not a streaming cap); that query has its own2s timeout.

The extracted one-click launcher ran on macOS26.7.2/build25G332 arm64, SDK26.5:
API compile/link and runtime symbol presence passed; runner_exit0 and
outcome=prerequisites_collected. That disposable CI account enumerated zero valid
file-Keychain entries. This is not the owner's Mac inventory. Full XPC matrix,
exact-instance binding, product minimum OS and private-key use remain untested.
All other product-readiness/ACL/installer jobs were deliberately skipped in this
narrow mode; retain product-code CI42564404285 separately. No product Rust changed.

Outgoing `cryptex-mac-kimlik-59411433.zip`, SHA256:
`7d80c39594a7e111dbd8fb27239a1be25f6c83089e704c88d08a1907f6f08730`.
Local archive and the exact archive exercised on Mac CI have identical SHA256.
Five names, payload member hashes, UTF8/LF and executable ZIP mode were rechecked
after packaging. The earlier undelivered563a43ad archive was removed only after
its recorded hash matched, leaving the verified candidate for the owner.

## Submitted Mac prerequisite receipt (2026-11-09)

The owner returned `MAC-KIMLIK-SONUC.zip`, SHA256
`516f4b6c9871af6824e72fb58a6d3aa7b38cb9d3124a410c7b9c6affd97db9f1`.
Its only member is `result.json` (993 bytes), collected at
`2026-11-09T19:58:48Z` and bound to source
`59411433c5c903bb15d2251f135f911c4b295fa7`. The supplied local observations
report macOS27.2/build26B123 arm64 and SDK27.2; API compile and runtime presence
both passed, outcome=prerequisites_collected and runner_exit=0.

Valid file-Keychain entries, distinct certificates and observed Apple team-label
counts are all zero. Data-protection Keychain was not enumerated, so this does
not establish that every possible signing identity is absent. No private key was
used. signing_selection_needed=false means no enumerated candidate to select;
signed_matrix_prerequisites_proven remains false. This is an owner-supplied
receipt, not remote attestation or a signed authorization-matrix result.

The user clarified that this was an available Mac, **not their own Mac**. Treat
the inventory as specific to that machine/account; do not attribute it to the
user's personal Mac or reuse it as another host's signing readiness. Access to
the tested Mac and its existing Xcode account/team are not yet established.
Do not repeat this successful API preflight on the same unchanged environment.
Next: establish access to a suitable Mac and existing signing identity before
running the authorized bounded signed matrix. A different Mac may need its own
prerequisite check. Missing identity availability is a reported prerequisite,
not a repository failure or permission to generate/import credentials.

Full XPC matrix remains NOT RUN, exact-instance binding NOT TESTED, product
minimum OS unconfirmed, production_changed=false and m8_complete=false. No
Windows VM or repeated pairing/transfer package is needed for this prerequisite.

## Personal MacBook follow-up receipt (2026-11-09)

The user supplied a new `MAC-KIMLIK-SONUC.zip` explicitly from their own MacBook.
SHA256: `e9e620e8f3749b883152f4d4ab334338c6df724a44c12a797ee2b0b51c9d706f`.
The sole member is result.json (989 bytes), collected at2026-11-09T21:04:04Z,
source59411433c5c903bb15d2251f135f911c4b295fa7. The local receipt reports
macOS27.2/build26B123 arm64, SDK27.2, API compile=pass, API presence=pass,
outcome=prerequisites_collected and runner_exit=0.

This account reports one valid file-Keychain entry, one distinct certificate and
one observed Apple team label. signing_selection_needed=true and
next=select_existing_signing_identities. This supersedes the earlier zero-count
environment as the working Mac prerequisite result, without contradicting it:
the two receipts describe different machines/accounts according to the user.
The sanitized receipt contains no certificate selector, team identifier or key.
It neither proves usable private-key access nor supplies a different-signer
fixture for the full negative matrix. No credential was exported or used.

Next: select the existing identity on the personal Mac and prepare the bounded
synthetic signed probe with explicit fixture requirements. Do not silently create
or import additional credentials. Do not rerun the successful API preflight.
signed_matrix_prerequisites_proven=false, native_xpc_matrix=not_run,
exact_instance_binding=not_tested, product_minimum_os=unconfirmed,
data_protection_keychain=not_enumerated, private_key_use=not_requested,
production_changed=false and m8_complete=false remain explicit receipt limits.
This receipt is owner-supplied local evidence, not remote attestation.

## Primary sources

- [Apple code-signing identity enumeration](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/).
- [Apple TN3161: file vs data-protection Keychain enumeration](https://developer.apple.com/documentation/technotes/tn3161-inside-code-signing-certificates).
- [Apple XPC peer requirement](https://developer.apple.com/documentation/xpc/xpc_connection_set_peer_code_signing_requirement(_:_:)).
- [Apple SecCodeCreateWithXPCMessage](https://developer.apple.com/documentation/security/seccodecreatewithxpcmessage(_:_:_:)).
- [Apple DTS API availability and process identity caveats](https://developer.apple.com/forums/thread/681053).
- [Python os.waitid platform availability](https://docs.python.org/3/library/os.html#os.waitid): Mac support added3.13; the system-Python launcher therefore uses documented kqueue instead.
- [Python kqueue API](https://docs.python.org/3/library/select.html#kqueue-objects).
