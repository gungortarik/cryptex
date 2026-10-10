# M4 final acceptance and CI coverage audit

**Date:** 2026-10-10
**Branch:** `m4/knowledge-engine`
**Reviewed tip:** `9ab75af43d122313b42d463b36209a4ee66ac0b4`
**Product-code baseline:** `b1c43e2c36534e360cb06fb5cacef5bf6fd210ee`
**CI coverage fix:** `9500e2f467565f7dad68dd72612ade7eb86b265f`
**Verdict:** technical acceptance evidence reviewed; final CI coverage correction
and human review remain open. M4 is not COMPLETE and is not merged.

## Review scope and authority

Read AGENTS, Master Spec, accepted ADR-0017/0018, the M4 milestone contract,
Definition of Done, implementation handoff, and M4 slice/security/platform
evidence. Checked the versioned reader, recovery compatibility boundary,
canonical writer, reducer normalization/cardinality, migration/rebuild,
retrieval guard, Desktop rendering, and CI invocation boundaries. Earlier
scoped security reviews remain the basis for the reducer/IPC security verdict;
this acceptance audit does not claim a new exhaustive security assessment.

The working tree was clean before fast-forwarding the local checkout from
`ebdd8c9` to the already-pushed documentation commit `9ab75af`.

## Acceptance mapping

| M4 requirement | Implementation and verification basis | Assessment |
|---|---|---|
| Frozen v1 bytes, schema and crypto | No branch delta to v1 envelope, integrity, validator, framing or v1 schema/TS artifacts relative to M3 merge `35538c1`; golden/strict-v1 tests in events | Covered |
| Strict v2 and compatibility classification | `versioned_reader.rs`, typed payload/schema tests, cross-version chain tests; compatibility boundary in canonical recovery preserves unsupported/unverifiable bytes | Covered |
| Deterministic consolidation and provenance | `cryptex-knowledge` replay, cardinality, duplicate provenance, explicit merge/revoke, correction/withdrawal and ambiguity tests | Covered locally on both OS at recorded slice SHAs; final CI invocation gap corrected below |
| Missing and late dependencies | Pending and safe-watermark tests; persisted late cross-device dependency output equals full rebuild | Covered by knowledge and daemon tests |
| Derived SQLite, independent schema, transactional rebuild | Additive migration preserves M3; idempotent rebuild, rollback, bound frame positions and interrupted-process reopen tests | Covered locally; interrupted-migration CI coverage corrected below |
| Offline, explicit authorization, confidence separation | Pure Rust reducer has no LLM/network extraction; trusted-daemon resolver boundary and receipt provenance are explicit; Master Spec confidence vocabulary preserved | Covered; no remote human-approval proof is claimed |
| Retrieval privacy and complete inventory | Fresh locked canonical scan, unknown stream/unsupported/pending/stale holds; exact projection comparison; Desktop guarded IPC allow/hold tests | Covered by daemon tests and prior security review |
| Desktop renderer safety | React text rendering; no raw HTML rendering added; request generation invalidates stale responses; hold/error clears results | Covered by source review and response-policy tests |
| Windows/macOS checks | Run `38021522882` succeeded for configured readiness/ACL jobs; exact-SHA platform slice evidence; native visual records | Existing coverage passes; missing final M4 CI steps must pass before closeout |
| Scope and production activation | Production writer still emits v1 `memory.note` only; no production v2 writer, trusted-device rollout or downgrade productization added | Production activation CLOSED; M5/M8/M10/M14 not authorized here |

## F1: required M4 test suite absent from final CI

The GitHub API confirms run
[38021522882](https://github.com/gungortarik/cryptex/actions/runs/38021522882)
completed successfully at `b1c43e2`: both readiness jobs and Windows IPC ACL
passed, and unsigned packaging was skipped by design.

However, its workflow ran events/storage/projection and full daemon tests,
but did not run `cargo test --locked -p cryptex-knowledge`. Cargo compiles
dependency libraries without running their unit tests. Consequently that run
did not execute the reducer/SQLite unit suite or the new interrupted-migration
test in `crates/knowledge/src/sqlite.rs`. Workspace Clippy is not test execution.
Desktop's response-policy tests were also absent from the workflow.

Fix: add the existing knowledge package test and Desktop `test:search-policy`
commands to the existing dual-OS readiness job. No production code, dependency,
canonical format, test assertion, job matrix or trigger was changed. The schema
drift command already checks both v1 and v2 bindings and needs no duplicate step.

Focused Windows checks after the workflow edit:

- `cargo test --locked -p cryptex-knowledge`: **35/35 PASS**, including
  `interrupted_migration_transaction_rolls_back_on_process_reopen`.
- `pnpm --filter @cryptex/desktop test:search-policy`: **2/2 PASS**.
- `git diff --check`: PASS.

The Cargo target was outside Git at
`%LOCALAPPDATA%/Cryptex-Dev/cargo-target-m4`. Fixtures were synthetic. No native
app, VM, OS security policy or personal/Cortex data was accessed in this audit.

One corrected dual-OS workflow is required to close this actual coverage gap.
This is justified by missing mandatory test execution, despite unchanged
production code; repeat runs without a failure or new gap are not required.

## Visual and security limits retained

- Windows visual verification is at `79e03d6`; subsequent Windows overlapped
  I/O hardening is covered by focused tests, security re-review and final CI.
  The renderer and tray interaction code have not changed since that visual
  check. Do not describe it as a new physical Windows visual run on `b1c43e2`.
- macOS retrieval-safety visual review is scoped PASS at documentation tip
  `ebdd8c9`, product-equivalent to `b1c43e2`. The fresh empty pilot held
  retrieval and showed no results. `pilot-quit.request` verified clean shutdown;
  actual status-item Quit remains NOT VERIFIED. M4 does not introduce tray
  behavior or list that interaction as a separate acceptance test.
- Empty-pilot hold is expected and does not demonstrate successful retrieval
  of populated history; synthetic allowed-result tests provide that coverage.
- IPC executable-path identity assumes the sibling install path cannot be
  maliciously replaced; it is not publisher-signature verification.
- A synchronous OS/SQLite call can retain a capped worker until it returns;
  rare Windows cancellation failures can delay safe I/O draining. These
  previously reviewed availability limitations do not authorize content leaks.
- Unknown remote devices remain fail-closed; ownership of the two test hosts
  does not enroll them as trusted peers. Prior OS-accessible plaintext cannot
  be revoked by daemon policy. Production v2 activation stays independently
  blocked until its accepted rollout/downgrade gate is implemented and tested.

## Remaining closeout steps

The concrete implementation diff and acceptance packet are available in draft
[PR #10](https://github.com/gungortarik/cryptex/pull/10), targeting
`m0/foundation`. The API reports it mergeable, which is a Git conflict check,
not M4 completion or approval. Human review remains OPEN.

The corrected workflow was dispatched once on review candidate
`f9bd8945151a44d450bd8340a6794a5f33df3e72`:
[run 38033047073](https://github.com/gungortarik/cryptex/actions/runs/38033047073).
The API confirmed that exact head SHA and both readiness jobs plus IPC ACL
started. Result is **PENDING**; unsigned packaging is skipped by design.

1. Record the correction commit and one dual-OS CI result including both new
   test steps. Until it passes, the final M4 CI acceptance gate is PENDING.
2. Present the concrete acceptance audit and diff for final owner/human review,
   required by Definition of Done section 11 and the M4 completion gate.
3. Only after those gates pass, record M4 completion and perform an authorized
   normal PR merge. Do not begin M5 or enable production v2 writing beforehand.
