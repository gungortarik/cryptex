# M8 review policy and REST adapter — scoped implementation contract

**Date:** 2026-10-20  
**Authority:** ADR-0019 accepted product direction; ADR-0004 backend boundary.  
**Scope:** In-memory review and comparison plus read-only loopback REST.

## Review state

`peer_review.rs` creates an immutable pairing candidate from complete local/peer
Brain, DeviceId, full Ed25519 public key, matching existing signing-key ID and
opaque transport association. Reject invalid identities, weak keys, wrong key
IDs, different Brains, the same DeviceId or reused signing keys. Preserve the
existing case-sensitive identity comparison. A well-formed public claim is not
an authenticated principal. Display the full public key explicitly; Debug and
errors are redacted.

Each review receives 32 OS-random bytes as an in-process ticket; this is not a
new wire authentication protocol. No time/counter/random fallback exists. A
review expires at 300 seconds using monotonic `Instant`, independently of
wall-clock time. This bounds the local UI session, not security revocation.

| State / input | Result |
|---|---|
| Pending + exact ticket/current snapshot before deadline | One local receipt, then closed |
| Pending + wrong ticket | Typed refusal; no receipt |
| Pending + changed snapshot | Typed refusal and terminal close |
| Pending + Cancel | Terminal close |
| Pending + expiry / invalid monotonic order | Typed refusal and terminal close |
| Closed + any confirmation | Refuse |
| New review after restart | New ticket; no restored approval |

The complete local and remote key/identity/transport snapshot is compared, not
just a label or shortened fingerprint. Callers must load the CURRENT setup state;
echoing the reviewed snapshot does not detect changes outside the object.

`LocalReviewReceipt` is not serializable, deserializable or cloneable. It cannot
implement a trusted resolver and grants no transport/canonical capability.
No production IPC, lifecycle, canonical writer or resolver consumes it. Bilateral
key possession, durable commit and native Desktop approval wiring are future
gated work, not proved by this helper.

## Cutoff arithmetic

`LastAcceptedSequence` accepts canonical ASCII decimal strings for [0, u64::MAX].
Zero is the no-eligible-events cutoff sentinel; issued event sequences remain
strictly positive via the existing `DeviceSeq`. Reject signs, whitespace,
exponents, fractions, Unicode digits, leading zeros and overflow. No floating
point is used.

`CutoffComparison` uses exact Brain/device/full-key binding. It compares inclusive
prefix membership, never authenticity. A matching comparison says only
`WithinPrefix` or `AfterCutoff`. Signature, chain, trust, issuer authority and
retrieval safety remain independently required. Cutoffs for different bindings
cannot merge. Arithmetic tightening takes the minimum and is tested for
associativity, commutativity, idempotency and no increase, including u64 limits.
These algebraic properties do not solve authenticated policy reconciliation.
There is no production revocation authority evaluator or durable policy loader.

## Read-only transport boundary

`syncthing_rest.rs` reuses exact `reqwest =0.13.5`, already in Cargo.lock, with
default features disabled and blocking access enabled. No package version
updates; the lock adds only dependency/feature edges. Do not use handcrafted
HTTP parsing in product code. Native HTTP fixtures are test-only.

- Numeric `127.0.0.1` or `::1` and nonzero port only; no arbitrary URL or DNS.
- Explicit no-proxy, no redirects and no automatic decompression, even under
  workspace feature unification.
- Existing per-profile OS SecureStore credential only; missing/denied/corrupt
  items fail without replacement. Service is `Cryptex[-Dev]-ManagedSyncthing`,
  account is `api:<bounded-profile-id>`. No provisioning or OS secret writes.
- Sensitive API header; credential/client Debug and typed errors redact secrets,
  response bodies, endpoint and native identity. No renderer exposure.
- Fixed GET version/status endpoints; one three-second total observation budget,
  including response-body reads and both requests, not a renewed timeout per read.
- 64 KiB body limit enforced while streaming, including chunked/unknown length;
  reject oversized declared lengths before body allocation. Never `.text()` an
  unbounded response before checking its size.
- Require exact `v2.1.5` and expected native identity. Reject duplicate JSON keys,
  malformed/missing/incorrect fields, non-UTF-8, truncated responses, unauthorized
  and unexpected HTTP status codes. Upstream unrelated fields may be additive.

Expected native ID validation is syntax only; tests use explicitly synthetic
checksum-unverified identities. Ownership, real native identity/checksum/profile
validation, wrong-key negative control and private profile ACLs belong to the
future managed-process controller. A status response alone cannot prove them.
The library does not launch/attach to an instance, configure devices/folders,
enable transport, mutate trust or accept incoming memory.

## Required scope limits

No new canonical schema, domain labels, signing protocol, trust root or
production writer is added. Existing v1/v2 readers/writers, `LocalDeviceKeyResolver`
and `RetrievalSafetyState` behavior stay unchanged. Diagnostic status correctly
continues to report the full transport controller/admission as unimplemented.
M7 managed-note write-back remains disabled. Actual M8 pairing/security events,
owned transport lifecycle, incoming validation, offline convergence and native
UI remain open. Windows-first batching and the consolidated later Mac stage
are retained; no intermediate manual Mac transfer is requested.

## Primary implementation references

- Existing ADR-0004 and `tools/m0-lifecycle-compare/scripts/syncthing-pilot.mjs`
  for pinned native version/status fields and accepted backend boundary.
- [reqwest 0.13.5 blocking client](https://docs.rs/reqwest/0.13.5/reqwest/blocking/struct.ClientBuilder.html)
  for timeout, proxy and redirect controls; local pinned crate implementation
  and actual loopback fixtures verify behavior.
- [Syncthing system status](https://docs.syncthing.net/rest/system-status-get.html)
  and [system version](https://docs.syncthing.net/rest/system-version-get.html).

This contract is scoped source/design review, not an independent security audit
of the still-unimplemented trust protocol. Protocol activation needs ADR-0019 §8.
