# Cursor prompt — independent M8 protocol and importer review

Perform an independent, read-only security review of the current working-tree
M8 protocol proposal and its first Windows staging/recovery implementation.
Review the exact working-tree contents; do not assume a commit hash identifies
them. The review must compare the code against the candidate contract in §5.1.1
and identify implemented behavior, missing requirements, and unsafe claims.

Do not edit files, run tests, change trust state, run GitHub Actions, commit, or
push. The protocol remains PROPOSED and must remain unaccepted until required
review findings are resolved and the owner explicitly accepts it.

Read these authority and implementation files before review:

- `AGENTS.md`
- `docs/00_MASTER_SPEC.md`
- `docs/07_SYNC_AND_CONFLICT_PROTOCOL.md`
- `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`
- `docs/milestones/M8_DEVICE_TRUST_REPLICATION.md`
- `docs/adr/ADR-0019-m8-peer-trust-design-proposal.md`
- `docs/adr/ADR-0019-m8-restriction-authority-addendum.md`
- `docs/adr/ADR-0015-brain-device-identity-foundations.md`
- `docs/adr/ADR-0016-immutable-event-store-productization.md`
- `docs/adr/ADR-0018-local-device-retrieval-identity.md`
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
- `evidence/m8/m8-windows-staging-recovery-slice2-20261024.md`

Review §5.1.1 and the current staging/recovery code adversarially. In
particular:

1. Does a Syncthing `transport/incoming` folder, exact managed ID/path check,
   and daemon-side import preserve the accepted rule that Syncthing is transport
   while `cryptexd` is the supported canonical writer? Is any stronger
   same-user-process isolation actually required by the cited authority, or is
   the proposal clear about its threat-model boundary?
2. Can the daemon take a safe snapshot from transport while rejecting
   symlinks/reparse points, path traversal, hard links where relevant, mutable
   files, malformed names, oversized files/batches, and duplicate paths on
   both platforms? Identify concrete races and required handle-based checks.
   Specifically inspect the Windows component-by-component `NtCreateFile`
   walk, its `FILE_OPEN_REPARSE_POINT` and share flags, and the native tests for
   junction rejection and ancestor-rename refusal. Distinguish tested behavior
   from a broad race guarantee. The approved threat model does not require
   isolation from arbitrary same-user malware; assess whether a remote
   Syncthing peer can still control any path or state that crosses that
   boundary. Check cleanup behavior on malformed Unicode names, missing or
   renamed transaction roots, and hostile or unknown journal entries.
3. Are sealed `.seg` files sufficient for convergence? How are missing earlier
   segment indexes, gaps, forks, duplicate-identical segments, divergent
   destinations, and active `.open` packs treated?
4. Is validation complete before publication, including event schema/signature,
   Brain/device identity, full stream chain, trust reducer, cutoff, and
   cross-device authorization dependencies? Which locks and inventory snapshot
   must be revalidated after staging?
5. Is the manifest sufficient and tamper-evident for crash recovery? Review
   durability order, same-volume no-replace behavior, parent-directory sync,
   partial publication, crash after last move but before commit marker, retry,
   quarantine/hold, and startup gating. Check Windows and macOS guarantees
   separately; do not assume rename semantics are identical.
6. Does startup recovery definitely run before every canonical reader,
   retrieval, and materialization worker? Could a second daemon, another
   process, or an already-running reader observe partial publication?
7. Does the proposed inventory + sorted device-lock order integrate with the
   actual writer, readers, sequence allocator, SQLite/materialization, and
   pending-journal mutex without deadlock or an inventory race?
8. Challenge the pre-existing M8 approval protocol as well: approval/session
   process binding; stale UI/replay/cancel; parser bounds; journal loss/tampering;
   reducer cycles/cutoffs/forks; v1/v2/v3 chain and `RetrievalSafetyState`;
   and accepted-ADR conflicts.

Return findings only, ordered by severity (Critical/High/Medium/Low). For each
finding give exact file/line references, a concrete counterexample or attack
sequence, affected invariant, and smallest safe correction. Separate confirmed
code facts from assumptions. If an area is sound, state what you checked and
what evidence supports that conclusion. End with exactly one recommendation:
`NOT READY`, `READY FOR OWNER ACCEPTANCE`, or `BLOCKED BY MISSING EVIDENCE`.
Do not call the protocol accepted, implemented, or tested.
