# M8 local-admission cutoff resolution

Status: independently source-reviewed conservative implementation, native-verified
through2276652 in Mac/Windows CI43646684219. The owner
selected the [cutoff and strict-retirement rules](m8-final-protocol-acceptance-decision-20261114.md).
This design implements those rules inside the existing daemon trust builder;
it creates no new authority source, wire format or product activation.

## Separate signature evidence from admission

Authenticate the local stream first and verify every local approval transcript.
Retain its exact peer identity binding and earliest granting sequence. Repeated
equivalent grants use their earliest position; conflicting identity/transport
bindings still refuse. These provisional pins may verify signatures in the
complete candidate history. They do not yet authorize content, transport or
restrictions. Unknown/unresolvable keys, malformed history and retirement tails
continue to fail closed before eligibility analysis.

## Conservative interval proof

For each known device D, maintain a sound interval [lo(D), hi(D)] for its
effective cutoff. Start lo=0 and hi=min(u64::MAX, verified retirement cap).
The local enrolled identity is an implicit issuer; peer P's admission is
definite when earliest_grant(P)<=lo(local), impossible when that sequence is
greater than hi(local), and otherwise unresolved.

In each simultaneous round, a restriction can possibly contribute only if its
issuer can be admitted and its event sequence is <=hi(issuer). It definitely
contributes only if its issuer is definitely admitted and event sequence is
<=lo(issuer). Recompute:

* next_lo(target) = min(retirement cap, all possibly contributing cutoffs).
* next_hi(target) = min(retirement cap, all definitely contributing cutoffs).

Taking all possible restrictions is conservative for the minimum cutoff;
taking only definite restrictions bounds it from above. Potential restrictions
can only disappear and definite ones can only appear: lo rises and hi falls.
Each bound belongs to the finite set of actual cutoff values/retirement caps,
so strict changes terminate. Check cancellation each round and edge. Do not use
an iteration limit that silently chooses a result or alters the protocol cap.

On convergence, classify each grant using the proven local bounds. If any
admission remains unresolved, return typed UnresolvedAdmissionDependency for the
whole Brain. Do not choose a roster by arrival order or an assumed stable
assignment. This is monotone proof of definite/possible facts, not selection
among potentially oscillating roster fixed points. The source-bound
[correction receipt](../../evidence/m8/m8-admission-terminal-native.json) records
completed independent source review and native regression verification. This is
not a formal proof over all possible histories or full product acceptance.

When all admissions are decided, retain only eligible peer bindings and the
eligible direct roster. Run the existing restriction SCC procedure on that roster.
Its unresolved-cycle hold is mandatory even if these numeric intervals happened
to coincide; interval equality is not proof of SCC authority. Before exposing a
Ready result, verify its raw local cutoff classifies every grant exactly as the
decided roster and every raw authority cutoff lies inside its proven interval.
Any disagreement holds; never rerun with a newly assumed roster. Non-admitted
devices receive effective content cutoff0. The canonical scanner still refuses
an inventory outside the admitted set; a provisional signature pin cannot bypass
that gate or authorize an incoming batch.

The procedure is sound and deliberately incomplete. Example: local grants A@1
and B@2; A restricts local to1 while B restricts local to0. A unique consistent
assignment may exist, but the conservative bounds can still leave admission
unresolved. Return the selected typed hold rather than choosing that assignment.
This availability limit is exercised explicitly; no promise of Ready for every
unique mathematical model is made.

## Required proof cases

Genuine four-message signed anchors for the L/A/C counterexample; an earlier
equivalent C grant that remains eligible; direct and indirect grant/restriction
dependencies; unresolved restriction cycles; a provably ineligible grant whose
policy cannot affect others; full-key pinning without admission; sorted/shuffled
unions; zero/u64::MAX/gaps; cancellation; immutable source bytes. Preserve current
unresolved SCC behavior and all existing wrong-key/transcript/tail refusals.

Strict retirement remains a prior full-history check: a retirement binds the
actual immediately preceding event, and any later authenticated record holds.
Replace the impossible reducer-only retirement breaker with signed terminal
boundary/gap/content-tail/policy-tail/ambiguity cases under that selected rule.
