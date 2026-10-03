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
| ADR-0004 | Managed Syncthing integration | SOURCE RECORD MISSING / ADDENDUM REQUIRED | M0 P0-4 must prove managed Syncthing lifecycle/configuration. Record chosen packaging/supervision in the ADR addendum before the replication milestone. |
| ADR-0007 | Local IPC transport | ORIGINAL SOURCE RECORD MISSING / IMPLEMENTATION ADDENDUM ACCEPTED | `docs/adr/ADR-0007-local-ipc-implementation-addendum.md` selects pinned `interprocess` 2.4.4 from M0 evidence. Production Desktop reconnect on both OSes remains an integration proof; do not infer or reconstruct the missing original ADR. |
| ADR-0012 | Background runtime / atomic update lifecycle | SOURCE RECORD MISSING / PENDING M0 | M0 P0-2 must resolve supervision, startup, single-instance, reconnect, update and rollback behavior on macOS and Windows. Accept ADR-0012 before M1/M2. |
| ADR-0013 | Physical immutable event storage layout | SOURCE RECORD MISSING / PENDING M0 | M0 P0-1 must benchmark event packaging at realistic scale on both platforms. Accept ADR-0013 before M2. |

## 4. M0 gate

M0 must complete all four P0 technical spikes before the project proceeds to M1.

The M0 gate requires:

- ADR-0012 accepted
- ADR-0013 accepted
- ADR-0007 implementation addendum completed
- ADR-0004 implementation addendum completed
- any architecture changes caused by spike evidence reflected in the Master Spec before implementation continues

## 5. ADR source records

The original canonical ADR source files are not currently present in the repository. An implementation addendum may be added when M0 evidence resolves an exact implementation detail; it does not reconstruct the missing original record.

Do not infer or recreate their prior contents from ADR numbers alone.

If an original ADR source is recovered, add it to the repository and reconcile this index against it before changing status.

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
