# M1 Kickoff Plan — Brain + Device Identity

**Date:** 2026-10-05

**Revised:** 2026-10-05 (durable-before-return; concurrent allocate atomicity)

**Branch:** `m1/brain-device-identity`

**Base:** `8de3c8f91497cf21d503b9b1bdc5566716d88dc0` (`origin/m0/foundation`, PR #5 merge)

**Implementer:** Cursor (entire M1)

**Phase:** 0 — revised ADR drafts + plan for re-review (no product code, no dependency pins, no GitHub Actions in this phase)

## 1. Authority

- `AGENTS.md`
- `docs/00_MASTER_SPEC.md`
- Accepted M0 ADRs / addenda (0004, 0007, 0012, 0013)
- `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`
- `docs/15_PLATFORM_MACOS_WINDOWS.md`
- `docs/17_TEST_AND_FAILURE_MATRIX.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/milestones/M1_BRAIN_DEVICE_IDENTITY.md`
- Proposed: `docs/adr/ADR-0014-os-secure-store-integration.md`, `docs/adr/ADR-0015-brain-device-identity-foundations.md`

## 2. Fixed requirements (non-negotiable)

- Device private keys in OS secure storage
- `cryptexd` sole canonical writer / privileged secret authority on a device
- Secrets never in repo files, privacy-unsafe logs, or ordinary Markdown
- No custom cryptography
- Hardware-backed / non-exportable keys are **not** required
- No device is canonical master
- Wall-clock is never security authority
- Offline recovery signing secret is not part of normal device runtime
- No real personal / Cortex data in fixtures
- Pairing/revocation **product** behavior → M8
- Recovery Kit **container** / bootstrap product → M11
- Full event-store semantics → M2

## 3. Phase-0 deliverables (revised for re-review)

| Artifact | Purpose |
|---|---|
| Handoff | M0 complete + M1 started; Cursor owns M1 |
| ADR-0014 (PROPOSED) | Secure-store direction; acceptance vs spike vs M1 gate separated; K1/K2/P1 comparison; **no library default/pin** |
| ADR-0015 (PROPOSED) | Identity model; first issue `1`; durable-before-return; **same-`device_id` concurrent atomicity** (incl. first-create race); missing/corrupt fail-closed; pairing records; RK public metadata |
| This plan | Sequence of work after ADR decisions |

**Still not in phase 0:** product Rust/TS changes, dependency pins, Actions runs, or unsolicited secure-store spike code.

## 4. Decision gates (aligned with ADR-0014)

| Gate | Meaning |
|---|---|
| **ADR review** | Owner accepts / revises / rejects ADR-0014 and ADR-0015. Status remains PROPOSED until accepted. |
| **Limited secure-store spike** | Only if owner authorizes while ADR-0014 is still PROPOSED: product-separated evaluation of candidates (at least `keyring-core`+explicit providers vs direct platform APIs). No product backend merge, no workspace production pin. |
| **Product implementation** | Only after the relevant ADR is **ACCEPTED**. |
| **M1 secure-store completion** | Accepted product integration physically verified on **Windows and macOS**. macOS physical verification is **WAITING** until run on a Mac host. Both-OS physical product proof is an **M1 completion** requirement, not conflated with ADR acceptance. |

## 5. Post-acceptance implementation slices (smallest useful)

Ordered after ADR acceptance (and after any authorized spike that informed ADR-0014’s selection addendum):

1. **Pure identity/sequence types + schemas** (ADR-0015) — synthetic fixtures; UTF-8 LF.
2. **Sequence allocator foundation** — explicit `initialize` (`last_issued = 0`); `allocate` requires existing state (missing → fail-closed); persist via crash-safe atomic replace before return; same-`device_id` atomicity incl. first-create race; exact schema_version only.
3. **SecureStore trait + fake store** (ADR-0014) — unit tests without OS prompts.
4. **Windows product secure-store adapter** — physical evidence matrix.
5. **macOS product secure-store adapter** — Mac-host physical evidence (**WAITING** today).
6. **Daemon wiring** — synthetic Brain create; public metadata under app-data; private material only via SecureStore.
7. **Pairing record persistence** — data structures only; no pairing UI (M8).
8. **Recovery Kit public metadata** — offline secrets absent from normal runtime (container = M11).
9. **Security review + DoD** — required CI once at the final appropriate M1 gate.

## 6. Cross-platform / failure / security checks (planned)

| Area | Checks |
|---|---|
| Identity | immutable `brain_id`; install vs device; synthetic-only fixtures |
| Sequence | initialize then first issue `1`; allocate missing fail-closed; durable atomic replace; crash/restart non-reuse; concurrent unique/increasing; unsupported schema rejected; Windows-safe DeviceId filenames |
| Secure-store | accepted adapters on both OS; deny/missing; no secret bytes in logs |
| Pairing records | state validation; Syncthing ID ≠ trust |
| Recovery metadata | public fields only; offline secret absent from runtime |
| Writer boundary | non-daemon paths cannot write canonical identity metadata |


### 6.1 Required sequence crash/restart test (after ADR-0015 acceptance)

Planned verification (local first; not Actions in phase 0):

1. Allocate one or more sequences and record the last returned value `n`.
2. Kill/crash or hard-stop the allocator process **after** a successful return (and separately probe interrupted persist if feasible).
3. Restart and allocate again.
4. Assert the next returned value is **`> n`** (no reuse of any previously returned sequence).
5. Assert allocate never returns a value whose durable `last_issued` update was not committed.

### 6.2 Required concurrent allocate test (after ADR-0015 acceptance)

Planned verification (local first; not Actions in phase 0):

1. Drive many concurrent allocate calls against the **same** `device_id` (including a cold start where the counter does not yet exist).
2. Collect all successfully returned values.
3. Assert the set has **no duplicates**.
4. Assert sorting the returned values yields a contiguous or otherwise strictly increasing sequence consistent with the allocator rules (each value unique and `>` all earlier successful returns for that device).
5. Assert the first-create race does not yield two `1` values or split-brain durable state.
6. Optionally assert distinct `device_id`s may allocate without mutual exclusion across devices.

## 7. Explicit non-goals reminder

- M8 pairing UI / revoke / retire / compromise / cutoff productization
- M11 Recovery Kit container, encrypted backup, first-device bootstrap product
- M2 hashes, signatures, sealed-segment event-store productization
- Broad Desktop product UI (M13)
- Selecting/pinning `keyring` (or any provider) without spike evidence

## 8. Continuation point

**ADR-0015:** **ACCEPTED** (2026-10-05). Identity/sequence implementation slices 1–2 are authorized.

**ADR-0014:** remains **PROPOSED**; secure-store spike and dependent code require separate explicit owner authorization — not started.

**Actions:** deferred until the final appropriate M1 gate.

**Now:** implement identity types + durable sequence allocator under ADR-0015; run local tests only.
