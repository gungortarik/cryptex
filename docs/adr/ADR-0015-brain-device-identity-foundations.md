# ADR-0015 — Brain / Device Identity, Sequence Foundation, and Recovery Public Metadata

**Status:** ACCEPTED  
**Date drafted:** 2026-10-05  
**Date revised:** 2026-10-05 (owner review revision; durable-before-return; concurrent allocate atomicity)
**Date accepted:** 2026-10-05
**Milestone:** M1 — Brain + Device Identity  
**Authority:** `docs/00_MASTER_SPEC.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/07_SYNC_AND_CONFLICT_PROTOCOL.md`, `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`, `docs/10_BACKUP_RESTORE.md`, `docs/milestones/M1_BRAIN_DEVICE_IDENTITY.md`, `docs/milestones/M8_DEVICE_TRUST_REPLICATION.md`, `docs/milestones/M11_BACKUP_DISASTER_RESTORE.md`, `AGENTS.md`

This record proposes the **M1 identity and sequence data model**: immutable Brain identity metadata, install/device identity, sequence-allocation foundation, pairing **data structures only**, and Recovery Kit **public metadata** only.

It deliberately does **not**:

- productize pairing UI, revoke/retire/compromise workflows, or sequence-cutoff enforcement product behavior (**M8**)
- select Recovery Kit serialization/container or first-device bootstrap product flows (**M11**)
- select event hash/signature algorithms or full event-store semantics (**M2**)
- select the OS secure-store library (**ADR-0014**)

**Accepted.** Identity/sequence product slices may proceed under this decision. Secure-store selection remains ADR-0014 (PROPOSED).

## 1. Context

Canonical durable data includes immutable brain identity metadata. Later replication depends on per-device sequences and explicit device trust. Audit corrections require device-sequence cutoff (not wall-clock) for post-revocation acceptance. Recovery must restore content and trust, but the offline recovery signing secret must not live in normal device runtime.

## 2. Decision

### 2.1 Brain identity metadata (canonical, immutable)

Minimal public UTF-8 JSON (LF) record, daemon-written only:

| Field | Role |
|---|---|
| `schema_version` | positive integer for this metadata format |
| `brain_id` | opaque UUID string identifying the Brain |
| `created_at` | RFC 3339 timestamp for **display/timeline only** — never security authority |

No device is master of the Brain. `brain_id` is not silently rewritten in place.

### 2.2 Install identity vs device identity

| ID | Lifetime | Role |
|---|---|---|
| `install_id` | This Cryptex installation / local instance | Local runtime binding; new install ⇒ new `install_id` |
| `device_id` | Trusted-device principal for the Brain | Stable trust/sequence principal once enrolled |

Do **not** treat OS machine GUIDs, Syncthing device IDs, or hardware serials as Cryptex trust proof. Those may be diagnostic annotations only.

Private key material is referenced by handle/id and stored via the approach accepted in ADR-0014; public identity fields may live in ordinary runtime/canonical metadata.

### 2.3 Sequence-allocation foundation

- Each trusted `device_id` owns a durable monotonic `u64` sequence state allocated only by local `cryptexd`.
- Sequence values are the ordering basis for later **device-sequence cutoff** comparisons (M8 productizes cutoff enforcement).
- Wall-clock / `created_at` must never decide revocation acceptance or post-revoke event admission.

#### 2.3.1 New device — initial counter value

When a device’s sequence state is **first created** (no prior durable counter for that `device_id`):

1. Persist initial state `last_issued = 0` (meaning **no sequence numbers have been issued yet**).
2. The first successful allocate advances the counter and returns **`1`**.
3. Subsequent successful allocates return `2`, `3`, … (strictly monotonic).

So the first usable device sequence number is **`1`**, never `0`. The value `0` is reserved for “none issued yet” in durable state and must not be used as an event sequence id.

#### 2.3.2 Existing counter - missing or corrupt

If sequence state for a `device_id` was **previously created / expected to exist**, but the durable counter is **missing** or **corrupt** (unreadable, wrong type, non-integer, fails integrity checks defined at implementation):

