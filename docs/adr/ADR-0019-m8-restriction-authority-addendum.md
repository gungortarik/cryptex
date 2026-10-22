# ADR-0019 addendum — owner-confirmed cross-device restriction authority

**Status:** ACCEPTED product permission only; protocol activation still gated.
**Date:** 2026-10-22.
**Scope:** M8 issuer-permission choice; no writer/resolver/IPC activation.

## Owner decision

The owner selected “Evet, diğer cihazın erişimini kapatabilsin (önerilen)” in
response to the explicit choice between self-only restriction and restricting
a lost/stolen different device from a trusted installation with user approval.
This resolves that product permission choice. It does not accept an unspecified
security record, bootstrap exchange, causal reducer or cryptographic protocol.

## Accepted permission

A currently trusted installation may, through an authenticated Desktop operation
with explicit local owner confirmation, issue a restriction targeting another
device's exact Brain/DeviceId/event-key binding. Existing self-retirement remains
in scope. No installation becomes master and normal operation does not require
the offline recovery signing secret.

This is an application-trust permission, not a consequence of transport pairing,
matching ownership, health, mutable pairing JSON or an arriving event key.
Admission of a third device still requires explicit per-installation approval;
cross-device restriction authority does not create transitive admission grants.

The target/cutoff presented for confirmation must be immutable for that request.
Changing the target Brain, identity, key or retained-history boundary invalidates
confirmation. Cancel performs no new restriction. Names and wall-clock times
cannot select the security target or replace sequence-cutoff semantics.

## Honest-daemon operation boundary

1. Verify local issuer identity/key and its authority before offering the action.
2. Show the exact target binding and verified retained-history boundary; require
   explicit owner confirmation through the authenticated Desktop/daemon path.
3. Commit only using the reviewed immutable security format/transaction. No
   direct canonical writes by Desktop, CLI, MCP, plugins or AI tools.
4. Apply a received restriction only after independent issuer/signature/causal
   checks. A record asserting “owner confirmed” is not proof of a human action
   if its issuer key/device is compromised.
5. Conflicting, missing or mutually affected authority remains a content hold
   until the reviewed policy determines a safe outcome. No arrival-order or
   wall-clock tie-break, and no naive minimum over unauthenticated cutoffs.

These are permission constraints, not the missing verification algorithm.
ADR-0019 section 8 still requires that exact algorithm before activation.

## Limits that the product must state correctly

- A compromised trusted issuer/key can forge that issuer's signed actions;
  choosing this authority therefore increases its potential denial-of-service
  impact. Explicit UI confirmation alone cannot cryptographically constrain an
  already-compromised device. Compromised-issuer/causal-conflict cases require
  the scoped security review; this addendum does not claim to solve them.
- A disconnected lost device is not remotely erased. Restriction does not
  retract previously copied plaintext or implement M10 Purge.
- Local durable action and propagation to other installations are different
  states. Offline/unconfirmed propagation must not be shown as global success.
- Retire/revoke/compromise cannot widen a previously valid retained prefix or
  reauthorize the same restricted identity/key. Re-enrollment requires fresh
  identity/key and explicit approval under the complete protocol.
- Loss of all trusted ordinary devices still uses the M11 recovery-root flow.

## Remaining gates

The product permission part of TRUST-04 is selected; causal issuer validation,
exact bootstrap/consent, durable anchors/rebuild, strict security bytes/domains/
golden vectors, reader-first compatibility and transaction/lock/crash behavior
remain open. No current in-memory receipt or helper is promoted into authority.
No trust-changing code is authorized until the complete addendum is reviewed.
M8 remains open; M7 write-back stays disabled.
