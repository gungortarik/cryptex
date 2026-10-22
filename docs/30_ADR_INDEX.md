# 30 — Cryptex ADR Index

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## 1. Purpose

This file is the authoritative index of Cryptex Architecture Decision Records referenced by the current specification set.

It does not create architectural decisions by itself.

If an ADR record is referenced by authoritative documents but its source file is not yet present in the repository, that absence is recorded explicitly rather than silently reconstructed.

## 2. Status vocabulary

- `ACCEPTED` — decision is accepted and authoritative for its scope
- `PROPOSED` — decision record exists but is not yet accepted
- `PENDING M0` — decision must be resolved by an M0 technical spike
- `ADDENDUM REQUIRED` — an existing decision requires implementation-specific detail before the relevant production milestone
- `SOURCE RECORD MISSING` — authoritative documents reference the ADR, but the ADR source file is not currently present in the repository

## 3. Referenced ADRs

| ADR | Subject | Current status | Required action |
|---|---|---|---|
| ADR-0004 | Managed Syncthing integration | ORIGINAL SOURCE RECORD MISSING / M0 IMPLEMENTATION ADDENDUM ACCEPTED | `docs/adr/ADR-0004-managed-syncthing-implementation-addendum.md` selects pinned native 2.1.5 file transport, isolated managed profile and backend-owned authenticated loopback REST for M0 P0-4. Both-OS staged lifecycle and changed-version/cold-restore evidence retained. Production secrets, bounded REST, installed distribution and delivered notices remain mandatory separate gates. |
| ADR-0007 | Local IPC transport | ORIGINAL SOURCE RECORD MISSING / IMPLEMENTATION ADDENDUM ACCEPTED | `docs/adr/ADR-0007-local-ipc-implementation-addendum.md` selects pinned `interprocess` 2.4.4 from M0 evidence. Production Desktop health/reconnect is physically verified on both OSes within recorded scope; broader Desktop/CLI/MCP APIs remain incomplete. Do not infer or reconstruct the missing original ADR. |
| ADR-0012 | Background runtime / atomic update lifecycle | ORIGINAL SOURCE RECORD MISSING / M0 IMPLEMENTATION PROPOSAL ACCEPTED | `docs/adr/ADR-0012-background-runtime-implementation-proposal.md` records Mode B + per-user Desktop startup; closing the window hides, explicit Quit cleanly stops the daemon and exits Desktop. Owner accepted this scoped implementation direction on 2026-10-05. Developer file-control gating, signed installed updater and production canonical safety remain mandatory product gates. |
| ADR-0013 | Physical immutable event storage layout | ACCEPTED | Source: `docs/adr/ADR-0013-physical-event-storage-layout.md`. Canonical packaging is immutable sealed per-device segment/pack files. One-file-per-event is rejected. Hybrid is not selected. `segment_events=1000` remains a P0-1 benchmark profile, not a frozen production constant. |
| ADR-0014 | OS secure-store integration | ACCEPTED | Source: `docs/adr/ADR-0014-os-secure-store-integration.md`. Decision addendum: `docs/adr/ADR-0014-os-secure-store-decision-addendum.md`. Accepted 2026-10-06: **P1** direct platform APIs under `cryptexd`. Product pins: Windows `windows-sys = 0.61.2` + `widestring = 1.2.1`; macOS `security-framework = 3.7.0`. K2 and high-level `keyring` unselected. Gate A complete; Gate C Windows and macOS product verification both VERIFIED 2026-10-06 (evidence/m1/gate-c-product-secure-store-windows-20261006.md, evidence/m1/gate-c-product-secure-store-macos-20261006.md). |
| ADR-0015 | Brain/device identity foundations | ACCEPTED | Source: `docs/adr/ADR-0015-brain-device-identity-foundations.md`. Accepted 2026-10-05. Brain/install/device identity; sequence first-issue `1`; durable write before return; same-`device_id` concurrent allocate atomicity including first-create race; missing/corrupt typed fail-closed; pairing records only; Recovery Kit public metadata. Pairing/revocation product remains M8; kit container remains M11. |
| ADR-0016 | Immutable event store productization | ACCEPTED | Source: `docs/adr/ADR-0016-immutable-event-store-productization.md`. Accepted 2026-10-07 at `5cc89b709dcf236698c3253b8582c873de7c7f0f`. RFC 8785 JCS; domain-separated BLAKE3+Ed25519; full-`u64` `device_seq` decimal string; crash-safe signing-key init; ordered `.open` quarantine-before-truncate recovery; safe quarantine; single Rust→schemars JSON Schema→TS path; per-device `.open`/`.seg` with configurable seal default (not a frozen protocol constant). Windows quarantine extended-path addendum ACCEPTED 2026-10-07: `docs/adr/ADR-0016-windows-quarantine-extended-path-addendum.md` (`\\?\` I/O in quarantine adapter only; §8.1 names and DeviceId policy unchanged).
| ADR-0017 | Structured event evolution and knowledge materialization | ACCEPTED | Source: `docs/adr/ADR-0017-structured-event-evolution-and-knowledge-materialization.md`. Accepted 2026-10-08. Defines strict v2 structured assertions, versioned cardinality, deterministic M4 reduction and additive rebuildable tables in M3 `projection.sqlite`. It is a narrow dependent refinement of ADR-0016: v1 bytes, validator, hashes/signatures, framing and quarantine behavior remain unchanged; verified-unsupported payload and unverifiable-envelope handling are explicit. Production v2 writing stays disabled until the separate activation gate passes.
| ADR-0018 | Local device identity for M4 retrieval | ACCEPTED | Source: `docs/adr/ADR-0018-local-device-retrieval-identity.md`. Owner approved 2026-10-08. Defines explicit local-only DeviceId enrollment bound to the ADR-0016 daemon signing identity and a bounded, read-only guarded Desktop search IPC. Unknown peer streams fail closed; this does not implement M8 pairing/retirement, cross-device trust, or v2 production writing.
| ADR-0019 | M8 peer admission and security-event evolution | ACCEPTED product direction only / PROTOCOL ACTIVATION GATED | `docs/adr/ADR-0019-m8-peer-trust-design-proposal.md`: owner authorized the bilateral approval/no-transitive-grant direction subject to repository rules on 2026-10-20. In-memory policy helpers and read-only transport work may proceed. A complete reviewed bootstrap/security-event/issuer-authority addendum is still required before trust-changing implementation. |

## 4. M0 gate

M0 completed the four P0 technical spikes and integrated at
`8de3c8f91497cf21d503b9b1bdc5566716d88dc0` (`origin/m0/foundation`, PR #5).

The M0 gate required:

- ADR-0012 accepted (scoped implementation proposal)
- ADR-0013 accepted
- ADR-0007 implementation addendum completed
- ADR-0004 implementation addendum completed
- any architecture changes caused by spike evidence reflected in the Master Spec before implementation continues

## 4.1 M1 gate

M1 was completed and integrated into `m0/foundation` by merge commit
`c2168f3edbd4deb00d5e16f5f35643948028b0ca` on 2026-10-07 (PR #6).

- ADR-0015 is **ACCEPTED** (2026-10-05); identity/sequence/pairing-record/RK-public-metadata implementation may proceed under that decision
- ADR-0014 is **ACCEPTED** (2026-10-06) with **P1** and the recorded product pins; SecureStore implementation may proceed under `cryptexd`; K2/`keyring` unselected
- Windows and macOS physical **product** secure-store Gate C both VERIFIED 2026-10-06 (evidence/m1/gate-c-product-secure-store-windows-20261006.md, evidence/m1/gate-c-product-secure-store-macos-20261006.md); Gate A spike is not product verification
- pairing/revocation product behavior remains assigned to M8
- Recovery Kit container/serialization remains assigned to M11
- required GitHub Actions PASS on implementation source `a399205`: run [37569756071](https://github.com/gungortarik/cryptex/actions/runs/37569756071) (Windows/macOS readiness + Windows IPC ACL; unsigned package skipped by design); owner accepted the delegated Codex technical review recorded at `evidence/m1/m1-final-review-20261007.md`; M1 gates are satisfied and PR #6 is merged. M2 started 2026-10-07 on `m2/immutable-event-store`; ADR-0016 accepted 2026-10-07; product slices may proceed under that decision.

This index accepts ADR-0015 and ADR-0014 by the statuses above.

## 4.2 M2 gate

M2 — Immutable Event Store is **COMPLETE within its accepted scope** and integrated into `m0/foundation` by PR #8 at merge commit `817b6a126cf6e9cd3e8a27b1348ec977bdc9dcdd` (2026-10-08).

- ADR-0013 remains **ACCEPTED** (sealed per-device segments; `segment_events=1000` is not a frozen production constant)
- ADR-0016 is **ACCEPTED** (2026-10-07) at `5cc89b709dcf236698c3253b8582c873de7c7f0f`
- Final macOS verification and dual-OS CI PASS are recorded in `evidence/m2/m2-final-ci-gate-20261008.md`; owner-delegated closeout review and residual dispositions are in `evidence/m2/m2-owner-delegated-review-20261008.md`
- `LongPathsEnabled=0` is not claimed; this remains a documented verification limitation, not an M2 gate failure under the accepted addendum

## 4.3 M3 gate

M3 — Local Projection + Search is **COMPLETE within scope** and integrated into `m0/foundation` by PR #9 at `35538c144434e1dbfb97b47f89d2536b7646046a`. Final review and dual-OS verification are recorded in `evidence/m3/m3-final-review-20261008.md` and `evidence/m3/m3-final-ci-gate-20261008.md`. SQLite/FTS remains derived and disposable; M4 additions are governed by ADR-0017 and do not change M3 schema v1.

## 4.4 M4 gate

M4 — Knowledge Engine is **COMPLETE within accepted implementation scope** and
integrated into `m0/foundation` by normal PR #10 merge
`a6c32bd30f883e546e2eb976787a59057fae211b` after owner-delegated review. Final review: `evidence/m4/m4-final-review-20261010.md`; corrected
Windows/macOS CI PASS: run `38033047073` at `f9bd894`. ADR-0017 is
**ACCEPTED** and governs strict v2 structured-event evolution, deterministic
knowledge materialization, retrieval safety, and additive derived state in the
existing M3 SQLite database. ADR-0018 is also **ACCEPTED** and authorizes only
the local DeviceId/key binding plus bounded guarded Desktop search IPC required
by the M4 retrieval gate. Unknown peer streams remain fail-closed; no M8 device
management is added. The exact M4 implementation sequence and tests are in
`docs/milestones/M4_KNOWLEDGE_ENGINE.md`. Production v2 writing remains
disabled until ADR-0017 §8.2 separately passes; M8/M10/M14 work is not
authorized by M4 acceptance.

## 4.5 M5 implementation gate

M5 — Capture + Timeline is **IN PROGRESS** on
`codex/m5-capture-timeline`, based on the verified M4 merge above. Slice 1 daemon
capture/reconciliation was owner-authorized and implemented. Its focused Windows
and macOS local verification is complete at product fix `d206ea4`; exact Mac
tested HEAD and host are recorded in
`evidence/m5/m5-slice1-macos-verification-20261010.md`. Slice 2 retry/restart
safety also has focused Windows/macOS local PASS at product fix `07005d9`;
Mac tested HEAD `44e686c` and scenario counts are recorded in
`evidence/m5/m5-slice2-macos-verification-20261010.md`. This is not power-loss
verification. Follow
`docs/milestones/M5_CAPTURE_TIMELINE.md` and
`evidence/m5/m5-kickoff-plan-20261010.md`. No new ADR is accepted by this status
update. Later slices require separate authorization; final M5 CI, Definition of
Done and owner acceptance remain open. No Actions run or merge was performed for
the Slice 1/2 documentation closeouts.
Production v2 activation remains CLOSED.

## 5. ADR source records

Some original ADR source records remain absent. An implementation addendum may
record evidence for an exact implementation detail without reconstructing a
missing original record. ADR-0007 has an accepted implementation addendum.
ADR-0012 and ADR-0004 have accepted, evidence-scoped M0 implementation records;
neither reconstructs its missing original source record.

ADR-0013 is **ACCEPTED** at `docs/adr/ADR-0013-physical-event-storage-layout.md`.
ADR-0012's scoped implementation proposal and ADR-0004's scoped M0 P0-4
implementation addendum are accepted; their original source records remain
missing. Other referenced ADR source files remain missing until recovered or
written through their spikes.

The scoped ADR-0012 proposal and ADR-0004 addendum are not reconstructions of
missing original source records. They record new, evidence-scoped decisions;
their mandatory production gates remain open.

If an original ADR source is recovered for other ADRs, add it to the repository and reconcile this index against it before changing status.

If M0 evidence requires a new or revised decision, create an explicitly scoped addendum with:

- context
- decision
- alternatives considered
- evidence
- tradeoffs
- platform implications
- recovery/security implications where relevant
- acceptance status
- date

## 6. Source basis

This index consolidates explicit ADR references from:

- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/33_CURSOR_HANDOFF.md`

It does not independently introduce or accept any architecture decision.
