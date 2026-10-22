# M8 independent protocol security review

**Date:** 2026-10-22  
**Reviewer:** Cursor independent read-only review, existing completed task `Security review of proposal`  
**Candidate:** `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md` on `codex/m8-device-trust`; review output did not record a Git SHA. The reviewed candidate text was included in checkpoint `b753c7b`.  
**Recommendation:** **NOT READY**  
**Disposition:** Proposal remains PROPOSED / NOT ACCEPTED. No trust activation.

## Findings

### Critical

1. **Sync/import race:** The proposal admits that an importer may mutate canonical event files outside the daemon lock protocol. A mutation between target-prefix validation and the reader's verification can produce a stale or mixed snapshot. Require a reviewed importer publication barrier using inventory and affected device locks; post-hoc scans are insufficient. Trust activation remains blocked until implemented and raced in tests.
2. **Issuer eligibility contradiction:** The proposal excludes non-approved issuers from authority but also treats their canonical events as invalid input, which can make previously valid restrictions from an issuer that is later restricted turn into a Brain-wide hold. Distinguish unknown/unresolvable streams from known, cryptographically verifiable history whose issuer is now prefix-ineligible; filter the latter by effective cutoff.

### High

3. **Approval session binding:** “Authenticated Desktop process/session” lacks a concrete same-user IPC security binding. Require connection-scoped capability, OS-authenticated peer identity, disconnect invalidation, and per-platform replacement/reuse analysis; same-user or PID alone is insufficient.
4. **Restriction lock set:** Post-confirmation target-prefix revalidation does not explicitly lock both issuer and target. Require inventory lock plus sorted issuer/target writer locks through revalidation and append.
5. **Conflicting proofs:** A second valid proof for one transaction with a different peer identity/key/transport has no specified result. Pin the first durable proof; treat changed bindings as a typed transaction conflict.
6. **New-reader policy integration:** Verified v3 `security.*` must update `RetrievalSafetyState` through the policy reducer, rather than be classified as generic unsupported memory semantics. This reader integration is a prerequisite to v3 writes.
7. **Cycle denial of service:** A valid candidate-graph cycle causes a Brain-wide hold even when an edge is later ineligible. The availability impact and recovery path require explicit owner acceptance or a separately reviewed deterministic rule before protocol acceptance.

### Medium

8. **Uncertain-append recovery:** Search both stable event ID and transaction ID in the fully verified stream under locks before any sequence reservation. Never append another body for an existing event ID.
9. **Parser implementation:** Bootstrap bounds need a dedicated streaming/tokenizer parser before allocation/JCS; current event parsers do not provide this guarantee. Add explicit reject vectors.
10. **Journal durability wording:** Windows does not use the Unix parent-directory sync step. Match the proposal to the actual platform-specific durability helper and avoid claiming unsupported parity.
11. **Brain UUID canonicalization:** Lowercase-only M8 wire IDs need a single conversion point for legacy local metadata that the existing validator may accept in uppercase; do not rewrite v1/v2 bytes.

### Low

12. **Transaction entropy wording:** The introductory text said 128-bit-or-better while the schema used 32 random bytes. Use one 256-bit (32-byte) rule.

## Areas with no material issue found

The reviewer found the v1/v2/v3 chain-link direction, old-reader fail-closed intent, direct-anchor/no-transitive-trust model, case-sensitive DeviceId comparison, operation-digest field set, sequence-gap/cutoff handling, and exclusion of `security.*` from memory retrieval consistent with the cited source or authority text. These are scoped review observations, not implementation or runtime proof.

## Follow-up status

The proposal now incorporates documentation-level clarifications for findings 2–6 and 8–12, plus an explicit importer lock/publication release gate for finding 1. The cycle denial-of-service recovery choice remains open for owner decision; the importer and retrieval-safety gates also require implementation and tests. This review remains NOT READY and does not authorize acceptance, trust activation, or M8 completion. No Actions or tests were run for this documentation follow-up.
