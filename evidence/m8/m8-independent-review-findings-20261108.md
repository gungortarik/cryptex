# M8 independent technical review and remediation — 2026-11-08

## Scope and independence

The owner authorized a separate reviewer agent to perform a read-only technical
security review. The reviewer started with a fresh context and an isolated,
clean worktree pinned to `2165539491733b40ff61908955823e0b3fafe514`.
It inspected source, authority documents and the
[review packet](m8-independent-review-packet-20261108.md), rather than treating
the author's earlier self-review or native receipts as a security verdict.

This is a second-agent technical assessment, **not an independent human audit,
certification, human diff approval or M8 completion**. Normal publication remains
disabled. Reproductions used only synthetic identities, fake secure storage and
scratch runtimes outside the Git repositories; no owner profile was inspected
or changed. The pinned review worktree was not modified. Executable reproductions
were added only to a separate scratch source copy.

## Confirmed findings

### F1 — P2/medium: zero-cutoff restriction did not bind a known target's key

At the reviewed baseline, `trust_reducer::build_verified_trust_history` checked
the zero cutoff's zero hash but did not compare the target's signing-key ID and
full public key with the local identity or its directly approved peer binding.
An approved peer could sign a well-formed restriction naming the local device
ID together with an unrelated valid key and its correctly derived key ID. The
reducer then applied cutoff zero to the actual local device.

The reviewer reproduced this both through local-anchor history verification
and the real internal `publish_incoming` service: three genuinely signed,
contiguous peer segments were committed, cleanup/rebuild succeeded and a fresh
verified scan reported local cutoff zero. This is a policy/availability integrity
defect; it does not forge a signature or grant an unapproved peer trust.

Correction: compare both exact pinned key fields before interpreting **any**
cutoff for a known local/direct-peer target. A mismatch returns
`TrustHold::InvalidPolicyHistory`. Nonzero boundary hash/key-ID checks remain.
Intentional unknown-third-target restriction metadata is preserved and grants
no trust. Existing signed canonical bytes are never repaired in place.

Regressions:

- `zero_cutoff_restrictions_bind_known_targets_to_their_pinned_keys`: matching
  and unrelated genuine keys, for both local and directly approved targets.
- `rejects_signed_wrong_target_key_restriction_before_publication`: the actual
  service rejects the signed attack before canonical publication, leaves
  canonical hashes/incoming bytes unchanged and cleans unprepared staging.

The author observed the new binding test fail before the correction and pass
after it; the complete 14-test reducer group passed.

### F2 — P2/medium: lower-level import proof could cross runtime boundaries

At the reviewed baseline, staged verification checked locked device IDs but not
the runtime owning those locks. The final publication precondition also lacked
that check. Using synthetic copies of one runtime, the reviewer obtained a
policy proof from A, prepared and committed a batch in B, even while independently
holding B's actual canonical inventory lock. A's locks cannot protect B.

The current internal service's replay-pruning check rejects this route; the
defect was in its lower-level proof/publication interfaces. There is no external
production publication entry point. Those scope limits do not make the broken
lock-proof invariant acceptable for future integration.

Correction: compare the snapshot's runtime at staged admission **and again** at
the final policy publication precondition. Both refuse foreign runtime proofs
before a move or committed marker. The runtime predicate is available on both
platforms because the lower-level verifier is shared.

`import_policy_proofs_cannot_cross_runtime_boundaries` uses identical synthetic
identities and bytes in two runtimes. It tests admission refusal and independently
tests reuse of a legitimate A proof at B's final publication boundary, with B's
inventory lock held; canonical hashes stay unchanged and no commit marker is
written. The author observed separate failures at both boundaries before fixing
each check, then a passing regression.

## Correction binding, verification and bounded verdict

Both corrections are committed as
`f072ae234344bcee51b9bcb995fc4d1678aba45f`. The reviewer independently verified
that all four copied source/test files are raw-byte identical to that commit's
Git blobs, then ran all three new regression entries in scratch: **3 passed,
0 failed**. The clean review worktree remains pinned to the original baseline.

Author checks on the correction:

| Check | Result |
|---|---|
| Daemon library, explicitly excluding the known host owned-HTTP group | 554 passed, 0 failed, 4 ignored, 8 filtered; 74.41 s |
| Daemon binary tests | 15 passed, 0 failed |
| Complete reducer group | 14 passed, 0 failed |
| Complete internal publication group | 11 passed, 0 failed; 25.60 s |
| Strict daemon all-target Clippy (`-D warnings`) | Passed |
| Workspace rustfmt and `git diff --check` | Passed |

These selections overlap; do not add their counts together. All Cargo checks
used locked, offline Rust 1.98.1. The eight filtered library tests are explicitly
`owned_process::tests::http::`; the known host discrepancy remains unresolved.
This is not a full daemon-suite PASS. Source/test assertions in that group were
unchanged. Author broad-test log is outside Git under
`%LOCALAPPDATA%\Cryptex-Dev\m8-independent-remediation-20261108`.

Reviewer baseline checks used unchanged product code plus synthetic tests:
publication 11/11 (original nine plus two exploit reproductions), staging/recovery
63/63, instance 9/9, reducer 14/14 (original thirteen plus one reproduction).
An exploit reproduction passing demonstrates the defect, not safety. Correction
tests assert rejection instead. Reviewer logs/source bindings remain outside
Git under `%LOCALAPPDATA%\Cryptex-Dev\review-scratch\independent-2165539`.

The source review covered source/staging identity checks, Windows ancestor pins,
replay pruning, canonical lock lifetimes, local-anchor reduction, post-import
policy, prepared/temporary/final recovery, receipt cleanup, lifecycle lock files
and derived-refresh failure outcomes. **Bounded verdict: both reproduced findings
are resolved; no additional confirmed defect remains in this reviewed correction.**
This verdict neither permits activation nor closes the gates below.

## Remaining integration gates and continuation

These corrections do not activate ordinary transport publication. Next work is
the live daemon's runtime-wide reader/failure barrier before connecting its
managed controller to incoming publication:

1. Startup recovery must finish before transport, IPC, projection workers or
   guarded readers start. Preserve the current startup ordering.
2. A live import must exclude all canonical readers/writers and derived refresh
   paths for the complete transaction. A mutable instance guard alone does not
   cover already-running IPC worker threads holding runtime paths.
3. `RecoveryRequired` must retain a runtime-wide hold; committed-but-stale derived
   state must remain held until an explicit verified rebuild. Panic/interruption
   cannot reopen admission by dropping a temporary guard.
4. Review lock ordering against inventory/device locks. The existing staging
   reservation must still be released before waiting for canonical locks; do
   not turn that reservation mutex into a long-lived runtime barrier.
5. Before wiring a caller, verify concurrent retrieval/capture/owner operations,
   partial multi-file visibility, rebuild failure, retry, cancellation and restart.
   Keep ordinary activation disabled until those proofs and native gates close.

The self-device incoming-tail/allocator relationship is an explicit scope gate
before broader restore/public import support; accepting a signed tail alone is
not evidence that the local allocator is reconciled. The reviewer did not
establish a supported ordinary-M8 exploit for this concern.

Mac production ancestor-handle/no-replace/owned-endpoint parity, physical
filesystem/durability evidence, the development host's Windows TCP compatibility
case, later end-to-end offline/restriction integration, required CI and human
review remain open. Earlier native receipts retain their exact source scopes;
they are not native verification of this new correction. No repeat owner VM
exercise is required merely to reconfirm those earlier checkpoints.
