# M8 peer trust implementation review draft

Date: 2026-10-21. Status: **PROPOSED, incomplete, NOT ACCEPTED**.

This is a review input for ADR-0019 section 8. It grants no authority and is
not an implementation approval, cryptographic review or native test result.
The accepted Master Spec, ADR-0018 and ADR-0019 take precedence. Existing
Windows provisioning/session results remain separate from pairing coverage.

## 1. Concrete boundaries

- Only the daemon may commit canonical security history. Desktop requests
  owner review through authenticated, role-scoped IPC; AI, MCP and Obsidian
  receive no grant/revoke/recovery capability.
- Syncthing connectivity, M1 pairing JSON and a `LocalReviewReceipt` are not
  trust anchors. Neither a remote event nor a directory name may supply its
  own trusted verification key.
- Pairing concerns one existing Brain and two installations. It must not
  create or merge Brains, transfer private keys or invoke disaster recovery.
- Local memory creation remains offline-capable. A missing remote connection
  alone does not invalidate an already authorized local writer.
- No new canonical writer, v1 field or v2 assertion type is authorized here.

## 2. Proposed pairing state machine

| State | Required evidence | Allowed effect |
|---|---|---|
| Idle | Existing local identity and OS key verified | None |
| Candidate | Same Brain; distinct device/key; explicit native transport association | Display untrusted public claims only |
| Reviewing | Immutable daemon snapshot and fresh in-memory ticket | Display both full identities and full public-key fingerprints |
| Locally approved | Exact current snapshot matches ticket; owner confirms | Single-use local receipt only |
| Bilaterally verified | Both fresh approvals bound to the same transcript; key possession verified | Eligible for durable authorization transaction |
| Authorization durable | Reviewed event/anchor transaction verified after persistence | Eligible for explicit transport configuration |
| Transport configured | Owned private controller confirms exact association/config | Exchange only within the approved admission policy |
| Content eligible | Full inventory, trust, schema, signatures, chains and cutoffs valid | Guarded retrieval/projection may consume admitted content |

Cancel, expiry, daemon restart, identity/key/association change or failed
possession verification closes pending approval. The existing 300-second local
ticket lifetime is a UI freshness bound, not a revocation clock or remote
authorization lifetime. Restart requires a fresh review; it never recovers a
pending ticket into authority. Duplicate completion must be idempotent only
for the exact committed transaction, not a new approval.

Human comparison must use a separate trusted channel and the full Brain ID,
both Device IDs and full event public-key fingerprints. Short signing-key IDs
are lookup labels, not sufficient comparison evidence.

## 3. Transcript review requirements

The proposed authenticated transcript must bind protocol/version, Brain,
both participant roles, both installation IDs and full public keys, both
native transport identities/associations, fresh challenges from both daemons,
the complete reviewed candidate and the authorization transaction identity.
Role reversal, cross-Brain substitution and transport-association substitution
must fail. An unsigned remote claim cannot become a trusted resolver entry.

Before implementation, specify **exact bytes**, field bounds, challenge
generation/consumption, signature inputs/domain separation, message framing,
exchange channel, possession checks and replay handling. Reuse the accepted
Ed25519/hash/canonicalization libraries; do not introduce a bespoke encrypted
channel, recovery-secret exchange or ad hoc short authentication code.

No wire encoding or transcript domain is reserved by this draft. In
particular, a fresh challenge and two signatures by themselves are not a
reviewed authentication protocol. The out-of-band comparison, possession,
bilateral completion and persistence rules must be reviewed together.

## 4. Security reader and durable authority

Proposed logical records, not schema definitions:

| Record purpose | Binding that must be reviewed |
|---|---|
| Local bootstrap authorization | Existing Brain, local installation/key, explicit setup evidence and its durable anchor |
| Bilateral admission | Exact two-party binding, approved transcript, issuer authority and causal policy context |
| Retire/revoke/compromise | Exact target identity/key, inclusive last accepted sequence, authorized issuer and causal context |

A remote admission is not a transitive grant. Each installation must explicitly
approve its peer bindings; reconstructing a roster from replicated records
must not bypass that local approval. A derived roster is never the sole
canonical source. How local approval anchors survive reinstall/rebuild without
silently broadening trust is an **unresolved design gate**.

Implement the strict bounded reader and compatibility negotiation before any
writer. Define version, required/unknown/duplicate-field behavior, integer and
UUID encodings, canonical bytes, domains, signature/hash coverage, stream
placement and cross-version chain rules. Preserve frozen v1/v2 behavior and
existing golden vectors. Unsupported or unverifiable policy holds guarded
content; it is not skipped as an irrelevant memory event. Older-reader failure
must be demonstrated on actual bytes before the new writer is enabled.

