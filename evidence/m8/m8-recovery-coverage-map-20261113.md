# M8 recovery coverage map — November 13

This maps the nine crash boundaries in ADR-0019 addendum §5 to existing checks.
It is a gap inventory, not complete M8 acceptance or a power-loss certification.
The later [full protocol review](m8-final-protocol-review-20261114.md) identifies
grant-cutoff eligibility and retirement-contract issues outside the completed
scoped recovery proofs. The rows below do not certify those missing cases or
unconditional protocol acceptance. New targeted signed regressions are required
after the concrete policy decision; passed recovery tests are not discarded.
The baseline source `adad524623352dbcdc94b478040886637a777df0` passed ordinary
[CI43514494333](https://github.com/gungortarik/cryptex/actions/runs/43514494333),
including Mac/Windows readiness and Windows cross-account IPC ACL. New bilateral
and batched recovery passed full [CI43518226858](https://github.com/gungortarik/cryptex/actions/runs/43518226858)
and default-off native [CI43518652758](https://github.com/gungortarik/cryptex/actions/runs/43518652758)
on `40da5f7`; the [receipt](m8-terminal-journal-native-40da5f7.json) records scope.
The later c6e07d6 service-recovery slice passed exact-source
[CI43626437079](https://github.com/gungortarik/cryptex/actions/runs/43626437079)
on both platforms; its [review](m8-restriction-derived-recovery-review-20261114.md)
and [receipt](m8-restriction-derived-recovery-native.json) bind the new coverage.

| §5 boundary | Existing check and actual scope | Remaining acceptance |
|---|---|---|
| Offer/proof only | `bootstrap_journal::tests::durable_offer_proof_promotes_exact_prefix_into_owner_approval_transaction`; `trust_reducer::tests::only_a_verified_local_peer_approval_anchor_builds_the_direct_roster`. Durable public exchange and local canonical authority are distinct. | Connect the supported product flow; a domain test does not prove UI/transport visibility across the entire flow. |
| Owner confirms before record commit | `bootstrap_journal::tests::owner_security_operation_requires_exact_persisted_confirmation_before_append`, released-cancellation tests and `ipc::tests::approval_domain_tests::domain_final_delivery_failure_preserves_durable_signature_without_cancellation` preserve confirmation/failed-delivery evidence without canonical append. | Final product-flow proof across the confirmation/append crash boundary and truthful visible uncertainty. |
| Sequence reserved, event not durable | `canonical_writer::tests::sequence_gap_after_allocate_without_append_preserves_hash_chain`; append attempts reject zero/reused reservations. | Preserve shared-allocator behavior in final v3 product integration; no automatic startup append. |
| Security-event durability uncertain | `knowledge_materialization::tests::confirmed_self_retirement_append_recovers_after_durable_append` injects failure after durable append/before tip; recovery proves the existing event. Journal terminal tests inject write/flush/replace/reopen/sync failures. New batch proofs, bilateral counterpart arrival and seventeen real anchors passed both native platforms on40da5f7. | Domain reopen is distinct from a complete Desktop launch with seventeen operations. Fault seams are process/filesystem-step evidence, not physical power-loss proof. |
| Approval durable, roster stale | Incoming publication rebuild tests plus existing complete-inventory and guarded-vault/search tests distinguish canonical truth from derived freshness. | `incoming_publication` is Windows-only (`lib.rs` cfg); ordinary Mac readiness does not execute it. Final supported Mac incoming/rebuild/product path remains open. |
| Both owner approvals, local record not durable | Common `ipc::tests::approval_domain_tests::domain_durable_completion_requires_journaled_owner_approval_and_replay_is_noop` rejects four-message completion without local journaled approval, then appends the exact authorized event once; complete-transcript and Windows pairing fixtures add scoped evidence. | Final product-flow failure proof, including truthful local/peer status; a signed transcript alone is never sufficient. |
| Local approval durable | The same common domain completion/replay test proves a canonical no-op repeat. Windows `incoming_publication::tests::publishes_peer_batch_persists_reopens_and_replays_without_rewriting` and source-bound host/VM/controller receipts add their recorded paths. | Final cross-platform flow and truthful peer-durability/protection status; separate receipts do not imply an unexecuted combined run. |
| Restriction durable, projection incomplete | c6e07d6 daemon-service tests durably append signed cutoff0/1 restrictions, reconcile AppendUnknown once, hold all four guarded surfaces across on-disk SQLite reopen/partial-M4 failure, then rebuild exact admitted SQLite/vault content twice without canonical changes. Signed extrema/gap/overflow checks on1f5e6dc and existing Windows incoming-publication rebuild-failure tests add scoped evidence. | Domain invariants covered. Verify connection and visible failure/status behavior through supported product UI/session/import routes; service recovery is not full Desktop crash injection. |
| Worker shutdown uncertain | `managed_profile::controller::tests::failed_shutdown_never_reports_clean`; source-bound native combined-loss receipts and controller transfer/restart receipts. | Preserve unresolved status in final integrated product flow. Previously passed unchanged owner packages are not automatically repeated. |

## Additional evidence and precise limits

- Both ordinary native jobs run journal schema/digest/collision checks and the
  fourteen terminal-retention tests, including four actual worker
  processes. Invalid/truncated/unexpected journal evidence remains fail-closed.
- Windows `incoming_publication::tests::process_interruptions_preserve_evidence_and_recover_exact_canonical_bytes`
  covers prepared-temp/manifest sync, both destination moves and committed-temp/
  final-marker sync. `import_staging::tests::xproc_*` covers recovery companions
  and repeatability. These passed in the baseline Windows job; they are not
  silently attributed to Mac.
- `canonical_writer::tests::fail_closed_when_packs_exist_but_signing_keys_missing_does_not_mint`
  covers existing-history missing-key refusal. It does not establish every
  OS-secure-store loss/reinstall/recovery path on an enabled M8 product.
- c6e07d6 adds paired startup with empty/truncated/garbage-suffixed journals and
  a missing fake-store signing seed. Both native platforms preserve original
  journal/canonical/public metadata and refuse without replacement key writes.
  This closes the supplemental facade/domain gap, not an OS reinstall claim.
- November14 [compatibility evidence](m8-protocol-schemas-compat-review-20261114.md)
  adds actual pre-v3 product services at ae61603: successful trusted v1 reads
  precede typed Brain-wide hold on independently signed valid v3. Search,
  timeline, detail, vault and rebuild preserve canonical bytes, database state
  and vault contents on both native platforms. This supersedes the earlier
  envelope-only/version99 coverage gap at historical service-guard scope;
  installed old Desktop/secure-store acceptance remains separate.
- Full backup/restore implementation belongs to M11. M8 still needs its own
  schema rollout/downgrade and trust reconstruction acceptance; this map does
  not add the unrelated M11 or M16 campaigns as new M8 requirements.

## Bounded coverage reconciliation — November 14

Existing domain invariants cover exact offer/proof and owner confirmation,
sequence non-reuse, uncertain append/reconciliation, canonical-only trust and
journal-required completion/replay. Common approval-domain tests are
cross-platform evidence; they must not be mislabeled Mac-only IPC fixtures.
The native supervisor/controller receipts cover uncertain shutdown semantics.
The new restriction/partial-SQLite/domain-reopen and torn-journal/missing-key
facade cases passed both native platforms and are recorded in the
[current recovery review](m8-restriction-derived-recovery-review-20261114.md).

Coverage of these domain invariants is distinct from crash injection through
every boundary of the accepted product UI/transport path. The remaining
integration is concrete: authenticated approval/session routes with truthful
local/peer durability, incoming publication plus canonical trust reconstruction
and guarded derived refresh on each supported platform, and controller outcomes
carried into replication protection status. Transport connectivity grants neither
trust nor proof of peer durability.

Installed old Desktop, actual OS-store reinstall, physical power-loss and
seventeen-operation full Desktop launches are precise evidence limits, not
automatically additional campaigns. Do not reopen already covered lower-layer
journal write/flush/replace/readback/sync tests without a new failure or change.

The remaining decisions and final seven-area product acceptance are tracked in
[M8_REMAINING_ACCEPTANCE.md](../../docs/M8_REMAINING_ACCEPTANCE.md). This map
reduces uncertainty about existing coverage without relabeling unit tests,
development fixtures or source review as milestone completion.
