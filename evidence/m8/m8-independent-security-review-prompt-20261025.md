# Cursor prompt — independent M8 protocol and importer review

Perform a fresh, independent, read-only follow-up security review of Cryptex M8
source at exact commit `74754e3` on branch `codex/m8-device-trust`. The prior
follow-up at `3e83307` is recorded in
`evidence/m8/m8-independent-security-review-followup-20261025.md`; review the
pinned source commit, including prepared-temp recovery and the Windows M8
staging/journal directory-flush change, rather than earlier evidence summaries
or uncommitted working-tree assumptions.

Do not edit files, run tests, change trust state, run GitHub Actions, commit, or
push. The protocol addendum remains PROPOSED; trust and transport activation
must remain disabled. This is a review of current behavior and design gates, not
a request to approve or implement the proposal.

Read these authority files first:

- `AGENTS.md`
- `docs/00_MASTER_SPEC.md`
- `docs/07_SYNC_AND_CONFLICT_PROTOCOL.md`
- `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`
- `docs/milestones/M8_DEVICE_TRUST_REPLICATION.md`
- `docs/adr/ADR-0019-m8-peer-trust-design-proposal.md`
- `docs/adr/ADR-0019-m8-restriction-authority-addendum.md`
- `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md`
- `docs/adr/ADR-0015-brain-device-identity-foundations.md`
- `docs/adr/ADR-0016-immutable-event-store-productization.md`
- `docs/adr/ADR-0018-local-device-retrieval-identity.md`

Inspect at minimum:

- `crates/storage/src/layout.rs`
- `crates/storage/src/segment_io.rs`
- `crates/events/src/versioned_reader.rs`
- `services/cryptexd/src/canonical_writer.rs`
- `services/cryptexd/src/sequence_store.rs`
- `services/cryptexd/src/managed_profile/native_xml.rs`
- `services/cryptexd/src/managed_profile/native.rs`
- `services/cryptexd/src/knowledge_materialization.rs`
- `services/cryptexd/src/peer_review.rs`
- `services/cryptexd/src/import_limits.rs`
- `services/cryptexd/src/import_staging.rs`
- `services/cryptexd/src/error.rs`
- `services/cryptexd/src/main.rs`
- `services/cryptexd/src/paths.rs`
- `services/cryptexd/src/owned_process/`
- `evidence/m8/m8-windows-staging-recovery-slice2-20261024.md`
- `evidence/m8/m8-independent-windows-staging-review-20261024.md`
- `evidence/m8/m8-windows-staging-directory-flush-20261025.md`

Review the implementation against the owner-approved Windows publication
direction in ADR-0019 §5.1.1 and separately identify protocol details that are
still only proposed. At minimum, challenge:

1. Import staging, prepared-manifest validation and startup reconciliation:
   path/link boundaries, source and canonical file identity aliases, exact
   manifest/tree coverage, lock ordering, destination races, no-replace moves,
   cleanup and fail-closed behavior.
2. Whether the current recovery behavior is safe for each crash point. It only
   aborts an intact prepared transaction when every canonical destination is
   absent; visible destinations remain a conflict. There is not yet a canonical
   publication path, committed marker, or partial-publication replay. Windows
   staging/journal parent sync now opens and flushes a directory handle, rejects
   reparse points, and is covered by native tests; assess whether the exact
   implementation and evidence satisfy the durability contract, including
   pinned-path/race behavior and crash/power-loss limits. Confirm the source and
   exact limits of these claims.
3. Semantic import validation: supported v1/v2 schema/signature/chain checks,
   trust/key binding, cutoff policy and `RetrievalSafetyState`. Distinguish the
   implemented caller-supplied resolver/tip validation from absent policy
   integration.
4. All canonical writer and reader interleavings with inventory/device locks,
   sequence allocation, projections/materialization and startup gating. Check
   every daemon entry point and identify any path that can observe or publish
   partial state.
5. M8 protocol proposal R1–R5: bounded bootstrap parser, owner/session-bound
   approval IPC, pending journal/idempotency, v3 cross-version chain and
   `RetrievalSafetyState` reducer, identity/key equality. These are not fully
   implemented or accepted; identify any Master Spec or accepted-ADR conflict.
6. Windows and macOS parity, native durability/fault injection, process
   identity/revalidation, offline convergence, cutoff/restriction behavior,
   unknown-version holds and the remaining M8 Definition-of-Done gates.

Do not treat the 16/16 `import_staging::tests` or 414/418 daemon library tests
(4 ignored) as proof of the broader M8 gates. Report exact file/line references,
severity (Critical/High/Medium/Low), concrete counterexample or attack sequence,
affected invariant, and the smallest safe correction. Separate confirmed code
facts from assumptions. List what was actually reviewed and any missing
evidence.

End with exactly one recommendation: `NOT READY`, `READY FOR OWNER ACCEPTANCE`,
or `BLOCKED BY MISSING EVIDENCE`. Do not call M8 complete, enable trust, or claim
the full protocol is accepted.
