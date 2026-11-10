# M8 raw Mach actual-sender instance experiment

Status: eight synthetic native CI cases and eight personal-Mac cases PASS.
XPC exact-message binding and production integration remain NOT PROVEN/not implemented.
This is a bounded mechanism experiment under the owner-requested process-binding
investigation, not acceptance of a new transport or an amendment to ADR-0007.

## Why another mechanism was tested

The signed personal-Mac XPC receipt showed that a same-requirement second process
can use a forwarded endpoint. Apple's SecCode equality implementation compares
code-directory hashes, not process instances. kSecCodeInfoUnique also describes
static code. Retaining/equating SecCode cannot fix this. XPC's public PID getter
explicitly warns about recycling and notification lag; a retained Mach task-name
right pins an expected task but does not establish the sender of an exact XPC
dictionary. Do not substitute connection PID metadata for actual message identity.

The compile-only public SDK audit on source d1a2f8b, Mac
[CI42896438505](https://github.com/gungortarik/cryptex/actions/runs/42896438505),
passed its public-header baseline on macOS26.7.2 arm64/SDK26.5. Public includes
compiled audit_token_to_pid, audit_token_to_pidversion, LOCAL_PEERTOKEN,
TASK_AUDIT_TOKEN, task-name/liveness and joint PID/audit SecCode lookup.
xpc_dictionary_get_audit_token, xpc_connection_get_audit_token and
PROC_PIDUNIQIDENTIFIERINFO were undeclared in those public includes. This is SDK
surface evidence, not a complete absence claim for every OS or a native auth pass.
No private header, opaque token-slot parsing, EndpointSecurity entitlement,
debugger/task-control right or new certificate was used.

## Mechanism and scope

A disposable native receiver creates a private Mach receive right and temporarily
registers its send right only in its own task's inherited-port list. It explicitly
spawns two workers from the exact same signed binary. Each worker obtains the
actual inherited send right using mach_ports_lookup, not a numeric name passed
across namespaces. The receiver restores its prior registration list after spawn.
There is no bootstrap/launchd service or persistent registration.

Each worker sends its TASK_AUDIT_TOKEN opaquely over a one-shot inherited pipe.
Enrollment cross-checks the PID returned by posix_spawn with that token through
public SecCodeCopyGuestWithAttributes and strict code validity. A forged valid
token/PID pair must fail; the pipe alone is not authentication. A retained
task-name right and a second kernel code check close enrollment replacement
ambiguity. The exact enrollment pipe closes after fixed-size input and EOF;
only the synthetic control pipe is deliberately retained for the exec test.

For every received message, successful mach_msg with the public audit-trailer
request must precede parsing. Fixed non-complex message size/ID and absent
reply/voucher rights are required. Checked aligned bounds, exact trailer type
and complete audit-trailer size locate only the kernel trailer. A payload-supplied
token is deliberately ignored. Apple's audit-token getters compare actual sender
PID and process generation to the enrolled identity, with a retained live task
right and fresh independent expected-token/actual-trailer kernel code validation.
The trailer is send-time identity; current liveness validation is separate.
No audit token bytes, real certificate selector or private key enter the receipt.

Rejected senders cannot advance the accepted sequence counter. Repeated accepted
sequence values reject. This is a synthetic replay control, not implementation of
the full digest-bound owner confirmation/session protocol. The receiver pins its
spawned worker here; Desktop-parent -> daemon-child enrollment is not yet tested.

## Native CI result

Source `85ef4b13099ebf59b835ff466b897fa6bfe7429d` passed Mac
[CI42898425263](https://github.com/gungortarik/cryptex/actions/runs/42898425263),
job130117622560,24seconds, on macOS26.7.2 arm64/SDK26.5. C source SHA256:
`9be82cace4229cedde3af0f5cd95e08b8951d4ea8b1a381a67df383070dc4ea6`.
Warnings-denied compile and link with Security/CoreFoundation/libbsm succeeded.
CI used ad-hoc signing plus an exact identifier/codehash requirement; no owner key.

| Case | Observed result |
| --- | --- |
| Forged enrollment token/PID pair | Kernel validation rejected |
| Expected sender | Accepted, matching PID and generation |
| Same binary, shared actual send right, forged expected-token payload | Rejected; actual sender PID differed |
| Same sender replay | Sequence replay rejected |
| Expected sender after negative | Accepted; negative did not consume valid sequence |
| Same PID after exec of the same binary | Rejected; PID stayed equal, kernel generation changed |
| Fresh explicit enrollment of the other execution | Accepted on the same receive endpoint |
| Queued message received after sender exit and waitpid/reap | Rejected by live-task/code validation despite matching send-time PID/generation |

All eight exact cases passed. Actual PID recycling was not forced; same-PID exec
tests a distinct generation transition and is not relabeled as a PID-reuse run.
The successful same-process/cross-process XPC harness still reports its original
inconclusive negatives and signer-only insufficiency. Ordinary product readiness,
Windows ACL and installer jobs were skipped; retain earlier product CI separately.

Runs42897971790/source21a5ad3 and42898256798/source204ea8c stopped at compilation:
modern SDK deprecates mach_port_destroy as inherently unsafe. The probe now uses
checked explicit receive-right mod_refs and send/dead-name deallocation. No warning
or negative assertion was disabled. CI-only fixed-source compiler diagnostics
were added; owner receipts keep fixed diagnostics. No identity probe was delivered
before the native corrections passed.

Independent source-only review of85ef4b1 found no actionable correctness/privacy/
false-pass issue in the scoped trailer, enrollment, rights, generation, replay,
receipt or package paths. Four message/package tests, three SDK-audit tests,
eleven XPC-harness tests and eleven applicable prerequisite tests passed on Mac;
one non-Mac refusal test skipped. Process groups/deadlines/output caps use the
previously tested bounded runner. Successful native return follows reaping both
workers and releasing the receiver/retained task rights; failure remains a failed
receipt and the outer owned group is stopped before leader reap.

## Outgoing owner package and next gate

`C:\Users\Tarik\Downloads\cryptex-mac-surec-85ef4b13.zip`,20233bytes,
SHA256 `edafdd90908ef0820f2f69ebe8f7f2b0349b0eff969f0cd273dbf8db1fa49af6`.
Seven closed members, committed source binding, payload hashes, UTF8/LF and
executable launcher mode were verified. The package signs one temporary binary
with the personal Mac's single existing certificate, never exports/imports a key,
and runs only the synthetic message experiment outside Git in private TEMP.

The personal-certificate receipt is now recorded below; do not rerun this package.
Production integration, actual parent custody, receiver restart/session semantics, minimum
product OS and protocol/security acceptance remain open. A new Mach production
channel would require a separately reviewed/accepted ADR-0007 amendment. Do not
migrate IPC, enable owner operations/trust/transport or close M8 from this result.

## Personal MacBook receipt verified (2026-11-10)

Owner-supplied `MAC-SUREC-SONUC.zip`, SHA256
`14df58e67aff07697ba9a52d5245b41c494eac34a5011c1437356b2f6e98fddf`,
contains only regular result.json,2353bytes, collected at2026-11-10T20:51:15Z.
Source85ef4b13099ebf59b835ff466b897fa6bfe7429d and native C source SHA256
`9be82cace4229cedde3af0f5cd95e08b8951d4ea8b1a381a67df383070dc4ea6`
match the committed candidate. Duplicate-key/type checks, exact eight-case
inventory/results, digest formats and explicit scope limits passed validation.

The receipt reports macOS27.2/SDK27.2 arm64, native_compile=pass,
signing_mode=single_existing_certificate,
private_key_use=synthetic_fixture_signing_succeeded and ci_only=false.
Binary SHA256:
`6f8fb284a84f758821f69c6751c5c048adb390c014705779288f04beb99044e1`.
All eight cases in the table above passed on this personal Mac as well:
the actual same-binary second sender was rejected despite sharing the send right
and supplying the expected token in its payload; same-PID exec showed a different
kernel generation and rejected; the queued post-exit message rejected despite
matching its original send-time PID/generation. Expected controls, explicit fresh
enrollment, forged-enrollment rejection and sequence replay checks all passed.
Runner_exit0 is consistent with that complete synthetic inventory.

This establishes the tested raw Mach sender-instance mechanism on this host,
not production custody. The receiver enrolled its spawned worker; it did not run
the real Desktop-parent/daemon-child launch, restart or owner-confirmation flow.
The receipt explicitly retains xpc_exact_message_sender_binding=not_proven,
production_integration=not_implemented, product_minimum_os=unconfirmed,
actual_pid_reuse=not_forced, production_changed=false and m8_complete=false.
This remains owner-supplied local evidence, not remote attestation. Raw archives,
runtime data, credentials and token bytes are not copied into Git.

Next: review actual Desktop-parent custody and the production adapter boundary.
ADR-0007's existing Unix-socket transport remains accepted; the newly confirmed
public audit-generation getter also warrants evaluating that existing route before
recommending a different transport. This receipt does not select a Mach production
channel. Any such change requires its own concrete reviewed ADR amendment and
explicit owner acceptance. No further owner package is requested for this receipt
review; do not repeat successful API/signing/instance/Windows pairing tests.

## Primary references

- [Apple Mach object/port-right model](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/KernelProgramming/Mach/Mach.html).
- [Apple public Mach message/trailer types](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/mach/message.h).
- [Kernel audit trailer uses the actual sending task](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/ipc/ipc_kmsg.c).
- [Kernel process exec-generation handling](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_exec.c).
- [Kernel task-name rights and joint audit identity lookup](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_proc.c).
- [Public SecCode API](https://github.com/apple-oss-distributions/Security/blob/main/OSX/libsecurity_codesigning/lib/SecCode.h).
- [SecCode equality compares code hashes](https://github.com/apple-oss-distributions/Security/blob/main/OSX/libsecurity_codesigning/lib/Code.cpp).
- [Apple XPC peer-PID limitations](https://developer.apple.com/documentation/xpc/xpc_connection_get_pid(_:)).
