# M8 final protocol review — November 14

Reviewed implementation: `93406122a5ca99e22f65885b923060ed43490fbe`.
Outcome: NOT READY for unconditional full-protocol acceptance or Mac activation.
This is independent read-only agent source review, not an external human audit
or a new native execution. Existing exact-source CI/receipts retain their scopes.

## Findings

### M8-FINAL-R1 — P1: local grants bypass the local effective prefix

`services/cryptexd/src/trust_reducer.rs::derive_local_approval_roster` uses all
authenticated local peer approvals as pins/roster. `reduce_trust_history` clones
that complete roster into `trusted_devices` before resolving effective cutoffs.
It does not bind each admission to its granting event's accepted position.

The L@1→A, L@2→C, A@1 restricting L to1/hashL1, C@1-content construction in the
[decision proposal](../../docs/proposals/m8-final-protocol-acceptance-decision-20261114.md)
leaves C authorized in current source. This is source-derived, not a newly
executed signed fixture. Eligibility dependencies become circular if C then
restricts A. A naive first-pass roster or fixed-point patch is not an accepted
resolution. Decide/document the precise grant-cutoff policy before implementation.

### M8-FINAL-R2 — P2: terminal-retirement contract and evidence disagree

`build_verified_trust_history` refuses any later record after retirement;
proposed §§3–4 describe filtering known ineligible actions instead of holding
solely because current authority changed. Select one exact availability contract.
The reducer-only `self_retirement_filters_post_retirement_edge_inside_cycle`
fixture's A@3 restriction/A@4 retirement naming predecessor2 cannot be realized
as a valid signed chain under the mandatory immediate-predecessor rule.
It cannot establish full signed-history retirement behavior.

Recommendation: explicitly choose strict terminal-tail refusal, keep the existing
signed-history protection, replace that impossible breaker claim with genuine
signed gap/boundary/tail/ambiguity tests, and preserve real external SCC breakers.
The owner must knowingly accept the possible indefinite whole-Brain hold and
absence of a normal M8 override. Valid-tail filtering is a separate design choice.

### M8-FINAL-R3 — P2: consolidate the actual Mac peer mechanism

Addendum §2.4 still prescribed the stopped CLOFORK/Unix carrier and called its
replacement pending. The accepted disabled native direction is now per-message
kernel sender/process-generation/image/session evidence, not a Unix socket
unlocked by a separate handshake. Consolidate the normative candidate text and
reference selected wire3 compact retention. Preserve configured product-policy
and native acceptance requirements; do not claim the fixture enables production.

Documentation disposition in this review: the stale carrier recipe is replaced
by the reviewed candidate peer-binding requirements and a link to its historical
reassessment; R3 now explicitly references the selected wire3 retention contract.
No native behavior or activation changes. This resolves the stale-text portion
of R3, not product policy provisioning or R1/R2 acceptance.

## Other reviewed boundaries and product gaps

### M8-FINAL-R4 — authenticated staged policy conflict is a product blocker

Follow-up composition review found that `verify_post_import_policy` maps an
authenticated held union or removal of an admitted canonical device to ordinary
`ImportStagingError::Invalid`; `publish_exclusively` discards the staging session
and returns `Rejected`, and ordinary admission is reopened. An incoming valid
SCC conflict or newly invalidated grant can therefore disappear while the old
canonical content stays readable. This is source-derived, not a newly executed
incoming regression; the product publication/controller gates remain closed.

Before activation, distinguish authenticated policy holds from malformed,
unknown-key or mixed-malformed input. Hold shared ordinary admission, retain
bounded verified evidence and revalidate it after restart. A process-local latch
or recovery from old canonical alone is insufficient. Do not publish a held
union or silently repurpose a publication-ready prepared manifest. The durable
evidence/resumption contract must be concretely reviewed before a storage
mechanism is selected. Required cases are authenticated SCC conflict, grant
invalidation, restart persistence and ordinary rejection without policy authority
for malformed/unknown-key candidates. No new persistence design is selected here.

No further concrete defect was identified in the reviewed bootstrap/parser
bounds, hash/signing domains, session consumption, cancellation/replay, remote
approval metadata binding or reader rollout controls. This bounded review is not
a proof of absence of bugs or final integrated product acceptance.

Existing SCC Brain-wide hold is consistent with the accepted safe-conflict
boundary; its availability consequence still needs informed final acceptance.
The [recovery coverage map](m8-recovery-coverage-map-20261113.md) remains a valid
separation of domain evidence and integrated product work. Previously passed
schema/retention/service-recovery checks are not discarded or automatically rerun.

Mac incoming publication lacks the required platform filesystem adapter;
non-Windows `inspect_directory_chain` returns an empty vector and does not
enforce its owner-check parameter. Ordinary product image policy is unconfigured,
native owner gates remain closed, and replication diagnostics cannot infer
protection from a running transport. These remain implementation/acceptance work,
not reasons to weaken the two protocol findings.

No authority algorithm, terminal-tail policy, existing test expectation or product
gate was changed during this review. The [decision proposal](../../docs/proposals/m8-final-protocol-acceptance-decision-20261114.md)
is the concrete next action required by repository conflict/architecture rules.
