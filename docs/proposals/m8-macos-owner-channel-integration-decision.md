# M8 Mac owner-channel integration: approved disabled candidate

Status: **OWNER APPROVED FOR INITIALLY DISABLED CANDIDATE DEVELOPMENT**.
Date:2026-11-10. The owner explicitly authorized this bounded candidate in chat.
[ADR-0020](../adr/ADR-0020-macos-m8-owner-channel-candidate.md) records that scope.
Production activation, release identity and support-floor decisions remain open.
[ADR-0007](../adr/ADR-0007-local-ipc-implementation-addendum.md), the Master Spec
and accepted platform/runtime contracts remain authoritative.

## Evidence and exact boundary

The [personal-Mac signed bootstrap](../../evidence/m8/m8-macos-signed-bootstrap-owner-receipt-20261110.md)
passed19/19 on source8e8fabb. It demonstrates this isolated public-message
candidate's child/parent binding and specified hostile endpoint/right refusals.
It does not attribute subsequent Unix frames to that same process. The existing
Unix adapter's socket-owner sample is not a per-message kernel sender observation.
Forwarding an already-connected descriptor is a distinct boundary.

The product currently refuses Mac owner-security operations and durable pairing
completion with `platform_peer_identity_unavailable`. Leave that rule in place.
Health/reconnect success and a prior Mach bootstrap must never unlock arbitrary
later Unix requests, persistent bearer tokens or UI role claims.

## Recommended decision: bounded Mac integration candidate

Authorize a separate design/implementation candidate for Mac **M8 pairing and
owner-security requests** over a public kernel-sender-attributed message adapter.
Keep ordinary Unix IPC and the Windows pipe adapter. Preserve shared daemon
authorization, journal, runtime admission and canonical-writer services. Do not
introduce another writer, shell access, custom cryptography, permanent launch
registration or a new product support floor.

Scope all three M8 request families together: `bootstrap.exchange.*`,
`bootstrap.approval.*` and `owner.security.*`. Both the displayed prepare snapshot
and confirm/cancel/complete requests must travel through the same authenticated
adapter/session. Do not mint a Mach grant and redeem it over an unbound Unix FD.
Unimplemented privileged routes remain refused rather than falling back to Unix.
Other feature APIs are outside this proposed transport change.

Before enabling any operation, the candidate must establish:

1. **Product image policy:** define the authorized product signing identity and
   sealed bundle/counterpart policy. The owner's temporary development certificate
   and same-organization experiment are not product publisher acceptance. Do not
   derive release authority solely from an untrusted sibling file.
2. **Stable image observation:** resolve live/disk identity and concurrent bundle
   replacement behavior. Refuse an inconsistent observation; do not silently
   accept whichever image happens to occupy the path later.
3. **Actual supervised instances:** bind Desktop's captured child and daemon's
   actual parent in both directions, retaining live generation pins throughout
   privileged exchanges. Attaching to an existing daemon for health must not
   confer this new owner authority or spawn a competing canonical writer.
4. **Fresh sessions:** each daemon/Desktop restart, loss of either pinned peer,
   changed image/generation or invalid channel ends outstanding approval grants.
   Reconnect performs fresh authentication and requires a newly displayed review.
   Durable transaction state can resume; ephemeral capabilities cannot.
5. **Bounded adapter contract:** specify framing/maximum payload, sequence,
   deadlines, cancellation and resource ownership before native implementation.
   Existing transcript/request limits remain upper bounds. Imported fields cannot
   choose identity policy/process pins or bypass authenticated, allowlisted
   dispatch and validated operation-specific filesystem boundaries.
6. **No renderer authority:** the webview keeps only Desktop-local handles and
   review fields; signing capabilities and native rights remain outside it.

This approval authorizes development behind the existing closed product
gate, not immediate deployment or removal of the gate. Record the resulting
transport decision through the ADR process before calling it an accepted product
mechanism. A new release identity or support-floor choice needs its own concrete
evidence/decision; neither is silently chosen here.

## Acceptance before activation

Run actual Desktop/daemon native tests for first launch, existing-instance
handling, crash/restart, old live channel/grant refusal, reconnection, expiry,
cancel, single use and wrong-role/forwarded-right/changed-image refusals. Verify
canonical hashes and journal outcomes on interrupted confirmations, with no second
writer or duplicate append. Include bundle-change faults and bounded cleanup.
Test the product adapter, not only the previously passed C experiment.

Require reviewed source, exact-source cross-platform CI and a single pinned Mac
owner package only when relevant paths are executable. Preserve the already
completed signing/API/Windows transfer evidence. Final activation also remains
subject to protocol/platform/security and required human diff acceptance.

## Alternative

Retain Unix-only product IPC and the existing closed Mac M8 mutation gates while
investigating another public mechanism that meets the same per-request guarantees.
The current code cannot claim Mac owner-operation parity under that alternative.
Do not reinterpret a health/restart pass as solving the missing authorization.

## Why an explicit choice is required

[AGENTS.md](../../AGENTS.md) says: "If a requested change in tool roles, workflow,
architecture, branch strategy, milestone scope, or integration strategy is
ambiguous, ask the user before changing direction."
The owner's earlier approval covered a product-separated experiment. The owner
has now separately approved the recommended bounded integration candidate,
initially disabled. No repeat permission is needed for that development scope.
Activation or an expanded architecture/release-policy decision remains separate.