- **Fail closed** with a **typed error** (distinct variants such as `SequenceStateMissing` and `SequenceStateCorrupt`).
- **Do not** silently re-initialize to `last_issued = 0` or re-issue `1`.
- **Do not** invent a counter from wall-clock time.
- Recovery / rebuild of a lost counter is a deliberate later procedure (not silent heal); M1 foundation only requires safe refusal.

Additional allocate-time failures also fail closed with typed errors: non-monotonic persisted values, overflow of `u64`, and unauthorized non-daemon callers.

#### 2.3.3 Durable write before return (no reuse after crash)

For every successful allocate:

1. Compute the next sequence value `n = last_issued + 1`.
2. **Persist** the updated durable state (`last_issued = n`) and ensure it is durable for crash recovery (flush/sync as required by the storage primitive chosen after acceptance).
3. **Only then** return `n` to the caller.

Rules:

- A sequence value must **never** be returned to a caller before its durable counter update is committed.
- After process crash or restart, the next successful allocate must return a value **strictly greater** than any value that could have been returned before the crash (i.e. previously issued sequences are **not reused**).
- If durability cannot be confirmed, the allocate fails closed with a typed error and must not hand out `n`.
- In-memory-only counters are not acceptable for production allocation paths.

#### 2.3.4 Concurrent allocate atomicity (same `device_id`)

Allocate for a given `device_id` must be **atomic / serialized** with respect to other allocate calls for that same `device_id` (single mutual-exclusion domain per device counter, whether via lock, single-threaded owner queue, or equivalent durable compare-and-set).

Required properties:

1. Two concurrent successful allocates for the same `device_id` must never return the same `n`.
2. Returned values for that `device_id` must remain strictly increasing under concurrency (each returned `n` is unique and greater than all previously returned values for that device).
3. The durable-before-return rule (§2.3.3) still applies under concurrency: no caller observes `n` before `last_issued = n` is durably committed.
4. **First-counter creation race:** concurrent first-time initializes for the same `device_id` (no prior durable state) must not both create independent counters or both issue `1`. Exactly one initializer wins creation of `last_issued = 0` then issues `1` under the same atomic path; losers either wait and allocate the next values or fail with a typed contention/retry error that does not leave split-brain counter state.
5. Missing/corrupt fail-closed behavior (§2.3.2) must not be bypassed by a racing re-create.

Cross-`device_id` allocates may proceed independently; this section constrains same-`device_id` contention only.

### 2.4 Pairing data structures (not product behavior)

M1 defines durable **records** only, for example:

| Field | Role |
|---|---|
| `peer_device_id` | remote device principal |
| `state` | `pending` \| `trusted` \| `retired` \| `compromised` (data enum) |
| `local_brain_id` | must match local Brain |
| `updated_at` | display only |

**Out of scope (M8):** pairing UI, invite flows, Syncthing automation as trust, revoke/retire/compromise workflows, incoming validation pipelines, cutoff enforcement productization.

Syncthing device ID alone is never sufficient Cryptex trust (`docs/07`, `docs/08`).

### 2.5 Recovery Kit public metadata (not container)

Public, non-secret metadata only, for example:

| Field | Role |
|---|---|
| `schema_version` | metadata format version |
| `kit_id` | opaque id for the kit instance |
| `brain_id` | Brain this kit bootstraps |
| `purposes` | declared purposes (backup-decrypt / trust-bootstrap) — **not** embedded secrets |

**Hard rule:** offline recovery signing secret material is **not** stored in normal device runtime state and must not be written into these public metadata files, logs, or the Git repository.

**Deferred to M11 / backup crypto spike:** exact Recovery Kit encoding, container, encryption format, and first-device bootstrap UX.

## 3. Alternatives considered

