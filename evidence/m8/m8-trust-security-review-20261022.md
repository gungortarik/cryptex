# M8 trust activation readiness review

Date: 2026-10-22. Scope: accepted requirements, existing source boundaries and
ADR-0019 review drafts. This is an agent source/document review, not independent
cryptographic review, runtime testing or acceptance of a new protocol.

## Disposition

**Trust activation NOT READY.** Windows native transport/worker/startup evidence
does not satisfy ADR-0019 section 8. No accepted-spec conflict was found in
keeping the current local-only resolver and transport protected=false.
The blockers below are missing design/verification, not claims that currently
disabled trust code is an exploitable deployed feature.

## Current boundaries checked

| Source | Observed boundary |
|---|---|
| `services/cryptexd/src/peer_review.rs` | Public claims validate shape/key binding; local receipt explicitly proves only exact in-memory review, not bilateral authentication/durable authority |
| `services/cryptexd/src/local_device_identity.rs` | Resolver remains bound to enrolled local identity/key; no remote trust expansion |
| `crates/events/src/versioned_reader.rs` | v1 frozen path, v2 dispatched reader, future schema classified unverifiable; resolver keys external to event claims |
| `services/cryptexd/src/canonical_writer.rs` | Unsupported/unverifiable records have preservation/blocking regressions; no security writer selected |
| `services/cryptexd/src/ipc.rs` | New status response remains protected=false; authenticated status is not a trust operation |

These are source observations. Existing regressions were not rerun in this
documentation-only step, and no future schema golden vectors were fabricated.

## Activation blockers

| ID | Finding | Required closure evidence |
|---|---|---|
| TRUST-01 | No exact bootstrap transcript/transport/possession protocol | Reviewed bounded bytes and exchange, replay/cancel/restart/substitution cases |
| TRUST-02 | No accepted durable local approval/bootstrap anchor and rebuild contract | Exact anchor records and key-loss/reinstall/existing-Brain-join behavior |
| TRUST-03 | No accepted security schema/domains/stream integration | Reader-first code, exact signed golden vectors and real older-reader failure |
| TRUST-04 | Issuer permission and causal reducer are unspecified | Explicit who-can-restrict-whom rule; missing/forked/mutual-revocation outcomes independent of arrival order |
| TRUST-05 | No complete authorization transaction/lock/crash contract | Durable boundary crash matrix, exact idempotency and visibility rules |

Do not close TRUST-01 with a full fingerprint alone: comparison, possession,
freshness and consent persistence must all bind the same candidate. Do not
close TRUST-04 with `min(cutoff)` over signed records: signature validity and
issuer authority are different checks. Do not close TRUST-02 by promoting M1
pairing JSON, incoming keys or a derived roster into a trust root.

## Concrete continuation

`m8-authority-anchor-review-proposal-20261022.md` supplies a focused proposed
anchor/rebuild contract and restriction conflict cases. It is NOT ACCEPTED and
explicitly identifies the remaining issuer-permission choice. Incorporate its
resolved decisions into the complete ADR-0019 addendum, specify exact bootstrap
and event bytes, then perform scoped review before writer/resolver activation.

The accepted product direction need not be re-approved. It also must not be
misrepresented as acceptance of these unresolved protocol choices. Preserve
Windows-first staging, later consolidated Mac, M7 write-back off and no Actions.
