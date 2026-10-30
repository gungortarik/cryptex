# M8 macOS XPC peer-identity feasibility — owner-authorized spike

**Status: PRELIMINARY API FEASIBILITY; native spike NOT RUN.** Owner chose a
bounded XPC feasibility spike on 2026-10-25. This authorizes investigation and
synthetic native probes only. It does not accept ADR-0019, amend accepted
ADR-0007, change production IPC, enable trust/transport, or claim M8 complete.

## Scope and question

Determine whether documented XPC peer-code requirements can give `cryptexd` a
connection/message-bound check for the expected Cryptex Desktop executable and
whether XPC connection custody plus peer credentials can bind the launched
Desktop process instance without PID-only identity. Do not use private audit
token layouts, PID-only checks, or same-user-only authorization. Keep the
existing Unix-socket implementation unchanged during the spike.

## Primary-source facts checked

- Apple's `xpc_connection_set_peer_code_signing_requirement` requires the peer
  executable to satisfy a code-signing requirement. The XPC API collection
  exposes peer PID/EUID/EGID/audit-session getters as separate peer metadata.
- Apple DTS says the C XPC peer-code-signing requirement API was introduced in
  macOS 12; `NSXPCConnection.setCodeSigningRequirement` was introduced in
  macOS 13. The former checks messages from the peer against the requirement.
- Apple's BrowserEngineKit guide says the sender on a connection can change at
  runtime when an endpoint is forwarded, and describes checking the sender's
  code requirement for messages. This makes per-message validation material to
  the probe; accepting an endpoint alone is not enough.
- `SecCodeCreateWithXPCMessage` creates a code object from an XPC message. A
  separate `SecCodeCheckValidity` call is required to validate that code object
  against the expected designated requirement.
- Apple's `xpc_connection_get_pid` documentation returns peer PID, while Apple
  DTS warns that PID may become stale and be reused. PID is not process-instance
  proof by itself.

## Bounded native probe

Run only on a synthetic Mac test build and synthetic local XPC service. Record
OS version, SDK, API availability, signing requirement, and exact binary
identities. No canonical data, real trust, transport, or approval operation is
involved.

1. Build a minimal client/server XPC pair using the public API available on the
   product's actual minimum macOS target. Before activation/resume, set the
   expected peer code-signing requirement on both sides where supported.
2. Send a benign nonce echo from the expected signed test client. Confirm the
   peer requirement is enforced on every received message and capture only
   process/signing metadata, never memory or credentials.
3. Attempt connections/messages from an unsigned binary, a different signer,
   and a same-team binary with the wrong signing identifier. Each must be
   rejected before the handler performs a protected operation.
4. Forward the XPC endpoint to a second process and send from it. Verify the
   receiving side rechecks the actual message sender and rejects a peer that
   fails the configured requirement.
   Repeat with a second process signed with the exact same requirement. If
   XPC accepts that second instance, signer validation alone does not satisfy
   Cryptex's exact-instance gate; document the smallest capability/custody
   mechanism needed or report the design infeasible.
5. Terminate and restart the expected client while retaining/reusing an old
   endpoint/connection. Verify stale connections invalidate and a fresh
   connection performs a fresh peer check. Do not infer PID-reuse resistance
   from comparing `xpc_connection_get_pid` values.
6. Derive `SecCode` from the exact received XPC message and run
   `SecCodeCheckValidity` against the packaged Desktop requirement. Compare
   this result with the connection-level peer requirement for each test case.

## Pass criteria and limits

The spike is useful only if all negative cases fail closed and the expected
client succeeds on the actual minimum supported macOS. A code-signing
requirement proves the sender has the required signed executable identity; the
public documentation reviewed so far does not state that this alone identifies
one particular process instance among multiple valid instances of that same
binary. The test must establish whether XPC connection lifetime/custody closes
that gap for Cryptex's parent-launched daemon shape. If it cannot, report
**INFEASIBLE for the exact instance-binding requirement** and do not weaken the
invariant to PID or signer-only authorization.

The production Mac runtime currently uses `interprocess` Unix-domain sockets.
Moving it to XPC would alter accepted ADR-0007's IPC choice and needs a separate
owner-approved architecture amendment after feasibility evidence and review.
This spike itself must not modify production IPC or claim equivalent Windows
behavior. R1 digest-bound one-time approval sessions remain separate from OS
peer authentication.

## Result

**NOT RUN.** This Windows workstation has no macOS SDK/runtime, so there is no
native XPC compilation or adversarial run in this evidence. Official API
documentation supports a concrete probe, but does not by itself prove
same-process-instance binding. The next required action is to build and run the
synthetic probe on the Mac before any ADR-0007 decision.

## Sources

- Apple, [XPC connections](https://developer.apple.com/documentation/xpc/xpc-connections)
- Apple, [`xpc_connection_set_peer_code_signing_requirement`](https://developer.apple.com/documentation/xpc/xpc_connection_set_peer_code_signing_requirement(_:_:))
- Apple, [Using XPC to communicate with browser extensions](https://developer.apple.com/documentation/browserenginekit/using-xpc-to-communicate-with-browser-extensions)
- Apple, [`SecCodeCreateWithXPCMessage`](https://developer.apple.com/documentation/security/seccodecreatewithxpcmessage(_:_:_:))
- Apple, [`SecCodeCheckValidity`](https://developer.apple.com/documentation/security/seccodecheckvalidity(_:_:_:))
- Apple DTS, [Validating Signature Of XPC Process](https://developer.apple.com/forums/thread/681053)