| Alternative | Verdict | Reason |
|---|---|---|
| Collapse install_id and device_id | Rejected | Reinstall and trust-principal lifetimes differ. |
| Use wall-clock for revoke ordering | Rejected | Forbidden by audit / `docs/07` / `docs/08`. |
| First issued sequence = 0 | Rejected | Reserves no clean “none issued” state; `1` first-issue is clearer for cutoff math. |
| Silently reset missing/corrupt counters | Rejected | Could allow sequence reuse / weaken later cutoff; must typed-fail. |
| Return sequence before durable persist | Rejected | Crash/restart could re-issue the same `n`; violates monotonic unique issuance. |
| Unsynchronized concurrent allocate / racing first create | Rejected | Duplicate `n` or split-brain counters; weakens later cutoff and attribution. |
| Use Syncthing device ID as Cryptex device_id | Rejected | Transport ≠ application trust. |
| Put full Recovery Kit secrets on every device | Rejected | Violates offline-recovery secret boundary. |
| Specify Recovery Kit container in M1 | Rejected | Deferred to M11 / spike. |
| Separate ADRs per subtopic | Rejected for now | Avoids ADR sprawl; secure-store remains ADR-0014. |

## 4. Writer and storage boundaries

- All canonical identity metadata writes go through `cryptexd`.
- Desktop/CLI/MCP/Obsidian/AI must not mutate these files directly.
- Runtime paths remain under platform app-data roots (`Cryptex` / `Cryptex-Dev`), never inside the Git repo, home Desktop, or Documents by default.
- Exact subdirectory layout under the app-data root may be chosen during implementation after acceptance.

## 5. Evidence status

| Item | Status |
|---|---|
| Spec alignment (M1 / 06 / 07 / 08 / 10) | Documented in this draft |
| Sequence initial/missing/corrupt + durable-before-return + concurrent atomicity | Specified in §2.3.1–2.3.4 — crash/restart non-reuse and concurrent unique/increasing tests planned after acceptance |
| Cross-platform schema/unit tests | Planned — not started |
| Secure-store binding of private keys | Depends on **ADR-0014** |
| M8 cutoff product behavior | Explicitly deferred |
| M11 kit container / bootstrap | Explicitly deferred |

## 6. Recovery / security implications

- Public metadata is not a substitute for OS secure storage of private keys.
- Defined first-issue (`1`), fail-closed missing/corrupt counters, durable-before-return allocation, and same-`device_id` concurrent atomicity preserve later cutoff safety and prevent duplicate or post-crash sequence reuse.
- Pairing records must not become trust merely because Syncthing paired.
- Fixtures use synthetic IDs only — no real personal / Cortex data.

## 7. Acceptance criteria (satisfied for acceptance)

1. Owner review completed; proposal accepted 2026-10-05 (including §2.3.1–2.3.4).
2. Agreement that M8/M11 deferrals above are correct.
3. After acceptance: implementation + tests for schemas, first-issue `1`, missing/corrupt typed refusal, durable-before-return allocation, crash/restart non-reuse, concurrent same-`device_id` unique/increasing allocates (including first-create race), monotonicity, absence of offline recovery secrets in normal runtime, and redacted diagnostics — still without M8 UI or M11 containers.

## 8. Acceptance status

| Field | Value |
|---|---|
| ADR-0015 | **ACCEPTED** |
| Product implementation | **Allowed** for identity/sequence/pairing-records/RK-public-metadata under this ADR |
| Pairing/revocation product | **M8** |
| Recovery Kit container | **M11** |
| First issued sequence | **`1`** (`last_issued` starts at `0`) |
| Missing/corrupt counter | **Typed fail-closed** (no silent reset) |
| Durable before return | **Required** (no return of `n` before persist) |
| Crash/restart reuse | **Forbidden** (next allocate > any previously returned `n`) |
| Concurrent same-`device_id` allocate | **Atomic/serialized** (unique strictly increasing `n`; first-create race covered) |

## 9. References

- `docs/milestones/M1_BRAIN_DEVICE_IDENTITY.md`
- `docs/milestones/M8_DEVICE_TRUST_REPLICATION.md`
- `docs/milestones/M11_BACKUP_DISASTER_RESTORE.md`
- `docs/adr/ADR-0014-os-secure-store-integration.md`
- `evidence/m1/m1-kickoff-plan-20261005.md`
