# Cursor prompt — independent M8 protocol security review

Review the current working-tree proposal at
`docs/adr/ADR-0019-m8-protocol-addendum-proposal.md` as an **independent,
read-only security reviewer**. Do not edit files, run tests, change trust state,
run GitHub Actions, commit, or push. The proposal is explicitly PROPOSED and
must remain unaccepted until its required review is complete.

This is a follow-up to the initial NOT READY review recorded in
`evidence/m8/m8-independent-security-review-20261022.md` and the re-review in
`evidence/m8/m8-independent-security-rereview-20261022.md`. Review exact commit
`7fc6089` (or a later descendant only if these files are unchanged). Focus on
whether the new SCC/condensation cutoff procedure closes the cycle findings
without permitting unauthorized restrictions or order-dependent results, and
whether §5.1 now accurately distinguishes the proposed target lock contract
from current `canonical_writer.rs` behavior.

Read these authority/context files first:

- `AGENTS.md`
- `docs/00_MASTER_SPEC.md`
- `docs/milestones/M8_DEVICE_TRUST_REPLICATION.md`
- `docs/adr/ADR-0019-m8-peer-trust-design-proposal.md`
- `docs/adr/ADR-0019-m8-restriction-authority-addendum.md`
- `docs/adr/ADR-0015-brain-device-identity-foundations.md`
- `docs/adr/ADR-0016-immutable-event-store-productization.md`
- `docs/adr/ADR-0018-local-device-retrieval-identity.md`
- `crates/events/src/versioned_reader.rs`
- `services/cryptexd/src/canonical_writer.rs`
- `services/cryptexd/src/sequence_store.rs`
- `services/cryptexd/src/knowledge_materialization.rs`
- `services/cryptexd/src/peer_review.rs`

Challenge the proposal adversarially, with particular attention to:

1. M8-R1 approval digest completeness, session/process binding, stale state,
   restart, replay, cancellation after signatures are released, and TOCTOU
   between the displayed target prefix and canonical append.
2. M8-R2 whether all parser bounds really run before allocation/JCS and whether
   the exact wire schema and numeric rules are implementable.
3. M8-R3 journal tampering/loss, transaction-ID reuse, uncertain append,
   retries after sequence gaps, duplicate transaction IDs/event IDs, the
   security-operation-digest field set and domain, snapshot/signature
   generated-versus-released state, the proposed Unix and Windows journal
   atomic-replace/fsync steps, rollback/torn-write behavior, lock inversion,
   and whether the target inventory/device/sequence ordering is implementable
   without claiming that current code already satisfies it.
4. M8-R4 old-reader fail-closed behavior, key resolution without transitive
   trust, v1/v2/v3 chain integration, and interaction with Syncthing/import
   writes outside daemon locks.
5. M8-R5 identity canonicalization against the actual validators and the
   case-sensitive DeviceId contract.
6. The issuer/cutoff reducer: component-wise eligibility filtering before
   classifying cycles; externally established cutoffs; nested/dependent
   components; source/target ordering; self-retirement caps; skipped sequence
   numbers; forks; multiple restrictions; compromised issuers; Brain-wide
   hold/liveness; and convergence independent of arrival order. Challenge the
   new rule with concrete graphs, including a cycle containing an already
   ineligible edge and a nested cycle whose issuer cutoff depends on another
   component. Do not assume the candidate SCC/filtering procedure is correct
   merely because it is stated; identify a reproducible graph where its result
   is ambiguous, unsafe, or unavailable.
7. Whether the proposal conflicts with accepted ADRs, privacy requirements,
   platform durability guarantees, or existing event-store semantics.
8. Whether importer publication can actually join the proposed inventory/device
   lock protocol; verify the current `canonical_writer.rs` and sync/import path.

Return findings only, ordered by severity (Critical/High/Medium/Low), with exact
file/line references, a concrete counterexample or attack sequence, the
affected invariant, and the smallest safe correction. Separate confirmed code
facts from protocol assumptions. If you find no issue in an area, state what
you checked and what evidence supports that conclusion. End with a clear
recommendation: NOT READY, READY FOR OWNER ACCEPTANCE, or BLOCKED BY MISSING
EVIDENCE. Do not call the protocol accepted, implemented, or tested.
