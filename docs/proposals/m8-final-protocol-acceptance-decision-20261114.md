# M8 final protocol acceptance decision — November 14

Status: OWNER-SELECTED RULES; implementation and final acceptance pending.
On November14 the owner answered "Önerilen kuralları uygula" after the explicit
cutoff/strict-retirement choice, including possible indefinite whole-Brain hold
and absence of a routine M8 override. This selects A plus strict-terminal B for
implementation/review. It is not final protocol acceptance or product activation.
Reviewed baseline: `93406122a5ca99e22f65885b923060ed43490fbe`.

The user requested final protocol acceptance and Mac product approval/incoming/
protection-status integration. The independent full closeout review found two
policy/conformance decisions that must precede activation. This proposal makes
the choices concrete; implementation must not invent an authority algorithm.

## A. Local admission anchors must obey the local cutoff

Counterexample reconstructed from current source (not a natively executed new
test):

| Signed record | Meaning |
|---|---|
| L@1 | Genuine four-message local approval of A |
| L@2, predecessor hashL1 | Genuine four-message local approval of C, fresh transaction |
| A@1 | A restricts exact L identity to cutoff1/hashL1 |
| C@1 | Genuine signed content from C |

All identities/transcripts/links are genuine in this construction. A is directly
approved by L@1; the restriction graph is simply A→L, without an SCC. Current
`derive_local_approval_roster` takes both local anchors before reduction, and
`reduce_trust_history` keeps A+C trusted while resolving cutoff(L)=1. C's
content can therefore pass although its only admission is L@2, beyond L's cutoff.
The full proposal says the effective prefix caps ordinary content and policy
actions, but does not specify the grant/cutoff dependency algorithm.

Recommended policy text for explicit acceptance:

> Peer-admission anchors are policy actions and are not exempt from their
> author's effective cutoff. An anchor above that cutoff cannot add admission,
> transport, content or restriction-issuer authority. Verification of a known
> key/signature is distinct from permission to act. If anchor eligibility
> depends directly or indirectly on authority introduced by that anchor and
> cannot be independently resolved, that unresolved eligibility dependency
> holds the whole Brain. Mere known-key verification or a provably eligible
> earlier equivalent grant is not itself an unresolved dependency. A first-pass
> roster, arrival ordering or an unreviewed fixed point cannot resolve it.

After selection, the implementation design must distinguish verification pins
from eligible admission grants, retain actual granting event positions and
define the combined authority dependencies. No iterative pruning or expanded
graph algorithm is accepted merely by this proposal. Independent review and
genuine signed regression cases must challenge the chosen implementation.

Required cases include the acyclic counterexample, a still-eligible earlier
equivalent grant, a grant made ineligible only by authority it introduced,
indirect grant/restriction cycles, known signature pins without authority,
arrival-order permutations and unchanged canonical bytes. Unresolved authority
must never admit content or start transport. Do not reinterpret local grants
as permanently cutoff-exempt to preserve current code.

## B. Select strict terminal retirement explicitly

Current signed-history code refuses every record after a valid self-retirement.
Addendum §§3–4 instead describe later known actions as ineligible/filterable and
say changed current authority alone does not cause a hold. These are different
availability contracts. The existing reducer-only retirement-cycle fixture is
also unrealizable as a signed stream: A@3 is a restriction followed by A@4
retirement naming predecessor2; the mandatory prior verified record is A@3.

Recommended minimal safe closeout policy:

> Self-retirement seals that identity's chain. The control event binds the
> immediately preceding verified record/hash, including valid allocation gaps.
> Any later authenticated record is an invalid terminal tail and causes a typed
> whole-Brain hold. It is not automatically filtered. Ordinary valid
> cross-restriction prefix filtering remains distinct. Forks, ambiguous
> retirements and inconsistent predecessor bindings also hold.

This preserves current strict signed-history behavior. Explicitly amend the
proposal before relying on it as the final acceptance contract. Replace the
unrealizable retirement-SCC breaker claim with genuine signed terminal-boundary,
gap, later-content/later-policy refusal and ambiguity cases. Retain real
external-cutoff SCC breakers and unresolved-cycle holds.

Availability consequence: a known retired key producing later signed history
can cause potentially indefinite whole-Brain unavailability when that history
is admitted to the complete verification input. M8 has no routine override or
automatic cycle/tail repair. Never delete canonical events to clear the hold.
Incoming data still requires validation before publication. Choosing these
rules does not authorize publishing an invalid tail.

Alternative: define valid-tail filtering with a separate exact authority/tail
policy and genuine signed vectors. That is additional protocol design and must
be reviewed before changing the current rejection behavior. It is not silently
selected by a request to finish M8.

The existing SCC safe-conflict policy similarly permits potentially indefinite
whole-Brain holds and no normal M8 override. Final owner acceptance must include
that availability consequence; a passing test or agent review cannot supply it.

## Mac integration after the policy decisions

The stopped CLOFORK/Unix carrier is historical. The accepted disabled-candidate
direction is per-message kernel audit-sender evidence, retained process
generation/image/role, epoch/sequence/session binding, protected frontend bytes,
and no legacy Unix fallback for M8 owner families. Local development certificate
use and macOS26+ were already selected; no paid distribution service is requested.

Concrete implementation work remains:

1. Bind ordinary supported approval/session consumers to the authenticated
   native route and independently pinned signed inventory. A fixture signer or
   adjacent unverified executable cannot configure product trust. Enforce the
   selected OS floor and refuse missing/unavailable policy before startup.
2. Supply Mac incoming-publication filesystem adapters. Current non-Windows
   directory inspection returns no pinned handles and does not enforce the
   requested owner check; removing a Windows cfg is unsafe. Define and review
   descriptor-relative no-follow containment, owner/private-mode checks,
   same-volume no-replace publication, durability/recovery and rename/replacement
   behavior before connecting canonical writes.
3. Connect native controller and publication/rebuild outcomes to truthful
   replication status. Current metadata inspection always says not configured,
   and `transport.status` alone cannot claim protection or peer durability.
   Pending approval, local durable approval, transport readiness, validated
   incoming state, held recovery and unknown peer durability remain distinct.

Product integration and its native/human acceptance are still required after
protocol decisions. Default gates remain closed; no new public API, master
device, transitive grant, custom crypto or M11 override is authorized.

## Decision and execution order

The owner selected A plus strict-terminal B, including the stated availability
consequence. Selection is
permission to amend/design/implement those exact rules, not immediate full
protocol acceptance or product activation. Then implement genuine signed
regressions, independently review the authority algorithm, run exact-source
native CI and present the final consolidated protocol for acceptance. Complete
Mac product wiring and integrated acceptance separately.

Repository authority: `AGENTS.md` says "If requirements conflict, stop
implementation and report the conflict. Do not invent a new architecture."
It also requires asking before an ambiguous architecture/integration direction
changes. This concrete selection now authorizes implementation of the selected
rules without requesting the same choice again. Completed unrelated tests are
not reopened; final activation still has its separate acceptance gate.
