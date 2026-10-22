# M8 independent protocol security re-review

**Date:** 2026-10-22
**Reviewer:** Cursor independent read-only re-review (follow-up to
`evidence/m8/m8-independent-security-review-20261022.md`)
**Candidate:** `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md` at commit
`5723cf1` (byte-identical on descendant `eb52fd2` at review time)
**Branch:** `codex/m8-device-trust`
**Recommendation:** **NOT READY**
**Disposition:** Proposal remains PROPOSED / NOT ACCEPTED. No trust activation,
implementation, tests, Actions, commit, or push were authorized by this review.

## Focus

Re-check whether component-wise cutoff filtering closes the prior cycle
false-positive (prior High #7) without permitting an unauthorized restriction or
introducing order-dependent results. Also re-check M8-R1..R5 and prior Critical
findings against current code facts.

## Findings (at review time)

### Critical

1. **Ineligible external breaker (reducer):** Step 2 allowed treating an
   outside-SCC restriction as an “independently established” cutoff before the
   issuer’s effective cutoff was resolved. Concrete graph: `C→D` (cutoff 2),
   `D@5→A` (cutoff 9), `A@10→B`, `B@10→A`. Correct condensation order holds
   `{A,B}`; an unordered reading drops `A@10→B` and applies compromised
   `B@10→A`. Invariant: order-independent reduction; no ineligible cutoff
   influence (ADR-0019 §5/§8).
2. **Sync/import race (prior Critical #1, open):** Importer lock participation
   remains unimplemented. Code: writers/readers document out-of-protocol actors;
   `append_frame` does not require device/inventory locks.

### High

3. Step 2 “dependency component” procedure was not uniquely determined
   (SCC/condensation/fixed-point missing) → replica divergence risk.
4. Approval IPC OS peer-credential mechanisms still named as gates, not pinned
   per platform (prior High #3).
5. v3 `security.*` → `RetrievalSafetyState` integration absent in code; writer
   correctly forbidden until it exists (prior High #6).
6. “Competing fork” lacked a wire-level predicate.

### Medium

7. Proposed append lock lifetime (inventory held through append; issuer+target
   locks) does not match current `canonical_writer` (existing-device skips
   inventory; inventory released before append on first-create).
8. Self-retirement listed as “outside the component” despite being an in-SCC
   identity cap.
9. Bootstrap streaming parser still proposal-only (prior Medium #9).
10. Uncertain-append / journal durability / UUID wording: no new prose defect.

### Low

11. Cycle hold / no M8 recovery aligned with Master Spec §9 and ADR-0019 safe
    conflict; availability DoS remains an explicit owner-acceptance consequence.

## Areas with no material new issue

Old-reader v3 → `Unverifiable` fail-closed (code), direct-anchor/no transitive
trust, case-sensitive `DeviceId`, sequence-gap + existing-event cutoff binding,
operation-digest field set, known-but-ineligible issuer filtering intent
(prior Critical #2 closed in text, contingent on Critical #1 fix), conflicting
second-proof pin, R1 consume-before-sign + post-confirm revalidation text,
`security.*` exclusion from memory projection.

## Immediate documentation follow-up (after this review)

Applied the same day as a documentation-only correction in
`docs/adr/ADR-0019-m8-protocol-addendum-proposal.md` §4/§6/§7 targeting Critical
#1 / High #3 / High #6 / Medium #8: normative SCC + condensation topo
processing, eligible-only external cutoffs, self-retirement as a per-identity
cap, a minimal competing-fork predicate, and required golden vectors. That
follow-up is **not** a new independent review, not acceptance, and does not
close the importer or IPC implementation gates. Re-review of the corrected
procedure remains required before owner acceptance.

A further documentation-only correction now makes the §5.1 multi-device lock
order an M8 target contract rather than current behavior. The source review
confirmed that `canonical_writer.rs` holds one per-device lock through append,
skips the inventory lock for existing devices, and releases the first-create
inventory guard before append; it has no multi-device lock API. This does not
close the importer publication barrier finding or authorize trust activation.