## 5. Cutoff and issuer authority

For an independently authorized exact target binding, ordinary validation
still applies at `sequence <= cutoff`; greater sequences are ineligible.
Cutoff `0` retains no events. All sequence values use strict decimal `u64`
strings, including `18446744073709551615`. Wall-clock order is never authority.

A cutoff cannot be chosen from an unverified tip. Show the verified contiguous
prefix and require explicit owner selection. Cutoffs do not implement Purge or
erase already copied plaintext. Re-enrollment needs a new identity/key and
fresh approval; a stale grant cannot remove a valid restriction.

Do not apply `min(cutoff)` until issuer authorization and causal context are
verified independently. Mutually revoked issuers, delayed grants/restrictions,
missing predecessors, equivocation and policy forks require an explicit
deterministic rule or a conflict hold. The exact issuer/causal reduction is
**not selected** here; no timestamp tie-break or preferred master is permitted.

## 6. Persistence and restart review

The proposed effect order is: revalidate approval and authority, persist the
reviewed authorization transaction, verify durable state, configure transport,
validate incoming inventory, then publish a new guarded content generation.
Transport failure after durable authorization must remain visible and
retryable without producing a second grant. Failure before durable completion
must not enable transport or content. An incomplete transaction is a hold,
not permission to mint replacement identity or keys.

The implementation must supply a crash table for every write/flush/rename,
OS-store operation and transport update, including disk-full, access-denied
and process exit. Do not claim power-loss atomicity from a process-exit test.
Choose exact files and commit markers only after the storage design review.

Existing canonical code takes inventory ownership before first device-directory
creation/device ownership; guarded readers lock the inventory and existing
device streams. The new policy transaction must integrate with that order.
Before coding, enumerate all participating identity, inventory, stream,
policy, projection and transport locks and prove no inverse acquisition.
Do not hold canonical filesystem locks across network waits or owner dialogs.
Validation and visibility must use one consistent policy/inventory snapshot;
on change, retry within a bound or hold, never return partially guarded content.

## 7. Required failure matrix (not executed)

| Group | Cases | Required result |
|---|---|---|
| Binding | Wrong Brain/device/key/transport association; weak key; same installation | Refuse; no resolver/transport change |
| Approval | Cancel; expiry; restart; changed candidate; reused receipt | No new authority |
| Transcript | Replayed challenges; swapped roles; altered fields; one-sided completion | Refuse; no content |
| Compatibility | Old reader; unknown version/type; duplicate/oversized/truncated fields | Explicit incompatibility/hold; original bytes preserved |
| Persistence | Exit before/after each durable boundary; missing key/anchor; disk full | No replacement; exact replay only where specified |
| Authority | Unknown issuer; transitive grant; mutual revocation; missing/forked context | Refuse or explicit conflict hold |
| Cutoff | 0, 1, u64 max; before/at/after; stale grants; reordered restrictions | Inclusive verified-prefix rule; no widening |
| Admission | Untrusted stream; invalid signature/hash/chain; local stream overwrite | Reject/quarantine and applicable Brain-wide hold |
| Offline | Independent trusted local writes, delayed exchange, reconnect permutations | Deterministic validated union or explicit hold |
| Transport | Partial sync; foreign endpoint; wrong key; crash/restart; removed peer | No implicit trust/protected claim or unmanaged adoption |
| Privacy | Failure logs; renderer IPC; diagnostics; proposal content | No secrets/bodies; no AI security mutation capability |

Each cross-platform gate needs exact-source Windows and macOS evidence.
Native provisioning PASS cannot stand in for any pairing/authority row.

## 8. Review blockers and next implementation scope

1. Final bounded bootstrap transport/transcript and exact authentication bytes.
2. Local bootstrap/admission anchor persistence and rebuild semantics.
3. Separately versioned security schema, stream integration and golden vectors.
4. Issuer/causal policy, conflict holds and restriction reduction.
5. Transaction crash table, complete lock order and reader visibility rules.
6. Scoped security review against the accepted specifications, followed by
   explicit acceptance of the resulting addendum before trust activation.

These are unresolved requirements, not a claim that the design review is done.
Windows controller/status integration can proceed separately within the
existing no-peer/no-trust boundary. Pairing UI activation, incoming remote
admission and canonical security writes remain gated. M7 write-back stays off.
No Actions, commit, push, merge or M8 completion results from this draft.
