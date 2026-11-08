# M8 persistent-import independent review packet — 2026-11-08

## Status

Follow-up: the owner-authorized separate technical reviewer has now assessed
commit `2165539491733b40ff61908955823e0b3fafe514`. Two confirmed findings and
correction commit `f072ae234344bcee51b9bcb995fc4d1678aba45f` are recorded in the
[review result](m8-independent-review-findings-20261108.md). The original packet
below remains review scope, not a verdict. Technical agent review does not
replace required human review or close normal-publication/M8 gates.

Prepared for an independent reviewer. **This document is not an independent
review, approval, or authorization to activate publication.** The implementation
author's self-review found/fixed a lifecycle hardlink truncation defect; selected
Windows/Mac checks have now returned. Overall activation remains NOT READY until
the remaining design, lifecycle, durability and independent-review gates close.

## Authority and code scope

Start with `docs/00_MASTER_SPEC.md`, accepted ADRs, sync/platform contracts,
Definition of Done and `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md`.
ADR-0019 protocol addendum is proposed overall: its owner-approved Windows
§5.1.1 implementation slice is not blanket acceptance of the whole protocol or
permission to enable trust, transport, schema rollout or publication.

The core under review is present in pinned Windows source
`c55f3b70c763aef7541481e3897dba348d300c47`. Important implementation commits:

- `2705330`: crate-private Windows incoming transaction integration, replay
  pruning and test fixtures.
- `8f8bd36`: safe lifecycle lock-file adapter and exclusive caller-owned guard,
  security self-review/native preflight.
- `bc5e5d0`: Windows-only REST test-helper cfg correction; production behavior
  and test assertions unchanged.

Review these current files and their existing dependencies, not only the diff:

- `services/cryptexd/src/incoming_publication.rs`
- `services/cryptexd/src/incoming_publication_tests.rs`
- `services/cryptexd/src/import_staging.rs`
- `services/cryptexd/src/knowledge_materialization.rs`
- `services/cryptexd/src/instance.rs`
- `services/cryptexd/src/platform/instance_file.rs`
- Canonical inventory/device writer locks, local-anchor resolver and trust reducer.

The new service has no normal CLI, IPC, watcher or managed-controller caller.
Its matching `&mut InstanceLock` is necessary, not a complete proof that a live
daemon's concurrent readers are held after partial publication/failure.

## Evidence map

1. `m8-persistent-import-integration-20261108.md`: transaction/admission/error
   boundaries; immutable replay; process interruption and derived rebuild scope.
2. `m8-security-platform-review-20261108.md`: self-review reproduction/fix,
   host 26300.9818 owned-HTTP/TCP discrepancies and signing-policy observations.
3. `m8-macos-native-checks-20261108.md`: native arm64 Mac 80 selected tests on
   source `74c6c40`; native corrected-source build/strict Clippy on `bc5e5d0`.
4. `m8-windows-platform-checks-20261108.md`: exact source `c55f3b7`, Windows VM
   26200.9711, 26 selected tests and 60/60 exact-established TCP samples.
5. `m8-trusted-import-relay-20261108.md`: host/VM relay of public synthetic bytes
   through actual receiver policy validation; explicitly validated-only.

Receipt ZIPs are not committed. Evidence records their exact hashes/member
counts and source bindings. Native receipts are owner-supplied runner observations,
not attestation. Do not combine test counts into a claim that an entire current
cross-platform suite passed: source revisions, native subsets and lint scopes
differ. The host's full library suite had five failures; that fact remains.

## Review questions

1. Does single-instance/startup recovery precede any canonical inspection,
   staging, transport, IPC or derived reader? Can a future caller fail to hold
   all readers after `RecoveryRequired` or `CommittedNeedsRebuild`?
2. Do inventory then sorted device locks cover every canonical mutator and remain
   held from the exact local trust/M3/M4 snapshot through the final marker?
   Challenge concurrent append/seal/restriction and live-controller integration.
3. Can replay pruning accept a divergent/stale/untrusted segment, use a proof
   from another runtime, delete canonical/transport bytes, or evade input budgets?
4. Are stage/source/destination identities no-follow, single-link and hash-bound
   under appropriate ancestor handles? Does evidence survive every prepare/move/
   marker/cleanup ambiguity without false rollback or unsafe cleanup?
5. Does post-import policy reapply Brain, signature, chain, trust/cutoff/content
   semantics on the complete union, including peer approval and restriction
   controls, duplicate identities, forks and unsupported versions?
6. Is commit distinguished from derived refresh? Do stale M3/M4 watermarks stay
   held on restart until verified reconstruction? Review self-device incoming
   stream/allocator assumptions before any broader restore/public import scope.
7. Does lifecycle lock-file opening prevent symlink/hardlink truncation and
   bound malformed metadata reads without weakening genuine lock conflicts?
8. What native durability guarantee actually follows from each platform's file
   and directory primitives? Process-exit tests are not power-loss evidence.
9. Are private keys/backend credentials kept out of renderer, payload bundles,
   process arguments and logs? Are all tool/pilot scopes represented honestly?

## Explicit open gates

- Independent security assessment of corrected code, with findings/remaining
  assumptions recorded at exact commits. The author cannot close this alone.
- Live managed-controller publication wiring and enforceable runtime-wide
  admission/failure barrier, reviewed before adding any external entry point.
- Mac production publication design/parity: ancestor handle walk is absent in
  the current Unix helper; hardlink/unlink is not the Windows atomic move;
  managed owned-endpoint attestation is not implemented there. Passing primitive
  tests is not permission to remove `cfg(windows)` from the new service.
- Supported filesystem/marker-parent physical durability and native failure
  matrix. VM filesystem type was not recorded and physical power loss was not run.
- Development-host Windows 26300.9818 endpoint/TCP discrepancy, even though the
  same current tests pass on the 26200.9711 VM.

An acceptable review result states which bounded slice was reviewed, exact
commit(s), findings/severity, reproduction/validation and unclosed gates. It
must not equate a review pass for one slice with M8 completion or authorize
ordinary publication by implication. No new device exercise is needed to
reconfirm the already-recorded selected native checkpoints.
