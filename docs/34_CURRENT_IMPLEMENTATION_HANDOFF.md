# 34 — Current Implementation Handoff

**Snapshot date:** 2026-10-08
**Current milestone:** M4 - Knowledge Engine (ADR-0017 documentation acceptance integrated at `ee7e173`; Slices 1–4 and the Slice 5 reader/rebuild/retrieval-safety library are locally verified on Windows/macOS at their recorded SHAs; guarded Desktop retrieval integration plus UI hardening is at `652b4de`; Windows and macOS Rust/Tauri/frontend checks are recorded at their respective SHAs; IPC peer-auth hardening at `946642a459531247091e858c301fc7ea17835ff0` binds sensitive retrieval/enrollment to the live parent Desktop PID and expected sibling executable path (Windows retained process handle; macOS current-parent check). Follow-up `37924852ad2207f0ab618c6737e5e08f0a6907c8` moves authenticated Desktop connections off the listener, bounds Desktop connections at eight and expensive full-history searches at two, preserves a separate four-worker non-Desktop health pool, and applies one 15-second deadline across lock waits, verification, reduction and SQLite search; timeout returns a Brain-wide hold without partial hits. A further startup fix at `b6d652da07c175d086c625bfb498cf370dfc5fe2` keeps non-Desktop launches on health-only IPC while sensitive retrieval stays unavailable unless parent Desktop authentication succeeds. Windows `cryptexd` package tests pass 198/198; owner-reported macOS checks on the same code SHA pass (193 daemon package tests, crate tests, strict Clippy and Desktop check). Path identity is not code-signature identity and assumes the installed sibling executable cannot be replaced by an attacker; on that report, physical visual review and final DoD/security review remained open. The latest security re-review passes on `c2cf07b`; physical visual review, final DoD review and owner acceptance remain open)
**Latest security-hardening code:** `c2cf07bf0662589a9932c201f5e9e749e6a1013a` is pushed on `m4/knowledge-engine`. Windows fmt, knowledge 34/34, IPC 14/14, full cryptexd 199/199, strict Clippy, Desktop check, and diff check PASS. Owner-reported macOS verification on the exact SHA also passes: knowledge 34/34, cryptexd 194/194, fmt, strict Clippy, Desktop check, and diff check. The read-only security re-review found no remaining P1/P2 issue in the changed reducer/IPC code. This SHA adds cancellation-aware deterministic union-find/alias/revocation reduction and bounded search execution. Record: `evidence/m4/m4-security-hardening-cancellable-reducer-20261008.md`.
**Active branch:** `m4/knowledge-engine`, based on M3 merge commit `35538c144434e1dbfb97b47f89d2536b7646046a` (PR #9)
**M2 status:** COMPLETE within M2 scope after owner-delegated review and required verification; normal merge PR #8 integrated `m2/immutable-event-store` into `m0/foundation` at `817b6a1`.
**M3 closeout:** Complete within accepted scope; normal merge PR #9 integrated `m3/local-projection-search` into `m0/foundation` at `35538c1`. Final review: `evidence/m3/m3-final-review-20261008.md`.
**M4 architecture:** ADR-0017 was accepted and integrated at `ee7e173`; ADR-0018 local-only Desktop retrieval identity was accepted by the owner on 2026-10-08. Follow `docs/milestones/M4_KNOWLEDGE_ENGINE.md` in order. Slices 1–4 and the Slice 5 reader/rebuild/retrieval-safety library are Windows/macOS locally verified at their recorded SHAs. Explicit daemon-owned local DeviceId/key enrollment, read-only local key resolution, and `identity enroll-local` setup are implemented. Guarded Desktop search IPC plus renderer integration and empty/stale-query hardening are at code `652b4de` / branch `9eea611`; Windows and macOS frontend checks pass, with the latest macOS report at exact HEAD `9eea611` (`test:search-policy` 2/2, frontend build, `git diff --check`). Mac Rust/Tauri checks passed at `de1616c`; no Rust behavior changed after that report. The security review findings—caller-supplied Desktop role not bound to the connecting process and serial IPC framing without a total deadline—are addressed at `16cb39b` (Darwin enum/raw-FD compile correction is now included in the current candidate); non-Desktop health traffic has its own bounded four-worker pool; authenticated Desktop connections have eight bounded slots, with a separate two-worker search cap and one 15-second search deadline so slow searches do not block the listener or exhaust the health pool. Windows tests and strict Clippy pass; owner-reported macOS peer-credential/full parity passes at `946642a459531247091e858c301fc7ea17835ff0`. Desktop identity uses an expected sibling executable path, not signature/hash verification, and assumes install-directory protection. The scoped security review now passes at `c2cf07b`; physical visual review and final DoD/owner review remain open. Next: complete the M4 visual and owner acceptance gates, then run the single final Actions gate only on the reviewed candidate. Do not add M8 pairing/retirement, infer trust from the two machines' ownership, or enable production v2 writing. The future sync/import actor must join the canonical inventory-lock protocol. Phase-0 alternatives remain superseded audit records.
**M4 Slice 1 (historical status at initial landing):** code commit `2a382eb876d5646ff3e504dc3918423fa1594735` added the strict versioned reader and compatibility boundary. Windows checks and owner-reported macOS 27.0.1/arm64 checks passed at the recorded SHA. RetrievalSafetyState enforcement, which was open then, is implemented later through `ecbbaf7`; production v2 writing remains disabled.
**M4 Slice 2:** code commit `70dba01ff8fdb4018f6eb9be55d41943d300e90e` adds the strict typed `knowledge.assertion` v2 payload validator, separate generated v2 JSON Schema/TypeScript, and routes known v2 records to a typed supported classification. Windows and owner-reported macOS checks pass on this code SHA. Evidence: `evidence/m4/m4-slice2-typed-v2-payload-schema-20261008.md`. V1 artifacts and validator are unchanged; production v2 writing remains disabled.
**M3 Slice 1:** `evidence/m3/m3-slice1-workspace-scaffolding-20261008.md` - `cryptex-projection` path scaffolding + focused layout tests; no SQLite deps.
**M3 Slice 2:** `evidence/m3/m3-slice2-sqlite-fts5-pin-20261008.md` - `rusqlite =0.40.2` `default-features=false` + `bundled`; FTS5 fail-closed smoke; no schema/ingest.
**M3 Slice 3:** `evidence/m3/m3-slice3-projection-schema-v1-20261008.md` - empty schema v1 (`events` + text-only FTS5 + meta/checkpoint); open/create + schema-version validation; no ingest/search.
**M3 Slice 3 follow-up:** `evidence/m3/m3-slice3-fixture-schema-hardening-20261008.md` - exclusive temp fixtures + exact column/FTS shape validation; no ingest/search.
**M3 Slice 4:** `evidence/m3/m3-slice4-verified-feed-integration-20261008.md` - `for_each_verified_event` + transactional `project_verified_device_events`; no search/checkpoint advancement.
**M3 Slice 4 follow-up:** `evidence/m3/m3-slice4-idempotent-conflict-feed-errors-20261008.md` - identical replay idempotent; `EventConflict` on field mismatch; `ProjectionFeedError::Projection` preserved for insert failures.
**M3 Slice 5:** `evidence/m3/m3-slice5-rebuild-incremental-checkpoint-20261008.md` - rebuild + incremental catch-up; checkpoints in same txn; missing/corrupt/unsupported/checkpoint-invalid rebuild disposable only.
**M3 Slice 5 follow-up:** `evidence/m3/m3-slice5-replace-only-unusable-20261008.md` - `open_replacing_unusable` deletes only CorruptDb/UnsupportedSchema/SchemaInconsistent; Io/Sqlite/Fts5Unavailable preserve files.
**M3 Slice 5 blocking fix:** `evidence/m3/m3-slice5-device-canonical-lock-reader-20261008.md` - verified reader shares `writer.lock`; projection lock order device→SQLite; no concurrent recover/truncate during append.
**M3 Slice 5 lock follow-up:** `evidence/m3/m3-slice5-device-lock-visibility-boundary-handshake-20261008.md` - lock helpers `pub(crate)` + device-bound; boundary check before FS mutate; thread-scoped reader-at-lock handshake.
**M3 Slice 6:** `evidence/m3/m3-slice6-search-api-20261008.md` - `search_events` FTS + conjunction metadata filters; UTC `created_at` ranges; `InvalidQuery` for bad FTS/bounds.
**M3 Slice 6 follow-up:** `evidence/m3/m3-slice6-fractional-instant-precision-20261008.md` - exact UTC instants (incl. fractional seconds) for search range + ordering via `rfc3339_utc_instant`; no new stored column.
**M3 Slice 7:** `evidence/m3/m3-slice7-cross-platform-ci-wire-20261008.md` - CI readiness wires `cryptex-projection` only; `cryptexd` `projection::` covered by existing full `cargo test -p cryptexd` (duplicate filtered step removed); sealed/open + multi-device + idempotent rebuild matrix coverage; Windows-local PASS.
**M3 Slice 7 macOS four-check:** `evidence/m3/m3-slice7-macos-four-check-20261008.md` - owner macOS 27.0.1 arm64 four-check PASS at exact HEAD `428b03f33eda8fbf645678220dbd3ce3d88d108e` (fmt; projection 35; cryptexd `projection::` 14; Clippy `-D warnings`). Both OS local checks PASS at that tip.
**M3 Slice 7 tip (both-OS local PASS):** `428b03f33eda8fbf645678220dbd3ce3d88d108e` (behavior through CI redundancy cleanup; Windows + macOS local four-checks recorded)
**M3 final dual-OS CI:** PASS in run [37720627242](https://github.com/gungortarik/cryptex/actions/runs/37720627242) at exact branch tip `51e6dc772757510e08c14ef565a87534a8a9478b` (documentation-only after tested behavior SHA `428b03f`). Windows and macOS readiness plus Windows IPC ACL PASS; unsigned package skipped by design. Record: `evidence/m3/m3-final-ci-gate-20261008.md`.
**Historical M3 Slice 8 initial tip (Windows-local at the time; superseded):** `10a11c82b0e07e09fd7ef97c2384e9a385991adc`
**Historical M3 Slice 8 follow-up tip (Windows-local at the time; superseded):** 04c794f907704455cea2463f13cbce83635940ff
**Historical M3 Slice 8 constraint-class tip (Windows-local at the time; superseded):** `7043c1f8633beebe5f43dfc313e7d72a8c682366`
**M3 Slice 8 constraint-class fix:** `is_constraint_rejection` is ConstraintViolation-only (no message substring).
**M3 Slice 8 verified candidate:** `275fb7acf0c2c7b4d7c050acb54670e81520377f`; Windows and owner macOS local checks PASS; final dual-OS workflow [37725223487](https://github.com/gungortarik/cryptex/actions/runs/37725223487) PASS. Evidence: `evidence/m3/m3-slice8-macos-four-check-20261008.md` and `evidence/m3/m3-final-ci-gate-20261008.md`.
**Historical M3 Slice 8 meta-CHECK tip (Windows-local at the time; superseded):** 52fe953ecdb29db1f1730148921f1af07be5f50c
**M3 Slice 8 meta-CHECK hardening:** structural `id=1` parse + multi-id constraint-only probes; non-constraint probe errors non-replaceable; savepoint cleanup errors preserved. See evidence follow-up.
**M3 Slice 8 follow-up:** FTS integrity-check maps only confirmed corruption to replaceable `CorruptDb`; semantic `projection_meta` id CHECK probe rejects permissive constraints. See evidence follow-up section.
**M3 Slice 8 (security/DoD findings):** `evidence/m3/m3-slice8-security-dod-findings-20261008.md` — FTS5 external-content `integrity-check` (rank=1) on open; empty-device checkpoint incremental catch-up; schema v1 type/PK/CHECK validation; bounded clear + limited search; error classifications use SQLite result codes only. Windows checks and owner macOS four-check PASS at exact HEAD `4a2ea56`; final dual-OS CI PASS in run `37780668640`. Scoped security/DoD and owner-delegated final review PASS are recorded in `evidence/m3/m3-final-review-20261008.md`; M3 completion gate is PASS within scope.
**Latest verified candidate SHA:** `9ade9f7db18ec0e383594c19e592809e3b194531` (`9ade9f7`), on `m2/immutable-event-store`.
**Final dual-OS CI:** PASS in run [37706283147](https://github.com/gungortarik/cryptex/actions/runs/37706283147) against the exact candidate SHA above. macOS and Windows readiness plus Windows IPC ACL succeeded; unsigned Windows package skipped by design. Record: `evidence/m2/m2-final-ci-gate-20261008.md`.
**Closeout audit tip (L1/L2/F4–F6 + staging ownership):** behavior `b5c9d69`, tip `1a26493` — `evidence/m2/m2-staging-cleanup-ownership-20261007.md`. Windows checks PASS; owner-reported macOS four-check PASS at `9ade9f7` is recorded in the final-gate evidence. Owner-delegated closeout review PASS is recorded at `evidence/m2/m2-owner-delegated-review-20261008.md`; PR #8 integration completed at `817b6a1`. `LongPathsEnabled=0` is not claimed.
**Base:** `origin/m0/foundation` @ `817b6a126cf6e9cd3e8a27b1348ec977bdc9dcdd` (M2 merge / PR #8)
**M1 merge:** `c2168f3edbd4deb00d5e16f5f35643948028b0ca` (PR #6) — integrated ancestor
**M1 implementation source:** `a39920564561f75ada08cba8a47244bec9074345`
**M1 documentation closeout:** `0c84c5c6a2872480ac4bbc82ebb1541773ddfb75`
**M3 Slice 8 corruption-class candidate:** code SHA `6b61882dce0814abd51fbf063c69d7775c483e37`; exact branch HEAD with docs stamps `4a2ea56aa2ddc4024d82599f8dc671472179cbe8`. Windows local checks PASS; owner macOS four-check PASS at exact HEAD; final dual-OS Actions PASS in [run 37780668640](https://github.com/gungortarik/cryptex/actions/runs/37780668640). Evidence: `evidence/m3/m3-slice8-security-dod-findings-20261008.md`, `evidence/m3/m3-slice8-macos-four-check-20261008.md`, and `evidence/m3/m3-final-ci-gate-20261008.md`.
**M3 closeout:** Completion gate PASS within accepted scope, recorded in `evidence/m3/m3-final-review-20261008.md`. Normal merge PR #9 integrated M3 into `m0/foundation` at `35538c1`.
**Historical M4 Phase 0 records:** `evidence/m4/m4-phase0-kickoff-plan-20261008.md` and `evidence/m4/m4-phase0-input-and-semantic-decision-review-20261008.md`. Their open alternatives are superseded by accepted ADR-0017; see their supersession notices.

> This file is an operational status snapshot, not an architecture authority. If it conflicts with `docs/00_MASTER_SPEC.md`, accepted ADRs, the milestone contract, or other authoritative project documents, stop and report the conflict instead of silently reconciling it.

## Required reading before continuing

1. `AGENTS.md`
2. `docs/00_MASTER_SPEC.md`
3. `docs/milestones/M3_LOCAL_PROJECTION_SEARCH.md`
4. `docs/03_MEMORY_MODEL.md`
5. `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`
6. `docs/06_STORAGE_ARCHITECTURE.md`
7. `docs/17_TEST_AND_FAILURE_MATRIX.md`
8. `docs/21_DEFINITION_OF_DONE.md`
9. relevant accepted ADRs, including ADR-0013, ADR-0016, and ADR-0017 for canonical-event reads and M4 materialization
10. `evidence/m3/m3-kickoff-plan-20261008.md` and M2 closeout records under `evidence/m2/`

## Working agreement

### M2 closeout audit (L1/L2/F4–F6) — 2026-10-07

Source re-audit of previously deferred Low findings (see `evidence/m2/m2-closeout-l1-l2-f4-f6-20261007.md`):

- **L1 FIXED:** corrupt sealed `.seg` isolated from canonical tree only after durable quarantine; repeated load does not re-quarantine.
- **L2 NON-BLOCKER:** Desktop links `cryptexd` but only health IPC + sidecar process; Cargo features cannot guarantee isolation under workspace unification; Master Spec single-writer preserved by process/IPC/call sites.
- **F4 partial:** sealed integrity errors use `source_relative`; residual lifecycle absolute paths NON-BLOCKER.
- **F5 NON-BLOCKER:** quarantine containment by construction under `quarantine/canonical-events/`.
- **F6:** staging orphan cleanup FIXED; canonical `\\?\` remains addendum non-goal NON-BLOCKER.

Windows: `cryptex-storage` quarantine 24 ok; `cryptexd --lib` 126 ok; fmt check ok; clippy `-D warnings` on storage+cryptexd ok. **macOS four-check PASS** at source `9ade9f7`: quarantine 22, cryptexd lib 121, fmt and strict Clippy all passed on macOS 27.0.1 arm64. Final dual-OS CI run [37706283147](https://github.com/gungortarik/cryptex/actions/runs/37706283147) also PASS at exact SHA `9ade9f7`; see `evidence/m2/m2-final-ci-gate-20261008.md`. Owner-delegated closeout review PASS; M2 is complete and integrated via PR #8 at `817b6a1`. M3 Phase 0 plan is complete on `m3/local-projection-search` (`evidence/m3/m3-phase0-implementation-plan-20261008.md`); product implementation not started.

**Staging ownership fix (2026-10-07):** Arm `StagingCleanup` only after exclusive `create_new`; surface cleanup failures as `QuarantineCleanupFailed` without dropping the primary. Evidence: `evidence/m2/m2-staging-cleanup-ownership-20261007.md`. Windows quarantine **24** ok; cryptexd lib **126** ok; fmt/clippy ok. Mac four-check PASS and final dual-OS CI PASS are recorded in `evidence/m2/m2-final-ci-gate-20261008.md`.




**Historical M1 follow-up note (2026-10-07; superseded for owner-acceptance status):** Codex final review found fixed-temp races in
pairing and Recovery Kit public metadata persistence and corrected them with
serialized, synced atomic replacement. Sequence counter device-id validation now
also covers reads/initialization, not only allocation. Windows tests/format PASS; local Clippy
was blocked by Smart App Control (4551). Current-source cross-platform CI is
PASS at `a39920564561f75ada08cba8a47244bec9074345`, run 37569756071, including
strict workspace Clippy on both OSes. See
`evidence/m1/m1-public-metadata-final-review-20261007.md` and the newer
`evidence/m1/m1-final-review-20261007.md`. **Historical wording (superseded):** this paragraph originally said "Owner acknowledgment remains OPEN." That status is obsolete: the owner accepted the delegated Codex review on 2026-10-07; see the later M1 review update and `evidence/m1/m1-final-review-20261007.md`.

**Latest M1 review update (2026-10-07):** A final Codex review found that loss of
an initialized sequence counter could permit reuse after `initialize()` recreated
it at zero. Fixed at source `a39920564561f75ada08cba8a47244bec9074345` by committing
a durable per-device initialization marker before the initial counter; marker plus
missing counter now fails closed. Current-source final CI run
[37569756071](https://github.com/gungortarik/cryptex/actions/runs/37569756071) is
PASS (Windows/macOS readiness + Windows IPC ACL; unsigned package skipped by design).
Review record: `evidence/m1/m1-final-review-20261007.md`. The owner accepted the
delegated review on 2026-10-07; PR #6 was normally merged into `m0/foundation` as
`c2168f3`. M1 is complete within its defined scope. M2 has not started.

**Current status (2026-10-07):** M0 Foundation is COMPLETE (PR #5). M1 — Brain +
Device Identity is COMPLETE and integrated into `m0/foundation` at merge commit
`c2168f3edbd4deb00d5e16f5f35643948028b0ca` (PR #6). The behavior-bearing source
candidate `a39920564561f75ada08cba8a47244bec9074345` passed required Windows/macOS CI;
subsequent M1 commits were documentation-only review/acceptance records. Cursor was
the write-capable M1 implementer; the owner accepted Codex's delegated final technical
review on 2026-10-07.
**ADR-0015 is ACCEPTED**; **ADR-0014 is ACCEPTED** (2026-10-06) selecting **P1**
(direct platform APIs) with product pins `windows-sys = 0.61.2`,
`widestring = 1.2.1`, `security-framework = 3.7.0`. K2 and high-level `keyring`
are unselected. Gate C **Windows** and **macOS** product SecureStore are VERIFIED
(distinct host evidence under `evidence/m1/`). Required CI **PASS** on implementation
candidate `a399205`: run [37569756071](https://github.com/gungortarik/cryptex/actions/runs/37569756071)
(Windows readiness, macOS readiness, Windows cross-account IPC ACL; unsigned Windows
package job skipped by design). The owner accepted Codex's delegated technical review
on 2026-10-07; all M1 gates are satisfied and PR #6 is merged. The unsigned Windows
package was intentionally not part of the M1 gate.

**M1 status:** ADR-0015 ACCEPTED (2026-10-05). ADR-0014 ACCEPTED (2026-10-06, P1).
Identity/sequence, SecureStore (trait/fake + Windows/macOS P1 adapters), synthetic
Brain daemon wiring, pairing-record persistence, and Recovery Kit public metadata
persistence are in place under cryptexd. Gate C Windows and macOS product SecureStore
are both VERIFIED (2026-10-06; distinct host evidence files under `evidence/m1/`).
Gate A spike remains historical comparison only. Prior Windows tests, strict Clippy and format
for the install-identity staging/rollback follow-up pass locally (94 cryptexd tests);
The latest metadata and sequence corrections are verified by both-OS CI; Unix-specific
install rollback-sync tests and Windows/macOS readiness passed in CI run
37569756071. Codex final review and owner acceptance are recorded; PR #6 is integrated.
Production pairing/revocation remains M8; Recovery Kit container remains M11.
No custom cryptography.

**M1 audit (2026-10-06):** Sequence allocation and install identity are daemon-owned
(`evidence/m1/m1-scope-security-audit-20261006.md`). Brain public identity first-create is exclusive/no-replace; install-identity staging
cleanup and Unix rollback durability now match that hardened path (see audit). Final CI
PASS on candidate `a399205` is recorded (run 37569756071). The accepted M1 lifecycle and placement boundary is documented in
ADR-0015 §2.1.1 and the audit. The owner accepted the delegated Codex review; PR #6 is
merged. M1 is complete; M2 has not started.

**M2 closeout record (2026-10-08):** M2 productization was completed and integrated via PR #8 at `817b6a126cf6e9cd3e8a27b1348ec977bdc9dcdd`. The implementation is documented below as historical milestone context.
`m2/immutable-event-store`, based on `origin/m0/foundation` at `0c84c5c` (M1
documentation closeout). M1 merge `c2168f3` is an ancestor of the latest final-CI behavior tip `9ade9f7`. Earlier final-CI behavior tip `16f09a3` and its documentation closeout `7a84bdd` are historical; the latest gate is recorded in `evidence/m2/m2-final-ci-gate-20261008.md`. Owner-delegated closeout review PASS; M2 awaits normal integration. Cursor is
the sole write-capable M2 implementer. ADR-0016 is **ACCEPTED** (2026-10-07) at
`5cc89b709dcf236698c3253b8582c873de7c7f0f`. Slice 1 scaffolding and framing body_len enforcement are complete. Slice 2 event domain types +
schema/TS generation is recorded under `evidence/m2/m2-slice2-event-types-schema-20261007.md`; integrity primitives under `evidence/m2/m2-slice3-integrity-primitives-20261007.md` signing-key lifecycle under `evidence/m2/m2-slice4-signing-key-lifecycle-20261007.md` ; segment I/O under `evidence/m2/m2-slice5-segment-io-quarantine-20261007.md` ; durability hardening under `evidence/m2/m2-slice5b-storage-signing-hardening-20261007.md` ; canonical writer under `evidence/m2/m2-slice6-canonical-writer-20261007.md` ; writer transaction hardening under `evidence/m2/m2-slice6b-writer-transaction-hardening-20261007.md` ; schema validation/quarantine matrix under `evidence/m2/m2-slice7-schema-validation-quarantine-matrix-20261007.md` ; seq-gap/streaming-quarantine/framing reasons under `evidence/m2/m2-slice7b-seq-gap-streaming-quarantine-framing-20261007.md` ; cross-platform matrix + CI wiring under `evidence/m2/m2-slice8-cross-platform-matrix-ci-wiring-20261007.md` ; Clippy CI fix after failed run 37648931640 under `evidence/m2/m2-slice8b-clippy-ci-fix-20261007.md` ; macOS unused-`io` fix after partial-green run 37650566598 under `evidence/m2/m2-slice8c-macos-clippy-unused-io-20261007.md` ; Windows process-identity lifetime sync after near-final run 37653004440 under `evidence/m2/m2-slice8d-windows-process-identity-sync-20261007.md` ; pilot-runtime fixture uniqueness after macOS fail on run 37656380940 under `evidence/m2/m2-slice8e-pilot-runtime-fixture-uniqueness-20261007.md` ; ADR-0016 §8.1 quarantine directory format restored; Windows FS path-ops + per-device AcceptedEvent DeviceId binding under `evidence/m2/m2-slice8f-windows-path-ops-and-device-binding-20261007.md`; owner-approved Windows quarantine extended-path adapter (`\\?\` I/O only; no registry/`LongPathsEnabled` dependence; §8.1 names + DeviceId policy unchanged) under `docs/adr/ADR-0016-windows-quarantine-extended-path-addendum.md` (ACCEPTED) and `evidence/m2/m2-slice8g-windows-quarantine-extended-path-20261007.md`; regression tightened so >MAX_PATH existence/read/cleanup required checks use prefixed adapter paths only under `evidence/m2/m2-slice8h-extended-path-regression-tighten-20261007.md`; pre-Actions security/DoD review + in-scope fixes under `evidence/m2/m2-security-dod-review-20261007.md`; macOS CI `prepare_m1_runtime_foundation_*` fixture race fixed under `evidence/m2/m2-slice8i-install-runtime-temp-root-race-20261007.md`; full cryptexd `--lib` temp-root hardening under `evidence/m2/m2-slice8j-cryptexd-lib-temp-root-hardening-20261007.md` (shared `test_temp` exclusive allocator across identity/pairing/recovery/sequence/etc.). **Latest final dual-OS CI gate PASS** on tested SHA `9ade9f7db18ec0e383594c19e592809e3b194531`: run [37706283147](https://github.com/gungortarik/cryptex/actions/runs/37706283147) — macOS readiness **success**, Windows readiness **success**, Windows IPC ACL **success**, unsigned Windows package **skipped** by workflow design. Mac host four-check PASS at the same SHA is documented in `evidence/m2/m2-final-ci-gate-20261008.md`; run 37681630801 is historical. Owner-delegated closeout review PASS is recorded in `evidence/m2/m2-owner-delegated-review-20261008.md`; normal PR #8 integration into `m0/foundation` completed at `817b6a1`. Security/DoD dispositions remain documented in the existing audit records. `LongPathsEnabled=0` is not claimed. M2 is COMPLETE within scope and integrated at merge commit `817b6a1`; M3 Phase 0 plan is complete on `m3/local-projection-search` (`evidence/m3/m3-phase0-implementation-plan-20261008.md`); product implementation not started.
Do not freeze `segment_events=1000` as a production constant. Do not expand into
M3/M8/M10 product behavior. Do not dispatch further Actions unless the owner requests them.


**Historical M0 closeout (retained):** Final behavior-bearing source CI was
green at `c7fa011` (run `37387323751`); Mac embedded `.app` visual review
scoped PASS; scoped ADR-0012 and ADR-0004 accepted. Older continuation entries
below remain dated history. Prefer this current status and the ADR index for
present-day decisions.

- Work only on the current milestone (M4). Do not opportunistically implement M5 Quick Capture, M6/M7 Obsidian behavior, M8 replication, M10 purge, or M13 broad Desktop product UI.
- Follow the accepted M4 contract and authoritative memory/entity specifications. Knowledge materialization must remain derived/rebuildable; canonical history stays immutable and authoritative.
- If the user's intent about tool roles, workflow, architecture, branch strategy, milestone scope, or integration strategy is ambiguous, ask before changing direction.
- M3 final candidate `4a2ea56` has Windows-local checks PASS, owner-reported macOS four-check PASS, and final dual-OS Actions PASS in run `37780668640`; normal PR #9 merge is `35538c1`.
- M4 implementation is explicitly authorized after ADR-0017 documentation acceptance at `ee7e173` and follows `docs/milestones/M4_KNOWLEDGE_ENGINE.md`. Historical references here to the Slice 3 candidate `45c78fd` and the need to run macOS before Slice 4 are superseded by later recorded Slice 3/4/5 verification. Code SHA `c2cf07bf0662589a9932c201f5e9e749e6a1013a` has Windows and owner-reported macOS local verification; the latest read-only security review passes. Remaining gates are physical UI review, final DoD/owner review, and one final Actions run on the reviewed candidate. Do not enable v2 production writing or implement M8/M10/M14 opportunistically.
- Do not merge, rebase, switch branches, amend, force-push, or rewrite history without explicit approval.
- Runtime data must remain outside the source repository.
- GitHub Actions minutes are constrained. Prefer relevant local checks; run only the required final M4 workflow after its implementation candidate is reviewed.
- Do not touch retained P0-1 corpora unless the current approved test requires it.
- Never label behavior VERIFIED unless it was actually physically/test verified.
- Do not freeze the P0-1 benchmark profile value `segment_events=1000` as a production constant.
- **Historical M1 working-agreement residue (superseded):** earlier text said "Work only on the current milestone (M1)" and "normal PR #6 merge remains before recording M1 integrated." M1 is complete and merged at `c2168f3`; documentation closeout is `0c84c5c`. Owner acceptance of the delegated M1 review is recorded at `evidence/m1/m1-final-review-20261007.md`.
- ADR-0013, ADR-0014 (P1), ADR-0015, ADR-0016, and ADR-0017 remain ACCEPTED (ADR-0016 at `5cc89b709dcf236698c3253b8582c873de7c7f0f`; ADR-0017 accepted 2026-10-08). M2 and M3 are complete. M4 implementation is underway only on the authorized branch/slice; the v2 production writer has a separate closed activation gate.

## P0-1 status

P0-1 physical event packaging is complete and ADR-0013 is accepted.

Decision:
- canonical physical packaging uses immutable sealed per-device segment/pack files
- one-file-per-event was rejected at scale
- hybrid packaging was not selected
- the benchmark profile's `segment_events=1000` is not a frozen production constant

Integration update:
- the owner explicitly approved normal merge of `origin/m0/p0-1-cursor`
- merge `cb9069e` integrates accepted P0-1 tip `6d4d10a`
- ADR index conflict preserves accepted IPC, proposed lifecycle and accepted
  ADR-0013 together; no retained corpus changed

## M0 closure continuation (2026-10-05)

Mac native renderer was initially **FAIL**, entirely blank in checkout `dee43e2` /
retained raw Cargo release `7f5ab2b`; that failure remains preserved. A fresh
embedded Tauri `.app` at checkout `e755601` has now produced a **SCOPED PASS**:
rendered operator captures and clean owned-session shutdown. See
`evidence/m0/macos-embedded-frontend-visual-evidence-20261005.md`.
Pinned Tauri frontend mode depends on `tauri/custom-protocol`, not Rust release
optimization. Raw release in the old plan omitted it and can use devUrl port 1420.
Source `65ce7b9` adds typed pre-Tauri wrong-mode release rejection and explicit
embedded-frontend CI build/test/lint (full release suite). Independent focused
security/verifier review PASS, including release feature-omission sentinel.
Windows 23 debug / 24 release / 4 CLI, strict lint/format/package build, wrong-mode exit 2
and supported hidden session/clean exit 0 PASS. No new visible Windows UI or Actions.
Evidence: `evidence/m0/windows-embedded-frontend-mode-evidence-20261005.md`.

Mac scoped plan completed: 16 debug / 17 release / 4 CLI, targeted rejection and
fresh-package visual review reported PASS. Raw Mac artifacts were not copied for
independent re-hashing. Final CI **PASS** at
`c7fa011672973101cb2a5519b5883b26c6909f90`, run `37387323751`; see
`evidence/m0/m0-final-ci-attempt-20261005.md`. Scoped ADR decisions, human
closeout review and final checklist reconciliation are complete. M0 is complete
for the synthetic foundation scope; no merge/M1 is inferred.

### Earlier candidate CI and review state

Corrected final CI **PASS** on `5c074115cd61ef7c1eb47e8ed2cf8c1105360679`,
run `37377520994`: Windows readiness **93 debug Rust + 1 release resolver,
160/160 + 8/8 Node**; Mac readiness **75 debug Rust + 1 release resolver,
162/162 + 8/8 Node**; actual Windows cross-account ACL PASS. All required jobs
green; package-only installer intentionally skipped. Exact jobs/log provenance
and first failed/cancelled attempt are retained in
`evidence/m0/m0-final-ci-attempt-20261005.md`.
At that earlier candidate, only missing plain benchmark build prerequisite was
added. Subsequent result-recording commits were documentation only. New `65ce7b9`
changes behavior/workflow; the fresh final CI at `c7fa011` verifies that fix.
Independent verifier and security audit completed at `3de9d00` after owner
explicitly authorized both agents with "hemen basla". Results:
`evidence/m0/m0-independent-review-results-20261005.md`. No new blocking M0
implementation/security defect established; candidate CI independently verified.
Stale pending-documentation finding is reconciled. Five production residuals
remain mandatory, including file-controlled developer launch and buffered REST.
Windows native foundation UI review now scoped PASS: actual PID19376/1116,
matching renderer health.pong, readable wrapping/reachable controls and retained
screenshots; real Quit clean exit0, processes/lock/IPC/gate clean. Owner coordinated
left-display placement. See `evidence/m0/m0-foundation-ui-review-20261005.md`.
Historical before corrected package revalidation: **Mac-only** narrow native foundation visual review under
`evidence/m0/m0-native-foundation-ui-review-plan-20261005.md`; do not repeat old
Windows/Mac login/tray/upgrade suites. Then explicit human diff/ADR acceptance
and final-source CI/checklist reconciliation. Windows proof is not Mac visual proof.
Mac attempt at `9752798` remains **NOT VERIFIED visually**: Computer Use
`Invalid app`, no screenshot/layout/scrolling proof. Supplied retained build
`7f5ab2b` has matching relevant application source; isolated Desktop/daemon
94503/94513 health and clean exit0/lock/gate/IPC cleanup reported. Record:
`evidence/m0/macos-native-foundation-ui-attempt-20261005.md` (Mac-local artifact
not directly inspected on Windows). Continue the plan's **operator-screenshot
fallback**, fresh owned pilot, two actual Mac window captures and image-file
review. No old suite/build/Actions repetition or security setting changes.
Existing PR #5 targets m0/foundation; do not change integration/base, merge,
accept ADRs, complete M0 or start M1 without the required concrete approval.
No redundant CI dispatch requested; avoid disturbing the owner's game.

### Historical pre-review continuation

Final CI attempt at `e337bd3`, run `37376952961`, found missing plain
`m0-storage-bench` before Golden Node tests on a fresh Mac runner (ENOENT).
Workflow now builds it alongside the lifecycle helper; Windows focused Golden/
Doctor tests **6/6** and exact prerequisite build PASS. No assertions changed.
Windows ACL job passed; Windows readiness cancellation was requested to avoid
the same missing prerequisite. See `evidence/m0/m0-final-ci-attempt-20261005.md`.
At that first attempt, final CI was NOT VERIFIED. The corrected-source run
subsequently passed as recorded above; the earlier failure remains historical evidence.
Independent verifier/security review and human diff/ADR acceptance remain open.
Review packet: `evidence/m0/m0-human-acceptance-review-packet-20261005.md`.

Latest Mac namespace parity reported **PASS** at
`7f5ab2bdaa72e213f5cb16312bca0292c436db18`: macOS 27.0.1 arm64,
Rust/Cargo 1.98.1, Node v24.19.0. Desktop **14/14 debug + 14/14 release**, CLI
**4/4**, both strict Clippy profiles/format/locked release/diff PASS; all exits 0,
no failures/skips/ignored, cleanup error-free. Compiled defaults use Cryptex-Dev
in both profiles; no app/daemon, runtime/startup/config or repo changes.
Evidence: `evidence/m0/p0-2/macos-release-probe-root-evidence-20261005.md`.
This superseded pending namespace parity below. Corrected-candidate implementer
review completed for the narrow M0 boundaries with production residuals retained.
Next: one final CI run at the clean pushed candidate, then verifier/human diff
review and explicit ADR acceptance. No new Mac physical plan is requested.
ADR-0012/0004 remain PROPOSED and M0 IN PROGRESS.

Latest correction: `ad44e82` moves ordinary M0 release probe roots to
Cryptex-Dev/runtime/p0-2 on both OSes; `fc79f25` fixes a discovered release-only
strict Clippy warning without changing startup-error behavior. Windows final
Desktop tests **21/21 debug + 21/21 release**, both strict all-target lint
profiles, format/release build and negative CLI **4/4** PASS. No old runtime,
installed binary, registration, global config or visible app changed.
Evidence: `evidence/m0/p0-2/windows-release-probe-root-evidence-20261005.md`.
New narrow Mac task: `evidence/m0/p0-2/macos-release-probe-root-test-plan-20261005.md`,
expect **14/14 debug + 14/14 release + 4/4 CLI**. No previous plan rerun.
Final CI now covers release resolver regression and release-only strict lint;
do not dispatch until this new Mac parity and corrected-candidate review pass.
Historical audit finding below is corrected in code, not yet cross-platform
verified. Final checklist/ADR/human acceptance remain open.

Final implementer gate audit at source `8437559`:
`evidence/m0/m0-final-gate-audit-20261005.md`, **NOT READY for final CI**.
Retained foundation/update/notice-documentation proofs now fill previously empty
checklist evidence; no ADR or final gate is accepted. Two retained Windows native
report hashes still match. Genuine remaining implementation finding: ordinary
M0 release Desktop defaults to product `Cryptex/runtime/p0-2` on both OSes,
while development/pilot fixtures use Cryptex-Dev. Correct the ordinary M0 release
namespace, add path regressions and collect narrow Mac parity before final CI.
Do not mutate/delete old runtimes, login settings or historical evidence.
Security/verifier review remains incomplete until this correction is reviewed.
No previously passed Mac runtime/Syncthing plan needs repetition.

Latest Mac sidecar regression is scoped **PASS**, reported by the operator/Codex
at `388e46a7e5f750d88e7fc786e0c108dcea5329e4`: macOS 27.0.1 arm64,
Rust/Cargo 1.98.1, Node v24.19.0, pnpm 11.28.0. Desktop **13/13**, pre-Tauri
CLI **4/4**, format/strict Clippy/frontend/locked release/diff all PASS; every
command exit 0, no failures/skips/ignored, fixture cleanup error-free.
Backend-selected path and developer override authority plus real POSIX symlink
refusal passed. No visible app/daemon, edits, commits or Actions; repo clean.
Evidence: `evidence/m0/p0-2/macos-sidecar-boundary-evidence-20261005.md`.
This closes the new Mac validator regression, not physical renderer invocation.
No passed Mac plan requires repetition.

Owner scope decision recorded: "kurallar dahilinde onayliyorum" authorizes
preparing the scoped ADR-0012/0004 revisions in
`evidence/m0/m0-acceptance-scope-decision-proposal-20261005.md`.
Both proposals now separate M0 direction acceptance from mandatory production
gates: M1 secure storage/identity, M2 event semantics, M8 replication, M14 installed
updater coordination and M17 signed release/final notices. No production gate is
reported verified; the Master Spec and accepted milestone contract are unchanged.
Next: review these concrete revisions and the retained security/notice residuals,
then run required final CI once on the resulting stable candidate. Human diff/ADR
acceptance and the evidence-backed full M0 checklist remain required.
ADR-0004/0012 remain PROPOSED; M0 remains IN PROGRESS and M1 is not authorized. *(Historical M0-era note — superseded: M0 complete, M1 active, ADR-0015 ACCEPTED; see top.)*

The owner explicitly authorized completing the entire M0 milestone. Closure
audit: `evidence/m0/m0-closure-audit-20261005.md`. P0-4 managed Syncthing and its
ADR-0004 addendum remain unaccepted; managed staged lifecycle now has both-OS
proof, including the exact 2.1.2 -> 2.1.5 version pair; acceptance review remains open.
P0-1 transport evidence alone does not close them.
Do not declare M0 complete or start M1 based only on the P0-2 pilot.

Native interruption `7d7a60e` adds schema-2 intent ownership, absent-child checks
and exclusive recovery claim. Live/ambiguous PID, missing evidence and previous
recovery claim refuse mutation/startup. A killed updater at the selected but
pre-successor-launch boundary released the native gate; actual baseline restart
and protocol-1 health PASS. Interrupted recovery claims remain inspection-only.
Golden/Doctor foundation `7daadaa` generates deterministic synthetic sealed packs
with the accepted P0-1 codec and checks actual IPC/PID/held-lock health, retaining
`not-implemented` for canonical store/Syncthing checks. Fixture membership/hashes
were unchanged across real recovery. No M2 semantics were added.

Windows integrated checks: Rust **89/89**, lifecycle/foundation Node **123/123**,
storage capture Node **5/5**, workspace format/strict Clippy/locked build PASS.
The first workspace run found two parallel Mode B tests competing for the one
user IPC endpoint; their lifetimes now serialize without removing assertions.
Historical failure is preserved. Actual recovered pair `30972/29584`, Doctor
healthy, clean exit 0, lock released and endpoint absent. Evidence:
`evidence/m0/p0-2/windows-native-interruption-evidence-20261005.md`.
Combined Mac plan:
`evidence/m0/p0-2/macos-native-interruption-test-plan-20261005.md`.
Mac native interruption/Golden/Doctor parity was reported PASS at exact source
`502abf409925b368ff1377380c719069207f0d61`: Rust **71/71**, Node **123/123**
plus storage **5/5**, format/strict Clippy/build/package/syntax/diff PASS.
Updater `82701` was killed before candidate launch; old pair `82711/82719`
had exited cleanly. Recovery pair `82733/82740` used the correct baseline paths
and parents, protocol-1 health and Doctor healthy. Final exit 0 released all
locks/gates/IPC/intent/claim; synthetic sealed fixtures remained unchanged.
User-reported artifact hash and scope:
`evidence/m0/p0-2/macos-native-interruption-evidence-20261005.md`.
No Actions or global Syncthing/startup changes.

P0-4 continuation `7ff485d` adds an external pinned **2.1.5** native controller
and actual two-profile Windows runner. Auth rejection 403/403, pairing, native
history, exact two-content conflict preservation, same-version restart/new
transfer and four clean monitor exits PASS. An initial 8-second stop bound was
shorter than upstream's native 10-second connection close; source-based 25-second
budget now passes immediate restart shutdown at ~10.1 seconds without removing
exit/PID/database checks. Historical forced-cleanup failures remain recorded.
Combined Node **130/130** plus storage **5/5**, changed syntax/diff PASS.
Evidence and explicit Mac task are under `evidence/m0/p0-4/`.
ADR-0004 addendum is **PROPOSED**, not accepted. Notice inventory is retained
under `third_party/syncthing/2.1.5/`; actual bundled distribution, changed-version
compatibility and production secret storage are still unproved. No P0-4 or M0
completion claim, unrelated/global process/config change or Actions run.

Staged Syncthing follow-up copies the exact native binary plus pinned upstream
LICENSE/AUTHORS to a fresh proof package, resolves only that role and rechecks
membership/digests after the real run. Linked roots, corrupt metadata, tampered
payload/notices and wrong roles fail closed. Windows actual packaged-role run
passes all prior assertions and four clean stops (including 10.107s early stop).
Combined Node **137/137**, managed **14/14**, syntax/diff PASS. Mac plan now expects
**15/15** managed tests including its POSIX mode check. Installed distribution,
publisher authentication and actual version-change compatibility remain open.

P0-2 changed-package follow-up `309bb66`: native runner accepts a separate,
fully inventoried candidate and refuses unchanged/resource-only native proof.
Windows actual baseline Desktop metadata 0.0.0 to trial 0.0.1 (temporary external
config) passes complete pair selection and actual protocol-rejected rollback.
Both Desktop and daemon payload hashes differ in this Windows retained pair.
Five owned sessions clean exit 0 with correct slot/parent paths, protocol-1 health,
released locks and absent IPC. Accepted-codec synthetic sealed packs/manifest
remain unchanged after each scenario. Combined Node **143/143**, storage **5/5**,
release Desktop **17/17**, workspace format/strict Clippy/syntax/diff PASS.
Evidence/task: `windows-changed-native-package-evidence-20261005.md` and
`macos-changed-native-package-test-plan-20261005.md` under `evidence/m0/p0-2/`.
Repo product version remains 0.0.0; no installed updater or semantic daemon
version migration/signature/canonical-production acceptance is claimed.
Mac changed-package parity was reported PASS at
`9a74b40414fad7b34d6e038fbe95eff334820262`: 6/6 new tests, builds/format/strict
Clippy/syntax/diff passed, five clean native sessions, committed candidate and
protocol-rejected rollback with unchanged sealed fixtures. Mac Desktop changed;
daemon bytes stayed identical. Report hash and exact scope are preserved in
`evidence/m0/p0-2/macos-changed-native-package-evidence-20261005.md`.
Native interruption/foundation subsequently reported PASS at `502abf4` as above.
Managed Syncthing subsequently reported scoped PASS at `501d11d` as below.
ADR-0012 PROPOSED and M0 IN PROGRESS. No Actions or startup/install changes.

Managed Syncthing Mac tests at `502abf4` stopped before live launch: **7 PASS,
8 FAIL**, `SYNCTHING_BUNDLE_SOURCE_CHANGED`. Notice pins described Windows CRLF
checkout bytes while Git/Mac files were LF. Fix `f17bcf7` pins the exact Git LF
blobs and scopes `.gitattributes` to LF notice checkouts, without runtime
normalization or alternate accepted hashes. Three regressions cover checkout,
copied bytes and tampering. Windows validation exposed two native observation
races: idle status now waits within the existing 45-second bound and synthetic
fixture ENOENT is pending during replacement; malformed status, pull errors and
other I/O errors still fail. Both failed runs remain recorded.
Corrected Windows live proof passes all managed assertions and four clean stops;
combined Node **150/150**, storage **5/5**, managed **21/21**, syntax/diff PASS.
Evidence: `evidence/m0/p0-4/notice-byte-portability-failure-and-fix-20261005.md`.
Mac rerun at exact `501d11d64cb7d77e8b1c51ad453fdf15f326fc8d` reported
**22/22**, five syntax/diff checks and live isolated runner exit 0. LF notice
checkout and staged binary/notice digests/POSIX modes passed. Incorrect API keys
returned 403/403; pairing, pause/resume, idle, history, restart/new transfer and
both divergent conflict contents passed. Global config observed digest unchanged.
Four monitor PIDs `83988/84004/84014/83984` exited 0 in
`36/10028/791/155` ms and were absent; no forced cleanup/database-stop failure.
Exact report hash and limitations:
`evidence/m0/p0-4/macos-managed-syncthing-evidence-20261005.md`.
Managed Mac parity is closed; no passed Mac plan needs repetition.
No Actions run; installed distribution/version-change/secret acceptance stays open.

Changed-version Syncthing continuation `133428b` now proves Windows **2.1.2 ->
2.1.5** against fresh synthetic profiles. Product pin remains 2.1.5; the old
version is an explicit historical test fixture with a separate ownership scope.
Both CLI and authenticated REST versions are pinned. Folders pause before cold
profile snapshot; live/PID-ambiguous receipts, ownership/corruption, overlap,
links and changed backup refuse copying/restoration. Compatible candidate
transfer/restart/new transfer PASS. Injected version-health rejection stops the
candidate, preserves its profile, restores the complete cold baseline and proves
2.1.2 identity/key/payload plus a new transfer. No newer database is opened by
the old binary. Eight monitor exits 0/PIDs absent; observed global config unchanged.
Combined Node **160/160**, storage **5/5**, four syntax/diff checks PASS; existing
managed 2.1.5 live regression also PASS with four clean stops. Evidence:
`evidence/m0/p0-4/windows-syncthing-upgrade-evidence-20261005.md`.
Mac parity reported PASS at exact `dec5fb19fb37b1199ac0f192c929a7113388cc0b`:
**33/33**, four syntax/diff checks, two live scenarios/runner exit 0. CLI and
authenticated REST confirmed 2.1.2 -> 2.1.5. Compatible upgrade/restart/new transfer
and controlled version-health rejection/cold baseline restore/new transfer passed.
Original identity/key/payload and staged notices/hash/modes retained; global config
observed digest unchanged. Eight monitors exited 0 and PIDs absent:
`85878/85889/85894/85874/85906/85912/85922/85902`, shutdown
`18/17/18/897/19/10020/883/1002` ms. No forced cleanup/database-stop error.
Official historical archive checksum matched; exact input/report hashes and
user-reported artifact provenance:
`evidence/m0/p0-4/macos-syncthing-upgrade-evidence-20261005.md`.
The corresponding Mac plan is complete; no passed plan needs repetition.
Cold maintenance-window fixture rollback is not live canonical rollback,
installed updater, crash atomicity or power-loss proof. ADR-0004 remains PROPOSED.
No Actions, global installation/startup change or unrelated process termination.

Final CI coverage preparation: `.github/workflows/ci.yml` readiness now includes
Desktop Rust tests, explicit debug helper/release Desktop builds and all lifecycle
Node tests plus storage capture/sidecar staging. Both OS jobs retain their
existing selectors/triggers; readiness timeout is bounded at 35 minutes for the
extra build. Local equivalent helper/release builds and Desktop **17/17**,
lifecycle Node **160/160**, storage/sidecar **8/8** PASS. Static workflow review
and diff check pass; a YAML parser is unavailable locally and cloud workflow
execution remains NOT YET VERIFIED. No Actions dispatched; final CI still needs
one stable-source run after outstanding acceptance/security/notice review.
P0-4 Mac upgrade plan is closed; no new Mac request at this continuation.

Security/acceptance review continuation: `82c4d32` fixes a normal-session
renderer executable-selection hole. `p0_2_start_daemon` now validates every
request against the backend-selected ordinary absolute sidecar before runtime
creation/launch; stores/launches the approved backend path. Displayed path is
read-only. Backend-controlled developer override remains available; renderer
cannot replace it. Windows Desktop **20/20**, format/strict Clippy/frontend and
release build, lifecycle Node **160/160**, storage/sidecar **8/8** PASS.
New Mac task: `evidence/m0/p0-2/macos-sidecar-boundary-test-plan-20261005.md`
(Desktop **13/13**, CLI **4/4**, format/strict Clippy/frontend/release build).
Passed Syncthing/login/tray/native recovery plans need no repetition.

Review: `evidence/m0/m0-security-notice-acceptance-review-20261005.md`.
Exact notice digests remain pinned, but delivered source-access notice,
dependency inventory/installer inclusion, secret/permission boundary and proposed
signed/canonical acceptance prerequisites are unresolved. IPC byte framing is
bounded; whole-read deadlines are not established. Checklist now reflects
both-OS Golden/Doctor and scoped Syncthing proofs; security/human/final gates
remain unchecked. No ADR/roadmap prerequisite was waived. No Actions dispatched:
new Mac boundary check and acceptance review precede the final paid CI run.

Read-only Windows signing readiness audit: SDK x64 SignTool is available, but
the current-user My store has **0 currently valid code-signing certificates with
private keys**. No certificate export, purchase, policy change or signing attempt.
This does not inspect external signing services or establish publisher trust;
signed-package acceptance cannot be claimed from the existing unsigned proofs.

## P0-2 evidence status — verified slices; acceptance open

### macOS

The combined Mac release lifecycle slices now PASS, including physical tray
Open/Quit at checkout `dfbaded` using the validated `c77479f` release build.
Desktop `41780` / daemon `41851` survived physical tray left-click restore and
right-click Open with the same daemon PID. Snapshots recorded
`window_restored_via_tray_left_click` and `window_restored_via_tray_menu`.
Physical tray Quit with the daemon active exited 0 and recorded
`clean_quit_from_tray`; both processes and the held lock were gone. Login start
remained disabled. The operator identified the actual black rounded-square
icon left of ChatGPT after its menu-bar position changed; earlier synthetic
input failures are historical, not proof of an application defect.

The initial Desktop-owned lifecycle run at `c77479f` was PARTIAL. Local daemon supervisor
tests (3), sidecar staging tests (3), sidecar resolution tests (2) and release
package-smoke passed. Release Desktop `31422` automatically started daemon
`31562`, rejected a second Desktop launch, and restarted the SIGKILLed daemon
as `32006` within a 500 ms polling observation; renderer IPC answered protocol 1
with the replacement PID. One tray icon and duplicate daemon rejection were
confirmed. Login start was enabled through `Cryptex.plist` for the release
Desktop with `--cryptex-background`. The operator completed real sign-out/sign-in:
Desktop `32654` started with the background argument, no visible window and one
tray icon; daemon `32891` answered renderer IPC with protocol 1 after Dock reopen.
Explicit Stop left no daemon for a measured 10 seconds. Login start was disabled
through the renderer and `Cryptex.plist` removal verified. Final `Quit clean`
removed the Desktop/daemon processes and recorded `state=clean_quit`. Actual
tray Open/Quit could not be exercised; tray exit 0 was not captured. Those tray
checks were NOT YET VERIFIED in that initial run; the test registration was cleaned up and the
runtime evidence is retained.
See `evidence/m0/p0-2/macos-tauri-owned-supervisor-restart-evidence-20261003.md`.
The later physical diagnosis closes those two Mac tray checks. Signed installer
delivery, actual package update/rollback and ADR-0012 acceptance remain open.

The focused tray-only follow-up at checkout `924cc13` reused the validated
release build without repeating login/restart/Stop suites. Desktop `38558`
started daemon `38569`; red close-control activation hid the window and
preserved that PID. Native right/left clicks at the accessible tray item's
observed center did not expose Open/Quit or restore the window. Physical
display captures are retained outside Git in `/tmp/cryptex-tray-only-*.png`.
Tray Open/Quit remained NOT YET VERIFIED in that synthetic follow-up. Fallback single-instance focus plus
renderer `Quit clean` exited 0; no test processes or held lock remain and
login start remained disabled. The UI limitation's root cause is unproven.

Existing evidence proves:
- single-instance rejection
- crash detection / restart under the macOS native supervision candidate
- Mode B DesktopSupervisor process-level lifecycle
- Mode B GUI CloseRequested/hide path evidence
- physical traffic-light close and operator Dock restore; daemon PID unchanged
- synthetic compatible update / failed update / rollback marker-safety evidence

Still not enough to accept ADR-0012.

### Windows Mode B — valid physical foundation evidence

Experiment:
- `windows-mode-b-r2-20261002T050408Z`

Verified:
- daemon ready
- duplicate daemon rejected
- process-level window-close event leaves daemon alive
- forced crash detected
- DesktopSupervisor restart succeeds
- restart latency observed: 32 ms
- stale-lock wait before respawn
- clean stop
- runtime control-file cleanup
- post-restart status observation

Not verified by that run:
- physical Tauri GUI window close
- full Desktop IPC reconnect

### Additional Windows physical-host proofs (2026-10-02)

- Tauri `CloseRequested` hide/restore and daemon-survival proof: VERIFIED for the
  automated control-file-triggered physical Tauri event path. See
  `evidence/m0/p0-2/windows-mode-b-gui-window-close-20261002.md`.
- synthetic compatible update, bad-candidate rejection and rollback marker
  safety: VERIFIED for the Windows comparison harness. See
  `evidence/m0/p0-2/windows-update-rollback-evidence-20261002.md`.
- A direct child-process parent-exit control passed: the daemon retained its lock
  after the launching PowerShell exited. This does not reproduce HKCU Run or
  explain the separate post-login daemon exit; see
  `evidence/m0/p0-2/windows-parent-exit-control-20261002.md`.
- Fixed a reproducible HKCU comparison-harness defect: the on-demand
  `--leave-registered` probe now clears its one-shot stop marker before a later
  login. The physical Windows regression run retained the Run value during
  inspection, then removed it; see
  `evidence/m0/p0-2/windows-hkcu-stop-marker-cleanup-20261002.md`. This does
  not explain the historical reboot event, whose cause remains OPEN.
- Additional Shell-Core review confirmed the 9708 `Explorer_ExecutingFromRunKey`
  event is launch-operation completion, not proof of process exit: another
  startup PID from that same sequence remains alive. The old runtime artifacts
  are insufficient to identify why PID 25856 later disappeared.
- The shared production Desktop Rust IPC client has now physically reconnected
  to real daemons after supervised restart on both Windows and macOS; see the
  platform-specific proofs below. The actual Tauri renderer-to-command
  invocation during external Mode B crash/restart is VERIFIED on both Windows
  and macOS. The Mac release Desktop-owned lifecycle now has partial physical
  evidence (see the current Mac run above); Windows release-mode physical
  verification and the complete cross-platform production gate remain OPEN.
- A separate P0-3 candidate prototype now exists at `tools/m0-ipc-spike/`.
  Its Windows physical run verified role/version/capability handling,
  malformed/oversized rejection and a client reconnecting across an actual
  server-process restart. The DACL is explicitly set to the current token SID,
  and access denial with an authenticated second account subsequently passed
  on a disposable Windows CI VM (`windows-cross-account-acl-20261002.md`). The macOS physical
  runtime proof now verifies socket permissions/path, protocol rejection and
  reconnect after server restart. This is not production-client integration or
  an ADR-0007 decision. Evidence:
  `evidence/m0/p0-3/windows-ipc-spike-20261002.md`.
- A coordinated Windows release-renderer attempt kept the Tauri reconnect
  command active while Mode B force-killed and restarted the daemon. Mode B and
  its shared Rust IPC reconnect passed (`25988 -> 35808`, 34 ms), but the
  renderer's status did not change during the 12-second observation. A separate
  login daemon using the same named-pipe endpoint was active; this may confound
  the observation, but its effect is not proven. Classify the renderer crash
  reconnect as **NOT YET VERIFIED** and preserve the login daemon. The optional
  `--pre-crash-hold-ms` and `--post-restart-hold-ms` harness windows support a
  coordinated repeat that leaves the replacement daemon available for the
  renderer.
  See `evidence/m0/p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md`.
- A controlled Windows repeat used both bounded holds. The release renderer
  displayed `35224 -> 3432`, matching the Mode B measurement; restart latency
  was 32 ms, and both the harness and Desktop exited 0. The Windows external
  Mode B renderer crash/reconnect gate is **PASS / VERIFIED**. The earlier
  uncontrolled-window attempt remains historical NOT YET VERIFIED evidence.
  See `evidence/m0/p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md`.
- The coordinated physical macOS renderer crash/reconnect test is recorded at
  `evidence/m0/p0-2/macos-renderer-crash-reconnect-test-plan-20261003.md`.
  The repeat at `dc0ae10` used both bounded holds (15000 ms before crash,
  10000 ms after restart). The visible renderer displayed
  `IPC reconnected after restart · pid 24966 → 24987`; both PIDs match the
  retained harness measurement (30 ms restart, exit 0). The macOS renderer
  crash/reconnect gate is **PASS / VERIFIED**. Desktop remained open until
  the result appeared, then exited 0 through `Quit clean`. The prior attempt
  without a post-restart hold remains historical NOT YET VERIFIED evidence.
- `cryptexd lifecycle update-proof` now rejects a synthetic mismatched
  Desktop/daemon protocol pair before staging, keeps active daemon `v1`, accepts
  a matching pair, and then runs the existing failed-candidate/rollback proof.
  Windows local tests passed (44 tests); the physical-host measurement is
  retained under `%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\windows-update-compat-20261003`.
  This remains test-only protocol metadata; it does not update a Desktop
  package. The physical Mac repeat at `46169ee` is **PASS / VERIFIED** for
  the synthetic fixture: 33 local tests passed, all ten requested measurement
  fields matched, marker digest remained `d914619dfbb07c95`, and final instance
  count was 0. Tests, build and update-proof exited 0. Evidence is in
  `evidence/m0/p0-2/macos-update-compat-evidence-20261003.md`.
- The new instrumented Windows login at 16:41:48 EDT on 2026-10-02 confirmed a
  regular user-profile logon. After 11.4 minutes, neither the direct daemon Run
  entry nor the PowerShell observer had produced a runtime artifact; the
  observer never attached, so no daemon exit code was captured. Shell-Core had
  no matching Run execution event, and Code Integrity had no matching block.
  Both temporary Run values were then removed and verified absent. Classify this
  as **login start NOT OBSERVED / cause OPEN**, not daemon start-then-exit. See
  `evidence/m0/p0-2/windows-instrumented-login-result-20261002.md`.
- A second physical reboot/login in the `wl-1003h` test produced a fresh
  console daemon PID `26576`, attached observer PID `26356`, and another
  `alive_at_observation_deadline` result after about 601 seconds. The screenshot
  console was the expected output of this test's direct
  `cryptexd.exe lifecycle login-hold` HKCU Run command; it was not the Cryptex Desktop. Both
  test Run values were then removed by exact manifest comparison in the actual
  user's SID-scoped registry view. A cooperative stop was requested; fresh
  status reports `pid=none`, and no matching startup entries or process remain.
  The retained lock file is stale and is not held. Runtime evidence and a
  pre-cleanup registration backup remain under LocalAppData. See
  `evidence/m0/p0-2/windows-reboot-login-survival-repeat-20261003.md`.
- A current-source Windows release executable physically started the
  Desktop-owned daemon, rejected a second Desktop launch (exit 0), replaced an
  exactly identified child after controlled termination, and passed the
  automated Tauri close/show request path with the replacement daemon live.
  This run used the existing executable built from `c77479f`; literal tray
  Open/Quit and login startup were not exercised. A fresh
  `tauri:package-smoke` build is blocked on this host: Code Integrity events
  3033/3077 deny `rustc.exe` loading its generated unsigned
  `serde_repr` proc-macro DLL. No policy bypass was attempted. See
  `evidence/m0/p0-2/windows-release-desktop-owned-supervisor-20261003.md`.
- Desktop close/restore/snapshot proof artifacts now query a fresh daemon
  lifecycle observation for the live PID instead of treating `ready` as live;
  status errors are reported as unavailable. A physical stale-ready Windows
  regression proof is included in the GUI evidence.
- The test-only native login/reboot path is verified on both platforms (Windows
  HKCU Run; macOS LaunchAgent). This does not establish production registration
  or select a supervision candidate.
- The separate historical HKCU Run dispatch/exit findings remain without a
  root cause. Do not relabel the later successful reboot/login proof.

### Windows native Task Scheduler candidate — failed candidate

Experiment:
- `windows-mode-a-r1-20261002T052751Z`

Physical result:
- normal interactive user session
- `schtasks /Create` returned `ERROR: Access is denied.`

Classification:
- valid FAILED CANDIDATE / PERMISSION evidence
- not an invalid harness result
- no UAC/elevation bypass was used
- Task Scheduler via `schtasks` is not accepted as a no-elevation per-user registration path
- this failure does not automatically select Mode B

### Windows HKCU Run candidate

Test-only registration:
- value name: `CryptexDevP02HkcuRun`
- key: `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`
- runtime: `%LOCALAPPDATA%\\Cryptex-Dev\\p0-2\\compare\\mode-a-hkcu`
- no-elevation registration succeeded physically

The candidate is login-start only:
- it does not provide OS crash restart
- crash-restart parity remains OPEN
- ADR-0012 remains unresolved

## Historical physical event — earlier Windows reboot/login

Both Windows and macOS machines were shut down and restarted on 2026-10-02.

The Windows HKCU Run test value had been registered before shutdown.

After the real Windows reboot/login:

```text
registered=true
daemon_lock_held=false
daemon_pid=25856
ready_present=true
```

`Get-Process cryptexd` returned no running process.

The retained `lifecycle.status` contained:

```text
runtime_root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\compare\mode-a-hkcu
pid=25856
instance=held
health=active
supervision=unresolved
registration=not_configured
protocol_version=1
note=holding instance lock for harness
updated_unix=1790946028
```

Artifact inspection showed:
- `ready` exists, contains `25856`, last written 2026-10-02 13:00:28 UTC
- `daemon.lock` exists, contains `25856`, last written 2026-10-02 13:00:28 UTC
- `lifecycle.status` last written 2026-10-02 13:00:28 UTC
- `stop` is absent
- no live `cryptexd` process exists when checked
- the instance lock is no longer held

### Approved interpretation

**Windows HKCU Run login launch is PHYSICALLY VERIFIED.**

The matching timestamps and ready/lock/status artifacts prove the login registration launched `cryptexd` and it acquired the instance lock long enough to publish ready/status.

**Post-login daemon survival is FAILED / ROOT CAUSE OPEN.**

The process later disappeared without a stop file and without writing a clean-shutdown status. Do not claim HKCU Mode A passed.

This also exposes a truthful-health problem:
- stale `daemon_pid` can remain visible after the process has exited
- persisted `lifecycle.status` can still say `instance=held` / `health=active` even though a fresh lock observation says the lock is not held

Persisted status is therefore not sufficient evidence of current runtime health.

### Follow-up after the read-only audit

The Codex implementation pass keeps the supervision comparison open and changes
only lifecycle status reporting:

- `InstanceObservation` separates a PID associated with a currently held lock
  from the last PID recorded in lock/status metadata.
- `lifecycle status` refreshes the timestamped status snapshot from a fresh lock
  observation.
- the short-lived `lifecycle probe` writes a released/stale snapshot after it
  drops its lock, instead of leaving its temporary `active` report behind.
- platform status results expose `daemon_pid` separately from `recorded_pid`.
- tests cover crash, stale PID reporting, status refresh and the persisted
  snapshot.

This does not explain the post-login process exit or add a supervision
mechanism. A persisted snapshot may still age between checks; consumers must
query current status and must not treat the ready marker as liveness evidence.

### Benign Explorer Run dispatch probe — INCONCLUSIVE

The `wd-1003a` test-only registration had reported present before the user's
sign-out/sign-in. The post-login status output reported a registration mismatch
or absence and no marker; its old boolean does not distinguish the two cases.
Read-only inspection later found the exact registered
command present; rerunning Status returned `run_registered=true`. The
marker remains absent. Shell-Core shows that the Run key was enumerated but has
no launch event for the test marker. Its enumeration event does not expose the
value list, and the Security log was unavailable without elevation. Preserve
the session artifacts; use the currently matching registration for another
real sign-out/sign-in observation before cleanup. Do not infer a dispatch skip
or registration-removal cause. Evidence:
`evidence/m0/p0-2/windows-run-dispatch-followup-20261002.md`.

## Syncthing state after reboot

Before reboot, the running Windows Syncthing process was confirmed to be the old P0-1 development instance:

```text
syncthing.exe serve --home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1 --no-browser --no-restart --no-upgrade
```

It was launched from a user PowerShell process and was not registered as a Windows service, scheduled task, HKCU Run value, or Startup-folder item.

The reboot therefore stopped it. P0-1 is already complete. Do not restart it merely to continue P0-2, and do not delete retained P0-1 forensic/corpus data.

## Current continuation

### Windows implementation slice — committed; Mac release lifecycle passed

The project owner approved Mode B with per-user login startup launching
Desktop. The proposal is recorded at
`docs/adr/ADR-0012-background-runtime-implementation-proposal.md`; it is
**PROPOSED, not accepted**. The committed Windows implementation slice adds
unexpected child-exit detection and bounded restart handling to the current Tauri P0-2
supervisor, plus lock-aware restart behavior to the shared process controller.
A target-matched daemon sidecar is now built into the Tauri package-smoke
artifact. Release builds use the product app-data runtime root, start or attach
the daemon, watch adopted daemon processes, enforce one Desktop instance,
expose reversible per-user login-start controls, and create a tray menu for
Open/Quit. Signed installer packaging and complete Windows physical validation
of the startup/tray/supervisor flow remain open; Mac release-mode results are
summarized below.

Verification on Windows:

- `pnpm --dir apps/desktop build`: PASS.
- `cargo check -p cryptex-desktop --locked`: PASS.
- `pnpm --dir apps/desktop test:sidecar-build-script`: PASS (3 target/path
  tests).
- `pnpm --dir apps/desktop tauri:package-smoke`: PASS; Tauri's package-flavored
  release build staged `cryptexd-x86_64-pc-windows-msvc.exe` and produced
  `target/release/cryptex-desktop.exe`. The sidecar is a synthetic development
  build, not yet a signed installer. The current run includes tray and
  autostart integration and completed without compiler warnings.
- `cargo check -p cryptex-desktop --tests --locked`: PASS; all Rust test targets
  compile, including the pinned Tauri single-instance integration.
- `cargo test -p cryptex-desktop --bin cryptex-desktop --locked`: PASS (8 tests)
  on the current implementation, including release runtime paths and sidecar
  resolution.
- `cargo clippy -p cryptex-desktop --all-targets --locked -- -D warnings`:
  blocked because Smart App Control rejected `cargo-clippy` with OS error 4551.
- Rust formatting and `git diff --check`: PASS.
- `cargo test -p cryptexd --test mode_b_supervisor --locked`: test binary built,
  but two process-spawning cases were blocked by Windows Smart App Control
  (OS error 4551). Do not bypass that policy. `cargo check -p cryptexd --tests
  --locked` passed. The Mac run passed all three targeted process tests; Windows
  release-mode physical lifecycle verification remains open.
- `cargo build -p cryptex-desktop --release --locked`: PASS.
- Latest `pnpm --filter @cryptex/desktop tauri:package-smoke`: frontend build
  and sidecar preparation passed, but Tauri Rust compilation failed with
  `E0463` because Windows Code Integrity blocked the generated
  `serde_repr` proc-macro DLL (events 3033/3077). The previously built release
  executable was byte-matched to the latest Desktop source commit and used for
  partial physical lifecycle checks. Do not bypass the host policy. A fresh
  Windows release package must be built on an approved host/configuration.
- No GitHub Actions workflow ran.

A later single-job manual Windows package build was dispatched at `0867b9f`:
`https://github.com/gungortarik/cryptex/actions/runs/37168036553`.
Readiness/Mac and IPC jobs were skipped. It builds an unsigned development
NSIS installer and retains Desktop/daemon hashes; it is not signing or physical
host acceptance. See `evidence/m0/p0-2/windows-ci-package-build-20261003.md`
for execution status before requesting another run. That run completed:
NSIS build, 3 staging tests and 8 release helper tests passed. Artifacts were
downloaded outside Git and their hashes verified. The loose CI Desktop/daemon
ran on the Windows host; installer execution and signing remain unverified.

Windows physical tray Open and active-daemon Quit now PASS in
`evidence/m0/p0-2/windows-physical-release-tray-20261003.md`: Desktop `8140`,
daemon `22828`, native title-bar close hid the window, physical Open preserved
the daemon PID, and physical Quit exited 0 with both processes gone and lock
released. Actual Desktop login startup remains open. A login toggle attempt
from the CI artifact showed a direct-HKCU versus Windows startup-list mismatch;
it was disabled again and the CI processes cleaned up. Launch from the
operator's ordinary session before arming the real login test. Do not dispatch
another workflow for these local checks.

The ordinary-session follow-up is now armed: Explorer launched Desktop `12084`
and daemon `8052`; the renderer enabled startup and Windows' startup list
verified the exact CI-artifact Desktop command with `--cryptex-background`.
Preparation is outside Git at
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\windows-desktop-login-1003`.
The real sign-out/sign-in has now completed. Explorer launched Desktop `29872`
with `--cryptex-background` and daemon `26588` in the new session. The main
window was hidden, physical tray Open restored it, and renderer status/IPC
confirmed the held lock, active daemon and protocol 1. The renderer disabled
the product startup value; Windows startup-list removal was verified. Renderer
Quit clean exited 0 and both processes disappeared. These login slices PASS;
see `evidence/m0/p0-2/windows-desktop-login-result-20261003.md` for exact scope
and the stale agent filesystem records excluded from this proof.

The operator reported login-time consoles. A daemon-owned console host was
observed; all Desktop status/preflight/hold commands now use a platform adapter
with Windows `CREATE_NO_WINDOW`. Local check, all-target Clippy, 11 Desktop
tests and release package-smoke PASS. No host policy was changed and no new
workflow ran. The rebuilt release launched locally, then was cleaned up; its
physical no-terminal/flash observation remains inconclusive, and UI geometry
errors prevented the new-release status/IPC check. Do not count that visual
gate as passed. Product login is now disabled; test processes are gone.
The later observed foreground launch at checkout `73263ca` closes that visual
check: Desktop `14768` / daemon `8968`, renderer status held/active and IPC
protocol 1 passed, and the operator watching the screen confirmed no terminal
window/flash during launch and queries. Renderer Quit clean exited 0; processes
and held lock were gone. No login/reboot, rebuild or workflow was repeated.
Evidence: `evidence/m0/p0-2/windows-console-visual-result-20261003.md`.
Mac compile/lint request:
`evidence/m0/p0-2/macos-background-command-check-plan-20261003.md`.
Executed on macOS 27.0.1 arm64 at `83a770a`, Rust 1.98.1: format PASS
(exit 0), Desktop check PASS (exit 0, dead-code warning), strict all-target
Clippy FAIL (exit 101). `main.rs:64:4` reports unused
`mac_product_runtime_root` in the debug binary; its production caller is
release-only and its other use is test-only. The failure reproduced in an
individual command run. Exact evidence:
`evidence/m0/p0-2/macos-background-command-check-evidence-20261003.md`.
No application code, lint allowance, release package or Actions was changed.
That initial run did not close the strict Mac lint gate.

Source follow-up: `mac_product_runtime_root` is now compiled only for macOS
release or test targets, matching its actual callers and the Windows helper
pattern. No dead-code allowance was added. The Mac runtime tests also cover
missing HOME. Windows format, Desktop strict Clippy and the 11-test Desktop
suite were rerun locally. Mac revalidation has now completed at
`808bb1c039ab3735e3a6a777fafa6eae49790b10` on macOS 27.0.1 arm64,
Rust 1.98.1: format, Desktop check, strict all-target Clippy and targeted
`mac_runtime_path_tests` all exit 0 (2 tests passed, 2 filtered out). Check
and Clippy emit no warnings. The Mac compile/lint gate is now PASS/closed;
the evidence above retains the initial failure and appends exact new results.
No application code, physical lifecycle tests, release package rebuild,
runtime configuration or GitHub Actions was changed/run in this revalidation.

The Mac execution request at
`evidence/m0/p0-2/macos-tauri-owned-supervisor-restart-test-plan-20261003.md`
has been executed. The evidence at
`evidence/m0/p0-2/macos-tauri-owned-supervisor-restart-evidence-20261003.md`
records release Desktop startup and crash/restart, duplicate rejection, real
login startup, Dock reopen plus renderer IPC, Stop without restart, login-start
removal and process cleanup. The subsequent physical-input diagnosis verified
actual tray **Open Cryptex** and **Quit Cryptex**, including active-daemon
shutdown and foreground exit code 0. The test registration and processes are
cleaned up. Do not repeat the passed steps merely to fix the document state.

The focused diagnostic procedure at
`evidence/m0/p0-2/macos-tray-input-diagnostic-plan-20261003.md` is complete;
its physical result closes the Mac tray gate. Windows release login-start and
physical tray/menu slices are now verified, and the earlier release run
verified daemon replacement. The rebuilt Windows release's observed console-free
launch/query behavior and renderer status/IPC are now verified. The old CI
package predates the fix. The Mac debug-target lint failure is resolved and
revalidated at `808bb1c`; do not repeat its passed compile/lint or physical
lifecycle suites for this request. Next, complete the real compatible
Desktop+daemon package update/rollback and canonical-data checks on both
platforms. Keep ADR-0012 unaccepted until every required P0-2 proof passes.

Windows native-package candidate preparation now passed locally: retained
unsigned NSIS `0.0.0` and test-labelled `0.0.1` installers at source `5015ff1`,
with payload hashes outside Git under
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\native-package-proof-1003`.
No version/dependency change was made in the repository and no workflow ran.
Execution plan: `evidence/m0/p0-2/windows-native-package-replacement-plan-20261003.md`.
Baseline installation from the ordinary Windows session now passed its scoped
identity/status/IPC/clean-exit checks: Desktop `26124`, daemon `9412`, protocol 1,
exit 0 and no remaining processes. Evidence:
`evidence/m0/p0-2/windows-native-package-baseline-20261003.md`.
The installed Desktop differs from the retained loose binary only in Tauri's
three-byte `UNK` to `NSS` bundle marker; the exact comparison reproduces its hash.
Candidate `0.0.1` replacement now passed installed version/payload/status/IPC
checks: Desktop `9416`, daemon `28480`, protocol 1, clean exit 0 and no remaining
processes. Evidence: `evidence/m0/p0-2/windows-native-package-update-20261003.md`.
Operator rollback to `0.0.0` also passed: installed Desktop/daemon hashes match
the first baseline, Desktop `25436`, daemon `15340`, renderer protocol 1,
clean exit 0 and no remaining processes. Login startup remains disabled.
Evidence: `evidence/m0/p0-2/windows-native-package-rollback-20261003.md`.
The final installation is stopped at `0.0.0`. The real package sequence passed
its scoped install/replace/operator-rollback checks. A separate serialized
filesystem transaction candidate now passes 28 Windows tests, including five
actual updater-subprocess kill boundaries, plus 8 retained-binary-copy scenarios.
It is not integrated with installed Desktop and its health is injected fixture
behavior. Proposal: `docs/adr/ADR-0012-update-orchestration-candidate.md`.
Windows evidence: `evidence/m0/p0-2/windows-package-transaction-candidate-20261003.md`.
Mac executed `evidence/m0/p0-2/macos-package-transaction-candidate-test-plan-20261003.md`
at `780c0473bdb85b52abc994661f10671cc85e4992`, macOS 27.0.1 arm64,
Node 24.19.0. The default `/var` temporary path failed closed with
`UNSAFE_PATH` (1 passed, 27 failed): `/var` is a symlink and initialization
rejects symlink ancestors. With process-local TMPDIR set to the verified
`/private/var/.../T` real path, all 28 tests passed, none skipped, including
five actual owned-worker SIGKILL boundaries, pre-commit baseline recovery,
post-commit candidate selection, idempotence, corrupt-baseline refusal and
unchanged synthetic fixture inventories. POSIX directory fsync succeeded;
this is not power-loss proof. All four syntax checks and diff checks passed.
Evidence: `evidence/m0/p0-2/macos-package-transaction-candidate-evidence-20261003.md`.
No code repair or security-check weakening was made. Temporary fixtures were
cleaned, including 27 verified leftovers from the failed setup run. Optional
retained-binary-copy proof was not run; no two retained pairs were established.
That initial Mac serialized candidate run was scoped PASS with the explicit
non-symlink temporary-path prerequisite; its unprefixed command failed.
After reviewing `12f76ac`, the test fixture allocator now resolves only the OS
temporary directory before creating owned roots and registers cleanup before
experiment initialization. The package engine's link/junction rejection is
unchanged. A new alias regression passes on Windows: 29/29 tests, no skips.
Evidence: `evidence/m0/p0-2/windows-package-fixture-path-fix-20261003.md`.
Mac follow-up now PASS at `fc31489fc619b9b4b12f93cd350b215efa7f3c7c`:
the ordinary test command with the host's original `/var/.../T/` TMPDIR,
without override, passed all 29 tests (zero failed/skipped, exit 0). The alias
regression and all five actual SIGKILL boundaries passed; POSIX directory
fsync and fixture inventory checks completed. All six syntax checks and diff
check passed. Owned temporary roots were absent before and after execution.
The evidence above appends exact output and both temporary-path forms while
preserving the prior default-path failure and overridden 28-test PASS.
The Mac default-path fixture repair is now verified; no external override is
required. No optional binary staging, source repair or physical test was repeated.
Next: review native helper/startup integration and failure/data-preservation
gates before production architecture selection. Publisher trust, actual
runtime compatibility, concurrency, machine power loss and canonical-data
acceptance remain open; ADR-0012 remains PROPOSED. No repeated physical Mac
lifecycle suite, installed-package change or GitHub workflow ran.

Windows real-health candidate follow-up now PASS: the transaction health callback
executes the copied daemon and the actual shared Rust Desktop IPC client, matches
PID/protocol/held lock, then verifies clean exit 0 and lock release. Compatible
replacement, non-executable daemon rollback and actual protocol mismatch rollback
passed (3 scenarios); restored baselines also answered IPC. Active endpoint
preflight refusal was measured. Rust tool tests 10/10, Node adapter tests 4/4,
format, strict Clippy, local build and syntax checks passed. Evidence:
`evidence/m0/p0-2/windows-package-real-health-20261003.md`.
Mac real-health plan `evidence/m0/p0-2/macos-package-real-health-test-plan-20261003.md`
now PASS at source `1e07ca4` on macOS 27.0.1 arm64 with Node 24.19.0. Format,
Rust tests 10/10, strict Clippy, local helper build, Node adapter tests 4/4,
both syntax checks and diff check passed. The retained arm64 release Desktop /
daemon pair from `c77479f` was supplied as identical baseline and candidate
inputs (Desktop SHA-256 `0bfd52594a4a30bfcd6e37678835bb7d833cfe6be867f44996e45e18de36f1a5`,
daemon SHA-256 `bfbfa26c6df1934312ba0d483f5351ac759c8f3f96458f8753d5b4a70157feb4`).
Three real health scenarios passed: compatible pair, non-executable candidate
rollback and actual protocol 2/2 declaration against protocol 1 daemon rejection
and baseline restoration. Four healthy IPC observations confirmed Desktop role,
matching daemon PID, held lock and protocol 1; candidate/recovery exits were 0
with locks released. The invalid daemon error was `DAEMON_START_FAILED`, child
exit `-13`. Synthetic fixture bytes were unchanged. Full report remains outside
Git at `~/Library/Application Support/Cryptex-Dev/p0-2/mac-package-real-health-20261003-b15a85f/real-health-report.json`.
Detailed evidence: `evidence/m0/p0-2/macos-package-real-health-evidence-20261003.md`.
Desktop bytes were staged only, not run. This is not a version-changing Mac
upgrade or production canonical-data proof; installed updater handover, signing,
concurrency, power loss and canonical-data gates remain open. No GUI/lifecycle
repeat, Desktop rebuild or Actions ran; ADR-0012 remains PROPOSED.
Canonical-data preservation and an operator-context post-exit lock query were
not established by this baseline. The stock installer force-shutdown and sequential
Desktop/daemon copy behavior does not prove safe atomic updates. Automatic
rollback and incompatible-pair preflight remain unimplemented; keep these
distinct from the planned operator-controlled replacement/rollback experiment.

### Prior continuation record

### Latest Windows reboot/login result — VERIFIED

The user prepared `wl-1003h` from ordinary Windows PowerShell at
`2026-10-03T02:02:24Z`, source `0a7eb0e`, clean tree, then physically restarted
Windows. Boot was at 22:06:04 EDT; console session 1 logged in at 22:06.
Daemon PID 31512 started at 22:07:32 EDT and observer PID 31192 attached.
Shell-Core launch records match both PIDs. The complete 600-second observation
ended at 22:17:34 EDT with `alive_at_observation_deadline`, no stop marker and
no observed daemon exit. The direct process query afterward confirmed the same
live daemon and executable. A duplicate probe was rejected with exit 1.
See `evidence/m0/p0-2/windows-reboot-login-survival-20261002.md`.

The legacy comparison registration was safely backed up and replaced in the
user's actual context. Its old stop file remains retained. A second physical
reboot/login repeat and cleanup are recorded at
`evidence/m0/p0-2/windows-reboot-login-survival-repeat-20261003.md`. The
`wl-1003h` test registrations are now absent and no matching daemon process is
running, so registry-mutating Rust lifecycle tests no longer risk disturbing an
armed experiment. No GitHub workflow ran. The historical 13:00 UTC disappearance
still has no established cause. The Windows Mode B physical crash/restart proof
is already retained above. The next Windows lifecycle task is the release-build
Desktop physical startup/close/quit flow, not another Mode A login repeat.

Windows release foundation build is now verified: the full Rust workspace and
the Tauri Desktop with embedded frontend built successfully. The release IPC
process-level check passed. CI now includes the IPC spike on both platforms;
the first dispatched run passed on macOS and exposed a Windows-only redundant
borrow lint. The fix passed full local Windows workspace Clippy; combined-source
CI revalidation passed on Windows and macOS for source commit `a0f1c6f`.
See `evidence/m0/windows-release-build-20261002.md`. The user currently has no
accessible second Windows account. The cross-account IPC ACL proof instead
passed on a disposable GitHub-hosted Windows VM at source `7668d11`, run
`37085465368`. The test authenticates a temporary non-admin account and checks
the effective client-thread SID, exact access denial, owner connections before
and after, identity restoration and account cleanup. It refuses local workstation
execution. See `evidence/m0/p0-3/windows-cross-account-acl-20261002.md`.
The consolidated Windows result index is recorded at
`evidence/m0/windows-test-results-20261003.md`.

Retained diagnostic history and continuation constraints:

1. the second real login changed session 3 to 4, but the user's external
   collector found `wd-1003a` registration absent while Codex-context reads
   found it present. Do not equate these views or infer deletion/recreation.
   The user prepared fresh `wd-1003b` from ordinary Windows PowerShell at
   `2026-10-03T01:38:11Z`; external collection confirmed
   `run_matches_both_views=true`. The next collection at `01:39:33Z` still
   shows session 4 / Explorer created at `01:33:05Z`, before preparation.
   The subsequent real login changed session 4 to 5. Marker PID 15656 wrote
   at `01:42:14.7693778Z`; Shell-Core 9708 recorded that PID at `01:42:15Z`.
   The external-prepared benign HKCU Run command is now physically verified
   to dispatch after login. This does not prove daemon survival.
   Preserve `wd-1003a` and avoid preparing/cleaning startup values from Codex.
   The collector now records package identity and executable ancestry as well.
   `collect-windows-run-dispatch.ps1` retains observations in each session
   root. This command captures registry views, identity, policy, marker integrity
   and relevant Shell-Core events into a unique retained JSON observation.
   Five synthetic controls passed; collection on the physical Windows host
   recorded matching registry views, unchanged script hash, no Run-disabling
   policy values, Run enumeration and no experiment launch/marker.
   Registry visibility depends on observer context; package virtualization is
   a hypothesis, not yet a verified root cause. Preserve session evidence. This
   probe does not establish daemon survival or explain a termination cause.
2. The `wl-1003g` attempt was prepared through `Shell.Application` from Codex.
   Its process chain reaches Explorer, but its status wrapper still inherited
   Codex's registry view; this is not proof of the user's unvirtualized Run
   values. After the latest user sign-out/sign-in, no daemon or observer runtime
   files were present under `wl-1003g`. It is NOT a verified armed login test.
   Prepare a fresh instrumented root in the user's ordinary Windows PowerShell,
   confirm the manifest there, then sign out/in and return. Codex can read the
   retained results. Earlier `wl-1003c/d/e/f` preparations are incomplete.
   `wl-1003f` identified missing `Get-FileHash`; framework SHA-256 fixed that
   failure but not registry-view separation. No CI was dispatched. Preserve
   process-start/shell-launch
   evidence. Do not infer a termination cause if no observer attaches. See
   `evidence/m0/p0-2/windows-instrumented-login-result-20261002.md`.

   Latest external registry output identifies the occupied daemon value as the
   original `target/debug/cryptexd.exe lifecycle hold` command for
   `%LOCALAPPDATA%/Cryptex-Dev/p0-2/compare/mode-a-hkcu`; the external observer
   value is absent. This explains why cleanup against `wl-1003g` refused.
   The legacy root retains a stop file and records a clean shutdown at that
   login. The user ran `replace-legacy-windows-login.ps1 -Apply` from ordinary
   PowerShell and prepared `wl-1003h`; reboot/login and ten-minute survival
   subsequently passed as recorded above. The helper requires the
   exact confirmed old command and an empty observer slot, saves the old value,
   refuses a live legacy ready PID and restores the saved command on preparation
   failure if both slots are vacant. It preserves legacy runtime files.
3. P0-3 candidate comparison is recorded in
   `evidence/m0/p0-3/adr-0007-candidate-comparison-20261003.md`. The user
   selected the evidence-backed `interprocess` 2.4.4 candidate; the scoped
   implementation addendum is accepted at
   `docs/adr/ADR-0007-local-ipc-implementation-addendum.md`. The production
   daemon health endpoint and shared Tauri Rust client are implemented. A
   physical Windows Mode B comparison at source `c5e059d` verified that the
   client connected before forced crash and reconnected to the restarted daemon
   (`2880 -> 24780`, 32 ms restart); see
   `evidence/m0/p0-2/windows-desktop-ipc-reconnect-20261002.md`. Separate
   physical Tauri renderer proofs are now recorded for Windows and macOS in
   `evidence/m0/p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md` and
   `evidence/m0/p0-2/macos-tauri-renderer-ipc-reconnect-20261003.md`. Both drove
   health/reconnect commands through the real renderer and observed a new daemon
   PID after clean stop/replacement. The macOS and Windows repeats also prove
   renderer reconnect while external Mode B force-restarts the daemon. The
   production supervisor/packaging decision remains open; do not accept ADR-0012
   yet.
4. use the existing physically verified macOS LaunchAgent login/reboot proof
   in `evidence/m0/p0-2/macos-native-supervision-evidence.md`. The separate
   release Desktop login-start flow now has partial physical evidence on Mac;
   Windows release-mode proof and signed package integration remain open.
5. use `evidence/m0/p0-2/supervision-candidate-comparison-20261003.md` as the
   current test-evidence comparison; production supervision/update proofs and
   the ADR-0012 decision remain open.

## OPEN

### Next continuation — approved native complete-package handover pilot

Native recovery cleanup and Mac formatting parity are recorded below. The next
concrete plan is `evidence/m0/p0-2/native-package-handover-plan-20261005.md`.
It recommends extending the complete-package slot candidate to actual owned
Desktop/daemon handover in fresh isolated roots, with native resource inventory,
whole-handover exclusion, clean shutdown, real IPC readiness and baseline restart.
The project owner explicitly approved this isolated pilot in the Windows
conversation ("onaylanmistir"). No further method approval is needed within that
scope. No production updater or
ADR acceptance follows from pilot approval. Canonical fixture acceptance also
requires the accepted P0-1 layout reference/integration; do not reconstruct the
missing ADR-0013 decision or touch retained corpora. Mac gets an executable parity
plan after the first implementation slice. Documentation review needs no Actions.

First slice at `e5423f2`: separate native-tree inventory and staging preserve
resources, empty directories and POSIX modes; target/schema/path validation
refuses mismatches, links/junctions, case collisions and overlapping/reused roots.
Copied trees and source bytes are reverified; partial staging retains a typed
diagnostic. Windows Node v24.19.0: **79/79 PASS** (51 transaction + 4 health +
6 native recovery + 18 native tree), no failures/skips, exit 0; three syntax
checks and diff check passed. The retained Windows installed tree (three files)
was copied into a fresh owned proof root and reverified without executing it.
Evidence: `evidence/m0/p0-2/windows-native-package-tree-evidence-20261005.md`.
Mac plan: `evidence/m0/p0-2/macos-native-package-tree-test-plan-20261005.md`.
Mac reported **79/79 PASS**, no failures/skips, exit 0 at `8296e66`, macOS 27.0.1
arm64, Node v24.19.0 and Rust 1.98.1. Locked helper build, three syntax and diff
checks passed, including POSIX modes and symlink refusal. No fixture/child/flush
error was reported; Mac changed no files. Result:
`evidence/m0/p0-2/macos-native-package-tree-evidence-20261005.md`.
Native OS gate primitive at `80b727f` now holds a persistent fs2 lock across
cooperating pilot callbacks; startup/handover contenders refuse it, callback
exceptions release it, and killed helpers invalidate the lease. Existing Desktop
startup/transaction paths are not integrated and arbitrary callbacks are not
automatically canceled. Each later phase must honor abort and check `assertHeld`.
Windows: Rust **14/14**, Node **86/86 PASS** twice on final source, strict Clippy,
locked build, format, two syntax and diff checks PASS. Earlier helper spawn
`UNKNOWN` failure is retained in evidence, with no established cause or bypass.
Evidence: `evidence/m0/p0-2/windows-native-handover-gate-evidence-20261005.md`.
Mac plan: `evidence/m0/p0-2/macos-native-handover-gate-test-plan-20261005.md`.
Mac gate parity: Rust **14/14**, Node **86/86 PASS**, no failures/skips, exit 0 at
`6a7d294`, macOS 27.0.1 arm64, Rust/Cargo 1.98.1, Node v24.19.0. Format, strict
Clippy, locked build, two syntax and diff checks passed; fixture/child/flush
cleanup was error-free. Mac changed no files. Evidence:
`evidence/m0/p0-2/macos-native-handover-gate-evidence-20261005.md`.
Isolated owned session slice at `dd5021b` adds pre-Tauri pilot runtime validation,
separate instance identity, initially hidden windows, bundled sidecar binding,
blocked pilot autostart changes and clean quit control. Windows Node **97/97**,
format, strict Desktop Clippy, frontend/locked release build and five syntax
checks PASS. Local Desktop debug unit execution is BLOCKED by Application Control
4551; it is not counted as a PASS. One targeted Windows package/unit workflow
37308202823 completed SUCCESS at `dd5021b`: release Desktop **17/17**, sidecar
staging **3/3**, unsigned NSIS build and artifact upload PASS. Other workflow jobs
were skipped. Artifact ID 11344842389 has source/file-hash provenance. Independent
CI closes unit verification; the local debug policy block remains historical.
Real local loose release sessions PASS: Desktop/daemon `33552/3364`, then
`460/10924`, matching OS parent/executable identities, protocol-1 IPC, clean exit
0, gone daemon PID, released lock and absent endpoint. Completed launch snapshots
are hidden; this is not physical no-flash confirmation. Early intermediate
snapshot failures were preserved and the readiness event was corrected with a
regression. Same package reopened; no selection activation or rollback yet.
Evidence: `evidence/m0/p0-2/windows-owned-desktop-session-evidence-20261005.md`.
Mac plan: `evidence/m0/p0-2/macos-owned-desktop-session-test-plan-20261005.md`.
Initial Mac owned-session attempt at `8152267` was **NOT YET VERIFIED**: Rust test build
exited 101 before execution because main and the config test both generated a
Tauri context, embedding duplicate `_EMBED_INFO_PLIST` symbols. Format/diff
passed; Clippy, packaging, Node and live sessions were not attempted.
Fix `49e4e5d` tests the config transform directly with two visible windows and
preserves identity/window assertions; main is the sole context generator.
Windows local Desktop **17/17**, format and strict all-target Clippy PASS on this
fix without Actions or policy changes. This does not supersede the historical
4551 result or independently verify Mac. The Mac operator subsequently reported
the full updated plan **PASS** at `6712269b6fffe5d39df81d273bf3e46585a4e0dd`:
macOS 27.0.1 arm64, Node v24.19.0, Rust/Cargo 1.98.1, pnpm 11.28.0;
Desktop **10/10**, helper **14/14**, Node **97/97**, no failures/skips,
format/strict Clippy/helper build/current arm64 .app packaging/five syntax/diff
checks PASS, all exits 0. Owned Desktop/daemon pairs `72303/72313` and
`72322/72330` had correct executable/parent identities (Desktop parent runner
`72300`), protocol-1 health and held locks. Both clean exits were 0 with gone
daemon PIDs, released locks and absent IPC endpoint; native gate cleanup passed.
Snapshots were hidden; physical no-flash/focus observation remains unverified.
Mac changed no files or commits and ran no Actions. This closes only the
same-package owned-session Mac gate, not update/rollback. Report path/hash and
the initial failure are preserved in:
`evidence/m0/p0-2/macos-owned-desktop-session-evidence-20261005.md`.
Native selection slice `13b435d` now stages complete baseline/candidate slots,
writes a separate flushed pending intent and replaces selection under the held
native gate after proven baseline shutdown. Real candidate health failure with
proven cleanup restores selection and restarts baseline before clearing intent.
Uncertain cleanup/failed rollback retains diagnostics and reports failure;
pending intent blocks startup/handover. No interrupted recovery is implemented.
Windows **110/110 Node**, Desktop **17/17** and helper **14/14**, format/strict
Clippy/four syntax/diff checks PASS. Actual compatible candidate commit and
protocol-2-declared vs protocol-1-real rejection/baseline restart PASS, with
clean exits 0, correct OS identities, lock release and endpoint absence.
Both slots use the same prior pilot-enabled release bytes; this is selection
and rollback evidence, not a version upgrade or installed updater.
Evidence: `evidence/m0/p0-2/windows-native-selection-evidence-20261005.md`.
Mac request: `evidence/m0/p0-2/macos-native-selection-test-plan-20261005.md`.
Mac operator reported parity PASS at
`b18c28b39428bd87b4117e4bc1b461dc0798ccee`: macOS 27.0.1 arm64,
Node v24.19.0, Rust/Cargo 1.98.1, pnpm 11.28.0; Desktop **10/10**, helper
**14/14**, Node **110/110**, no failures/skips. Format, strict Clippy, helper
build, current arm64 .app packaging, four syntax and diff checks passed;
all plan/runner exits 0. Compatible baseline/candidate pairs `73913/73921`
and `73937/73947` committed candidate. Protocol-reject pairs `73959/73967`,
`73977/73984`, then `73993/74000` rejected real protocol mismatch (helper exit
2), cleaned candidate and restarted healthy baseline. Parent/executable
identities matched runner 73910 and owning Desktops. All five clean exits 0,
PIDs gone, locks/gates released, IPC absent and both intents removed.
Hidden snapshots passed; physical flash/focus remains unverified. Mac changed
no files or commits. Evidence:
`evidence/m0/p0-2/macos-native-selection-evidence-20261005.md`.
No Actions or installed startup changes on either host.
Next implementation is interrupted native handover recovery with owned-child
absence/ambiguous-state refusal. Signing, canonical acceptance, native
interruption and version-changing update remain open; ADR-0012 stays PROPOSED.

### Latest native interrupted-claim cleanup — Windows and Mac scoped PASS

A test-only Rust command `clear-interrupted-recovery-claim` serializes claim
cleanup with the already pinned fs2 0.4.3 OS lock. It validates an owned,
outside-repository root without links/junctions, requires a valid claim and OS
ESRCH for its owner, rechecks identical claim bytes and removes only that claim.
The OS lock file is persistent and never unlinked. Node adapter
`recoverAfterInterruption` calls the helper then existing guarded recovery;
another caller winning the new claim causes safe refusal. This does not lock
all updater operations or integrate an installed updater.
Windows: Rust **13/13**, Node **51 transaction + 4 health adapter + 6 native
recovery = 61/61 PASS**, no failures/skips. Strict Clippy, locked build, two new
Node syntax checks and diff check passed. `cargo fmt --all` was blocked by
Application Control (os error 4551); the policy was not bypassed.
Mac initially failed format at `486d86c`, applied the authorized formatter-only
repair to `package_recovery_claim.rs` in `7011fc7`, then passed Rust **13/13**
and Node **61/61**, zero failures/skips and test exit codes 0. Format, strict
Clippy, locked build, both syntax checks and diff check passed on macOS 27.0.1
arm64 with Rust/Cargo 1.98.1 and Node v24.19.0. No POSIX flush or child cleanup
error was reported; no fixture remained. Windows reviewed the formatting commit.
No dependency versions changed; Cargo.lock adds only the direct
fs2 edge for the lifecycle tool. No Actions, GUI or installed runtime was used.
Evidence: `evidence/m0/p0-2/windows-package-native-recovery-evidence-20261004.md`.
Mac plan: `evidence/m0/p0-2/macos-package-native-recovery-test-plan-20261004.md`.
Mac result: `evidence/m0/p0-2/macos-package-native-recovery-evidence-20261004.md`.
Native cleaner SIGKILL releases its OS lock; dead recovery claim cleanup and
baseline retry are verified. Broader stress, signing, native Desktop handover,
canonical-data and power-loss durability remain open; ADR-0012 is PROPOSED.

### Latest concurrent recovery slice — Windows and Mac scoped PASS

The test-only candidate exclusively creates `recovery-claim.json` before
inspecting pending intent. A second recovery and updater activation while the
claim exists fail with `RECOVERY_BUSY`. Normal release verifies claim contents.
Three separate-process regressions pass: competing recovery/updater refusal;
a prevalidated updater arriving after old-intent removal cannot activate and
its new journal survives the older recovery; and killed recovery retains claim
and journal, refusing automatic reclamation. Recovery no longer automatically
continues after interruption inside this claim; retained claim needs inspection.
Windows: **51 transaction + 4 health adapter = 55/55 PASS**, no failures/skips.
Four syntax checks and diff check passed. Prior faults, owner checks, process
races and five updater SIGKILL boundaries passed.
Evidence: `evidence/m0/p0-2/windows-package-recovery-claim-evidence-20261004.md`.
Mac plan: `evidence/m0/p0-2/macos-package-recovery-claim-test-plan-20261004.md`.
Mac operator/Codex reported PASS at
`0ce9c93f621dceedd2edb78d736eb526f760a817`, macOS 27.0.1 arm64,
Node v24.19.0: 51 transaction + 4 health adapter = 55/55, no failures/skips,
exit 0. All three claim tests, previous faults/owner/process races and five
updater SIGKILL boundaries passed. Four syntax checks and diff check passed;
no fixture remained and owned-child cleanup was error-free. Mac changed no
files and its branch was clean. Evidence:
`evidence/m0/p0-2/macos-package-recovery-claim-evidence-20261004.md`.
This is fail-closed candidate exclusion, not an OS lock or production selection.
Native automatic recovery after recovery-process crash, broad stress, publisher
trust, handover, canonical-data and durability gates remain open.

### Latest live-updater recovery guard — Windows and Mac scoped PASS

The filesystem candidate now writes schema-2 pending journals with process PID
and transaction UUID. External recovery permits rollback only after the OS
reports ESRCH for the recorded owner. Live/ambiguous owners are refused without
changing selection or intent. Legacy ownerless intents are retained for diagnosis.
Internal automatic rollback requires exact matching of the current invocation's
journal; changed owner content is rejected. An external retry after failed
automatic rollback waits for owner process exit, even if its update call ended.
PID reuse conservatively blocks recovery; no OS process lock is claimed.
Windows: **48 transaction + 4 health adapter = 52/52 PASS**, zero failures/skips.
Seven new cases cover recovery refusal at intent/selected/healthy, successful
baseline restoration after owned updater termination, continued live-owner
commit after refusal, legacy/malformed owner refusal and changed-token refusal.
Four syntax checks and diff check passed.
Evidence: `evidence/m0/p0-2/windows-package-live-owner-evidence-20261004.md`.
Mac plan: `evidence/m0/p0-2/macos-package-live-owner-test-plan-20261004.md`.
Mac operator/Codex reported PASS at
`b05be5e07b8289c6eedfd2fc6303a213511e4839`, macOS 27.0.1 arm64,
Node v24.19.0: 48 transaction + 4 health adapter = 52/52, no failures/skips,
exit 0. All new owner-guard cases, prior races/I/O faults and five SIGKILL
boundaries passed. Four syntax checks and diff check passed; no fixture remained
and owned-child cleanup was error-free. Mac changed no files and its branch
was clean. Evidence:
`evidence/m0/p0-2/macos-package-live-owner-evidence-20261004.md`.
Simultaneous recovery callers, broad stress, native handover/signing, durability
and canonical-data acceptance remain open; ADR-0012 remains PROPOSED.

### Latest separate-process updater slice — Windows and Mac scoped PASS

Four new tests run two actual owned Node updater subprocesses against one fresh
outside-repository package fixture. IPC barriers control staging and selection
validation; both processes exit before cleanup. Competing commits preserve the
winner and reject the stale caller, an existing pending intent refuses a second
updater, and simultaneously released validated contenders produce exactly one
successful commit plus one typed refusal. Post-exit recovery is idempotent and
synthetic fixture inventory assertions pass.
Windows: **41 transaction + 4 health adapter = 45/45 PASS**, no failures/skips.
Syntax checks and diff checks passed. No engine behavior changed for this slice.
Evidence: `evidence/m0/p0-2/windows-package-process-race-evidence-20261004.md`.
Mac plan: `evidence/m0/p0-2/macos-package-process-race-test-plan-20261004.md`.
Mac operator/Codex reported PASS at
`6b8776c479efcb3091985a7c90f7987ba74d2e2e`, macOS 27.0.1 arm64,
Node v24.19.0: 41 transaction + 4 health adapter = 45/45, no failures/skips,
exit 0. All four process races, previous recovery/concurrency tests and five
SIGKILL boundaries passed; three syntax checks and diff check passed. No fixture
remained, owned-child cleanup was error-free, and Mac changed no files.
Evidence: `evidence/m0/p0-2/macos-package-process-race-evidence-20261004.md`.
These are four controlled process races, not broad stress or an OS process lock.
Recovery competing with an active updater remains unverified. Native handover,
signing, power-loss durability, canonical-data acceptance and ADR-0012 remain
open. No GUI, installed runtime or Actions was used.

### Latest recovery-I/O slice — Windows and Mac scoped PASS

The test-only transaction candidate now wraps recovery mutation failures in
`RECOVERY_IO_FAILED`, retaining the underlying cause and structured phase,
OS error code, and observed intent-presence diagnostics. Six new regressions
inject write/flush/rename/unlink failures and automatic rollback failure after
rejected/throwing health. The four direct faults preserve intent bytes and
package slot inventories, then recover the baseline idempotently on retry.
Automatic rollback faults do not report that restoration succeeded.
Windows Node v24.19.0: **37/37 transaction tests + 4/4 health adapter tests PASS**,
zero failures/skips; engine/test syntax and diff checks passed.
Evidence: `evidence/m0/p0-2/windows-package-recovery-io-evidence-20261003.md`.
Mac request: `evidence/m0/p0-2/macos-package-recovery-io-test-plan-20261003.md`.
The Mac operator/Codex reported PASS at source
`9d9159c30af01b45af1c5c9f9994417f3beec611` on macOS 27.0.1 arm64,
Node v24.19.0: 37 transaction + 4 health adapter tests = 41/41, no failures or
skips, exit 0. All six fault cases, controlled concurrency and five SIGKILL
boundaries passed, as did syntax/diff checks. No temporary fixture remained;
Mac changed no files and its branch was clean.
Evidence: `evidence/m0/p0-2/macos-package-recovery-io-evidence-20261004.md`.
These are injected operation errors, not physical disk-full or ACL tests.
Final directory-flush failure after unlink cannot promise journal retention;
power-loss/native durability and cross-process serialization remain open.
No installed app, GUI, startup registration, canonical data, or Actions changed.

### Latest package-candidate concurrency slice — Windows and Mac scoped PASS

Review of the serialized filesystem candidate found a lost-selection race when
one updater paused after staging and another committed. The candidate now
revalidates the selected package before creating its intent, claims the intent
exclusively, then verifies the selection again before activation. It removes
only its content-identical owned intent when it detects that another package won;
unexpected intent content is retained and reported as a typed error.
Windows Node 24.19.0 ran the focused transaction suite: **31/31 passed**, zero
failures/skips, including two deterministic races (commit during staging and
commit between validation and intent creation), all five actual worker-kill
boundaries, and synthetic fixture preservation. `node --check` for engine and
tests plus `git diff --check` passed. Evidence:
`evidence/m0/p0-2/windows-package-concurrent-transition-evidence-20261003.md`.
Exact Mac parity request:
`evidence/m0/p0-2/macos-package-concurrent-transition-test-plan-20261003.md`.
The Mac operator/Codex reported parity PASS at
`ad379892ebd26f74544d37aa56e2552cc71daeee`, macOS 27.0.1 arm64,
Node v24.19.0: 31/31 tests, zero failures/skips, both controlled races and all
five SIGKILL boundaries passed. Syntax/diff checks passed, temporary roots were
cleaned, and the Mac branch was clean/synchronized. Mac changed no files.
Evidence: `evidence/m0/p0-2/macos-package-concurrent-transition-evidence-20261003.md`.

This remains a test-only serialized selection candidate. It has no OS-level
process lock, publisher signature validation, Desktop executable handover, or
installed updater integration. Recovery-I/O failures, stress
concurrency, native executable locks, canonical-data acceptance, and the
explicit ADR-0012 orchestration decision remain open. Do not accept ADR-0012;
do not run Actions for this slice.

- root cause of Windows HKCU Run post-login daemon exit
- Windows crash-restart parity for a no-elevation/native candidate
- Tauri renderer health/reconnect invocation is physically verified on both
  platforms for clean daemon stop/replacement and external Mode B crash/restart.
  Literal macOS traffic-light close and operator Dock restore are VERIFIED at
  evidence commit `52e9dc4` (test build source `aed973d`): Desktop PID `27505`, daemon PID `27600` unchanged after restore,
  `source=tauri_close_requested`, clean exit 0 and released daemon lock.
  See `evidence/m0/p0-2/macos-physical-traffic-light-close-evidence-20261003.md`.
  That historical run did not test the production tray menu or packaged
  supervision. The later Mac release Desktop-owned run is recorded at the top
  of this handoff; its physical tray Open/Quit checks now pass. Signed package
  verification remains open.
- Windows cross-account ACL denial verified on a CI VM; production Desktop/CLI/MCP
  integration remains absent
- real compatible Desktop + daemon package coordination and rollback/canonical-data proof
- production-backed Mode A vs Mode B vs justified hybrid comparison
- ADR-0012 acceptance

## Historical M0 BLOCKED note (superseded)

> **Historical only.** The text below reflected M0 closeout constraints before
> PR #5 merge and M1 authorization. It is retained for audit trail and must not
> be read as current status.
>
> **Current status (see top of this handoff):** M0 Foundation is COMPLETE at
> `8de3c8f`; M1 is active on `m1/brain-device-identity`; ADR-0015 is ACCEPTED;
> ADR-0014 remains PROPOSED.

~~P0-2 cannot be considered complete and ADR-0012 must not be accepted until the
current revised M0 criteria, scoped review, required CI and human approval pass.
Historical production prerequisites in earlier slices remain separate mandatory
product gates under the owner-authorized revision; no production proof is waived.

M1 is not authorized until all M0 P0 gates are complete.~~
