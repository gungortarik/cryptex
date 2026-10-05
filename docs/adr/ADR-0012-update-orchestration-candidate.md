# ADR-0012 update orchestration candidate

**Status:** PROPOSED — isolated M0 experiment, not production selection
**Date:** 2026-10-03
**Authority:** Master Spec; accepted version/update spec; M0 P0-2 contract

## Motivation and alternatives

The approved runtime direction is Desktop-owned Mode B with per-user login
startup. This does not determine updater orchestration. Native Windows NSIS
installation/replacement/operator rollback passed, but the generated installer
copies components sequentially and may force-stop Desktop. It does not establish
interrupted pair replacement or automatic rollback safety.

Candidates:

1. Stock installer replacement: retained actual install evidence exists, but
   interrupted copy can leave a mixed pair. Keep it as distribution evidence.
2. Two independently replaced component files with a journal: requires recovery
   for every partial combination and OS executable-lock behavior.
3. Stage a complete verified pair in a separate slot, then replace one selection
   record. This is the smallest filesystem candidate being measured here.

No updater framework, network service, signing policy, production layout or
compatibility support window is selected by this proposal. Node is the existing
pinned development runtime; these scripts are test tools, not a shipped updater.

## Candidate transaction

- Each slot has exactly Desktop bytes, daemon bytes and a versioned experiment
  manifest. Slot IDs are bounded ASCII identifiers; no paths come from metadata.
- Validate declared positive protocol equality and SHA-256 payload digests before
  staging. These digests establish correspondence with supplied experiment input,
  not publisher authenticity. Runtime protocol negotiation is a separate gate.
- Reject links/junctions, in-repository roots, unowned roots, reused slots,
  incomplete pairs, unknown manifest schemas and malformed records.
- Preserve the old slot. Write a flushed immutable pending intent containing
  before/after selections, then replace the selection record using a same-root
  rename. No pair is overwritten one file at a time.
- Require explicit health acceptance. Negative/throwing health restores the
  previous complete pair automatically. Reverify candidate bytes before commit.
- Delete pending intent only at commit. On restart, a valid pending transaction
  conservatively restores the old pair, even if the candidate was already healthy.
  Recovery is idempotent. Corrupt last-known-good/metadata causes explicit failure
  with diagnostic files retained; it is not silently treated as a successful rollback.
- Package operations receive no canonical-data path. Fixture inventory compares
  file membership and SHA-256 bytes before/after each failure scenario.

## Measurement contract and limits

The executable prototype is `tools/m0-lifecycle-compare/scripts/package-transaction.mjs`.
Tests force-kill owned updater subprocesses at five observed boundaries: staged,
intent, selected, healthy and committed. This is actual process interruption,
not a simulated exception, but it is not machine power loss.

The candidate has no OS-level process lock, same-user adversarial tamper
resistance or immutable OS permissions for slots. Two deterministic Windows
regressions now cover stale staging and the race between selection validation
and pending-intent creation: the stale updater fails with a typed error and
preserves the winner's selection. This is not stress testing or proof of native
cross-platform serialization. Mac parity of these two controlled interleavings
passed at `ad37989` (31/31 tests); see the Mac concurrency evidence. SHA-256 is from Node's
standard crypto library; no custom cryptography is introduced. Windows directory
fsync is not implemented through Node, so power-loss durability is explicitly
unverified. POSIX directory fsync must pass on Mac; failures are reported, never
swallowed.

The original filesystem-only proof uses injected health outcomes. Retained binary copies
are hashed/staged but not executed. Desktop is not connected to the selection
record. `.app` resources, code signing, native package layouts, executable locking,
installed startup references and successor launch remain outside this experiment.
Synthetic data is not accepted canonical events. No P0-1 corpus or production
data is used, and no M1/M2 event-store implementation is authorized here.

A separate Windows headless follow-up now connects the transaction callback to
actual copied daemon startup, the shared Rust Desktop IPC client, PID/held-lock
matching, clean stop and lock release. Failed executable and declared-versus-real
protocol mismatch restore the old selection; the experiment explicitly restarts
and checks the restored daemon. See `windows-package-real-health-20261003.md`.
This does not integrate installed Desktop/successor launch or change proposal
status. Mac real-health parity and the remaining native integration gates remain open.

## Gates before production integration or ADR acceptance

Windows recovery-fault follow-up passes six deterministic injected cases:
temporary selection write (`ENOSPC`), file flush (`EIO`), selection rename
(`EACCES`), intent unlink (`EACCES`), and automatic rollback rename failure
after rejected/throwing health. Mutation errors expose `RECOVERY_IO_FAILED`
with phase, underlying OS code, and observed intent presence. The four direct
faults retain the journal and permit an idempotent baseline recovery after the
fault is removed; neither automatic rollback fault claims restoration. This
does not establish actual disk-full/ACL behavior or power-loss durability.
Directory-flush failure after successful unlink may leave no journal; completion
is still reported as unestablished. Mac parity passed at `9d9159c` (41/41 tests);
later durability faults remain open. See
`windows-package-recovery-io-evidence-20261003.md` and
`macos-package-recovery-io-evidence-20261004.md`.

1. Mac filesystem/process-interruption parity using the exact test plan.
2. Review this candidate and select the scoped orchestration explicitly; do not
   infer ADR acceptance from passing fixture tests.
3. Define a native helper/launcher handover behind platform adapters: prevent
   competing transitions, quiesce daemon, prove lock release, protect verified
   package identity, and launch the selected complete package. A running Desktop
   must not overwrite itself or become an additional canonical writer.
4. Verify real signed package/resource integrity and actual Desktop/daemon IPC
   readiness. Pair metadata alone cannot establish runtime compatibility.
5. Exercise partial I/O, recovery I/O failure, cross-platform concurrent
   invocation (including process races beyond the two deterministic Windows
   cases), native executable locks, restart/login during transition and relevant
   durability boundaries. Define controlled errors when rollback cannot be
   completed.
6. Verify unchanged immutable storage fixtures using the accepted P0-1 layout,
   without altering retained corpora; production data is not a test fixture.
7. Measure real packaged flows on both targets and update the comparison before
   accepting ADR-0012. M14 productizes the accepted mechanism later.

No window manipulation is needed for this experiment. Actual GUI/native installer
steps must be scheduled with the operator; the user's game must keep focus.
