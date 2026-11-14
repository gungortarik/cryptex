# M8 admission-cutoff and strict-terminal correction — November 14

Owner-selected rules are recorded in the
[decision](../../docs/proposals/m8-final-protocol-acceptance-decision-20261114.md).
Verified implementation: `22766528093889363abe38905911b058af0836be`. This record
is not full protocol acceptance or activation of ordinary product routes.

## Authority correction

Every exact local peer binding retains its earliest authenticated granting
position. All local transcripts can initially supply provisional signature pins
to authenticate the complete input. Only eligibility-proved bindings escape the
builder as directly approved peers or in the returned key resolver.

The [reviewed conservative interval design](../../docs/proposals/m8-admission-cutoff-resolution-20261114.md)
proves definitely/possibly eligible restrictions while narrowing finite cutoff
bounds. Unresolved admission dependencies return a distinct typed whole-Brain
hold. The algorithm never assumes a peer roster and repeatedly prunes/re-admits
it to select a fixed point. It may safely hold even where a unique consistent
assignment exists; no completeness guarantee is claimed.

After eligibility is proved, the existing SCC procedure still decides restriction
authority. Before exposing Ready, its actual local cutoff must classify every
grant exactly as the proved roster and all raw authority cutoffs must fit their
proved intervals. Disagreement holds rather than selecting a new roster.
Non-admitted content cutoffs become0 only after those checks. Canonical scanners
retain their existing untrusted-inventory hold. Staged/post-import validation
uses the filtered bindings and reruns complete union eligibility; a provisional
key is never an admission or transport credential.

## Selected strict retirement

Any later authenticated record after a valid self-retirement returns typed
TerminalRetirementTail. Retirement still binds the actual preceding record/hash,
including sequence gaps. No signature/chain/boundary or fork check is weakened.
The impossible reducer-only retirement SCC-breaker fixture is replaced by a real
signed cycle and a falsely named retirement predecessor that must be refused.
Existing real external-cutoff SCC breakers and unresolved-cycle tests remain.

The preselection zero-cutoff test also needed a policy-correct expectation:
peer→local cutoff0 invalidates that peer's sole local@1 grant, so it now requires
UnresolvedAdmissionDependency. Wrong target keys still require InvalidPolicyHistory;
independent local→peer cutoff0 still resolves Ready with that exact cutoff.
Independent review confirmed this implements the explicit owner selection rather
than weakening a failing test.

## New native regressions

Nine new tests cover:

* genuine signed L@1→A/L@2→C/A→L1 history, forbidden C content and no exported C pin;
* a valid earlier equivalent grant despite a later cut-off duplicate;
* direct/indirect and conservative unique-model unresolved dependencies;
* an ineligible issuer unable to restrict another still-eligible peer;
* actual signed zero/gap retirement and content/policy/second-retirement tails;
* the genuine signed invalid-predecessor/SCC counterexample;
* cancellation in the interval proof;
* cancellation at every observed boundary through the real signed-history entry;
* an independent finite reference across64 arithmetic policy graphs, enumerating
  every cutoff assignment and checking all consistent models for every Ready
  result. Arithmetic models are not claimed as signed-history fixtures.

Signed tests preserve input bytes and exercise reversed record arrival order.
They use production bootstrap/signature APIs with public synthetic keys; they
are genuine signed regressions, not a replacement for the separately independent
wire-golden corpus. Existing v1/v2 artifacts and golden bytes are unchanged.

## Review, CI and limits

Independent read-only source review found no remaining actionable source issue
in the correction, including raw-SCC composition and filtered import/transport
callers. Its requested real-entry cancellation coverage was added. This is agent
source review, not an external human audit.

Native schema-format run43645853627 stopped on formatting at772f8e1; exact native
rustfmt output was applied without bypassing local Application Control4551.
Full run43646175067 on67f8336 passed all9 new Mac tests but failed the old
self-invalidating zero-cutoff expectation. That expectation was updated only as
described above under the selected policy; remaining Windows work was cancelled
as superseded, preserving the failed Mac log.

Exact-source [CI43646684219](https://github.com/gungortarik/cryptex/actions/runs/43646684219)
passed Mac/Windows readiness and Windows cross-account IPC ACL. Daemon library:
Mac546passed/7ignored, Windows667passed/5ignored, zero failures. All nine new
tests and the existing exact-key zero-cutoff and terminal-boundary tests passed
both platforms. Required workspace formatting/lint, release Desktop and Node
checks passed. Exact jobs, source-file hashes and limits are recorded in the
[receipt](m8-admission-terminal-native.json). Separate manual native
Desktop/identity/installer jobs and historical-reader generation were not rerun;
ordinary Mac readiness does not execute Windows-only incoming publication.

This closes neither R4 nor product activation. Fully authenticated staged policy
conflicts currently become clean rejection and can reopen old canonical content.
Before enabling that path, hold shared ordinary admission, retain bounded verified
evidence and revalidate it across restart, while malformed/unknown-key input
remains ordinary rejection. A volatile latch or a held canonical publication
is insufficient; no new persistence mechanism is selected by this correction.
Mac native filesystem adapters, configured image/OS policy, supported approval/
incoming/protection-status wiring and final integrated acceptance remain open.
