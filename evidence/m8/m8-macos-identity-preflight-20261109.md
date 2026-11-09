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

Local Windows checks:6 platform-independent tests pass;3 POSIX process tests are
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

## Primary sources

- [Apple code-signing identity enumeration](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/).
- [Apple TN3161: file vs data-protection Keychain enumeration](https://developer.apple.com/documentation/technotes/tn3161-inside-code-signing-certificates).
- [Apple XPC peer requirement](https://developer.apple.com/documentation/xpc/xpc_connection_set_peer_code_signing_requirement(_:_:)).
- [Apple SecCodeCreateWithXPCMessage](https://developer.apple.com/documentation/security/seccodecreatewithxpcmessage(_:_:_:)).
- [Apple DTS API availability and process identity caveats](https://developer.apple.com/forums/thread/681053).
- [Python os.waitid platform availability](https://docs.python.org/3/library/os.html#os.waitid): Mac support added3.13; the system-Python launcher therefore uses documented kqueue instead.
- [Python kqueue API](https://docs.python.org/3/library/select.html#kqueue-objects).
