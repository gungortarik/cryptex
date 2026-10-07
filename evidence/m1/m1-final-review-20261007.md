# M1 final review record — 2026-10-07

**Reviewer:** Codex, at the owner's request to perform the technical review
**Branch:** `m1/brain-device-identity`
**Reviewed source SHA:** `a39920564561f75ada08cba8a47244bec9074345`
**Pull request:** [#6](https://github.com/gungortarik/cryptex/pull/6), base `m0/foundation`
**Current-source CI:** [run 37569756071](https://github.com/gungortarik/cryptex/actions/runs/37569756071) — PASS

## Review scope and outcome

Reviewed the M1 handoff and authoritative Master Spec, DoD, accepted ADR-0014 and
ADR-0015, M1 milestone contract, scope/security audit, and implementation paths for
identity persistence, sequence allocation, SecureStore, pairing records, and Recovery
Kit public metadata. Checked the M1 boundaries against daemon-only writes, immutable
identity/canonical-data expectations, secret separation, typed fail-closed errors,
cross-platform persistence, and concurrent first-create/update behavior.

The review found one correctness issue and it was fixed before this result: sequence
counter loss followed by `initialize()` could recreate the counter at zero and permit
sequence reuse. No unresolved implementation blocker was found within the reviewed M1
scope after that correction. This review does not assert that later-milestone behavior
(production pairing/revocation, Recovery Kit container, replication, or production
distribution) is implemented.

## Finding resolved — sequence counter loss could permit reuse

`SequenceStore::initialize()` previously created a missing counter with
`last_issued = 0`. If a counter had existed, issued values, and was later lost, calling
`initialize()` could silently reset the allocator and reuse an earlier number. That
violated ADR-0015's fail-closed / no-reuse rule.

Fix at source commit `a39920564561f75ada08cba8a47244bec9074345`:

- Persist a per-device initialization marker durably before creating the first counter.
- If the marker exists while the counter is missing, return typed
  `SequenceStateMissing`; neither initialization nor allocation resets the sequence.
- Existing counters without a marker are recognized and backfilled, preserving their
  current high-water value rather than resetting it.
- Add regression coverage for deleted initialized counters and safe marker backfill.

## Verification

Local review checks completed before CI:

- `cargo fmt --all -- --check` — PASS
- `cargo check --locked -p cryptexd --tests` — PASS
- `cargo check --locked -p cryptexd --lib` — PASS
- Direct local test-binary/Clippy execution was blocked by Windows Smart App Control
  error 4551; no bypass was attempted.

Required current-source workflow result:

- Windows readiness — PASS
- macOS readiness — PASS
- Windows cross-account IPC ACL — PASS
- Unsigned Windows package — skipped by design
- Run conclusion — `success`, exact head SHA `a39920564561f75ada08cba8a47244bec9074345`

## Recommendation and gate status

**Technical review recommendation:** approve the reviewed M1 implementation for merge
to `m0/foundation`; the discovered sequence-reuse issue is fixed and required CI is
green.

**Owner acknowledgment:** OPEN. This record is an agent review and does not claim the
owner personally inspected the diff or accepted this recommendation. Per the M1 DoD,
do not mark M1 complete or start M2 until the owner acknowledges the review summary.
