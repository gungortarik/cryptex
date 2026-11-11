# ADR-0020 — Disabled macOS M8 owner-message adapter candidate

**Status:** ACCEPTED FOR DISABLED CANDIDATE DEVELOPMENT ONLY.
**Date:**2026-11-10.
**Owner decision:** "önerdiğin Mac entegrasyonunu başlangıçta kapalı tutarak uygulayalım."
This accepts implementation of the bounded candidate in the
[integration proposal](../proposals/m8-macos-owner-channel-integration-decision.md).
It does not accept production activation, a release signer, a minimum OS change,
or completion of ADR-0019/M8. ADR-0007 remains the ordinary product IPC decision.

## Decision

Develop a separate macOS adapter for all three M8 families:
`bootstrap.exchange.*`, `bootstrap.approval.*` and `owner.security.*`.
The eventual enabled adapter must bind every request to its actual kernel sender,
live pinned process instance, channel epoch/session and sequence. Preserve the
existing daemon domain services as the only canonical writer; the renderer keeps
only Desktop-local handles. Keep ordinary Unix IPC and Windows pipes unchanged.

Initially refuse all Mac Desktop M8 routes before any Unix connection or native
allocation, and refuse these families on daemon Unix inbound even when that
socket otherwise recognizes Desktop. No environment variable, JSON field, health
result, inherited bearer token or transport failure may enable/fall back around
that gate. A bootstrap handshake is not permission for later unbound Unix frames.

Move closed request wire types/limits into a transport-neutral module. Retain
the Windows wire format and existing signing, journal, admission and canonical
semantics. Parsing an envelope or a role/capability claim is not authentication.
The native backend may construct a one-use authenticated request only after
validating that exact message and its pinned sender; no public boolean factory.

## Preconditions before native authorization can operate

- Configured authorized product image policy, independent of an untrusted sibling.
- Verified live/disk/bundle consistency and refusal of concurrent replacement.
- Actual supervised child/parent pins and bounded native right ownership.
- Explicit payload/framing/deadline/replay contract within existing size limits.
- Fresh sessions after peer/channel loss or restart; old grants never reconnect.
- Shared authorization/admission held through work and actual bounded response.
- Native product-path tests, review, exact CI and required human acceptance.

The existing signed synthetic19-case receipt does not satisfy those product
preconditions. Unimplemented native routes remain unavailable. No permissive
shipped fixture or ad-hoc production policy is accepted.

## Alternatives and implications

Keeping Unix-only plus closed Mac owner gates remains safe but cannot establish
Mac M8 parity. Unlocking Unix after a separate handshake does not solve forwarded
descriptor/frame attribution and is rejected. Replacing all product IPC is outside
this decision. The chosen initially disabled seam supports incremental native
work without widening current authorization or changing canonical event bytes.

Mac pairing/proof/review through the legacy Unix route will now explicitly refuse
while this candidate is disabled. Existing domain protocol fixtures remain tests
of domain behavior; they do not prove an authenticated Mac product channel.
No new Mac/VM run is needed until a reviewed native product path exists.
