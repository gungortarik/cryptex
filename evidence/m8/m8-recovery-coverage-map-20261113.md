# M8 recovery coverage map — November 13

This maps the nine crash boundaries in ADR-0019 addendum §5 to existing checks.
It is a gap inventory, not complete M8 acceptance or a power-loss certification.
The baseline source `adad524623352dbcdc94b478040886637a777df0` passed ordinary
[CI43514494333](https://github.com/gungortarik/cryptex/actions/runs/43514494333),
including Mac/Windows readiness and Windows cross-account IPC ACL. New bilateral
and batched recovery passed full [CI43518226858](https://github.com/gungortarik/cryptex/actions/runs/43518226858)
and default-off native [CI43518652758](https://github.com/gungortarik/cryptex/actions/runs/43518652758)
on `40da5f7`; the [receipt](m8-terminal-journal-native-40da5f7.json) records scope.

| §5 boundary | Existing check and actual scope | Remaining acceptance |
|---|---|---|
| Offer/proof only | `bootstrap_journal::tests::durable_offer_proof_promotes_exact_prefix_into_owner_approval_transaction`; `trust_reducer::tests::only_a_verified_local_peer_approval_anchor_builds_the_direct_roster`. Durable public exchange and local canonical authority are distinct. | Connect the supported product flow; a domain test does not prove UI/transport visibility across the entire flow. |
| Owner confirms before record commit | `bootstrap_journal::tests::owner_security_operation_requires_exact_persisted_confirmation_before_append` and released-cancellation tests preserve confirmation and uncertainty semantics. | Final product-flow proof across the confirmation/append crash boundary; do not infer it from state-shape tests alone. |
| Sequence reserved, event not durable | `canonical_writer::tests::sequence_gap_after_allocate_without_append_preserves_hash_chain`; append attempts reject zero/reused reservations. | Preserve shared-allocator behavior in final v3 product integration; no automatic startup append. |
| Security-event durability uncertain | `knowledge_materialization::tests::confirmed_self_retirement_append_recovers_after_durable_append` injects failure after durable append/before tip; recovery proves the existing event. Journal terminal tests inject write/flush/replace/reopen/sync failures. New batch proofs, bilateral counterpart arrival and seventeen real anchors passed both native platforms on40da5f7. | Domain reopen is distinct from a complete Desktop launch with seventeen operations. Fault seams are process/filesystem-step evidence, not physical power-loss proof. |
| Approval durable, roster stale | Incoming publication rebuild tests plus existing complete-inventory and guarded-vault/search tests distinguish canonical truth from derived freshness. | `incoming_publication` is Windows-only (`lib.rs` cfg); ordinary Mac readiness does not execute it. Final supported Mac incoming/rebuild/product path remains open. |
| Both owner approvals, local record not durable | Journal complete-transcript tests and real Windows pairing fixtures keep four signatures separate from durable local approval. | Final product-flow failure proof, including truthful local/peer status; a signed transcript alone is never sufficient. |
| Local approval durable | Windows `incoming_publication::tests::publishes_peer_batch_persists_reopens_and_replays_without_rewriting`; source-bound host/VM/controller receipts cover their recorded paths. | Final cross-platform flow and truthful peer-durability/protection status; separate receipts do not imply an unexecuted combined run. |
| Restriction durable, projection incomplete | Windows `incoming_publication::tests::live_commit_rebuild_failure_holds_all_clones_until_verified_repair` and `post_commit_refresh_failure_never_reports_a_clean_rejection`; trust reducer's signed cutoff/retirement cases. | Finish signed target-history extrema/before-at-after matrix and accepted product-path projection recovery. Existing v1 guarded-search checks alone do not prove this row for every v3 policy case. |
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
- Current compatibility tests use unsupported version99 and preserve canonical
  bytes/derived sentinels. Frozen v1 envelope refusal plus those current-reader
  tests still do not prove a real old-product whole-Brain hold on signed v3.
- Full backup/restore implementation belongs to M11. M8 still needs its own
  schema rollout/downgrade and trust reconstruction acceptance; this map does
  not add the unrelated M11 or M16 campaigns as new M8 requirements.

The remaining decisions and final seven-area product acceptance are tracked in
[M8_REMAINING_ACCEPTANCE.md](../../docs/M8_REMAINING_ACCEPTANCE.md). This map
reduces uncertainty about existing coverage without relabeling unit tests,
development fixtures or source review as milestone completion.
