# ADR-0016 — Immutable Event Store Productization

**Status:** PROPOSED

**Date drafted:** 2026-10-07

**Milestone:** M2 — Immutable Event Store

**Authority:** `docs/00_MASTER_SPEC.md`, `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md`, `docs/26_VERSIONING_MIGRATIONS_AND_UPDATES.md`, `docs/milestones/M2_IMMUTABLE_EVENT_STORE.md`, ADR-0013 (ACCEPTED), ADR-0014 (ACCEPTED), ADR-0015 (ACCEPTED), `AGENTS.md`, `evidence/m0/p0-1/`, `evidence/m2/m2-kickoff-plan-20261007.md`

This record proposes the **M2 productization choices** left explicitly unresolved after ADR-0013 and the event/storage/security specs: logical event envelope, on-disk record serialization, integrity hash and device signature algorithms/libraries, schema-version encoding, quarantine layout, production segment naming, and seal-roll defaults.

It deliberately does **not**:

- reopen ADR-0013’s packaging architecture decision (sealed per-device segments remain accepted; one-file-per-event remains rejected; hybrid remains unselected)
- freeze P0-1 benchmark profile `segment_events=1000` as a protocol constant
- implement M3 SQLite/search, M8 replication product behavior, or M10 purge product workflows
- invent custom cryptography

**Not accepted.** Dependent product implementation must not proceed until this ADR (or an accepted narrower successor covering the blocking choices) is ACCEPTED.

## 1. Context

M2 must productize:

- finalized sealed per-device segment packaging
- Rust-sourced JSON Schema + TypeScript bindings
- atomic durable canonical writes
- hashes / signatures / integrity-chain behavior
- quarantine for invalid / corrupt / incompatible / unverifiable input
- schema-version behavior

Authoritative docs require those behaviors but explicitly leave field names, serialization, crypto libraries, quarantine layout, segment filenames, and seal-roll policy unsettled (`docs/04` §22, `docs/06` §16, `docs/08` §21).

Reusable foundations already exist:

- M1 identity/sequence (`brain_id`, `device_id`, first usable sequence `1`, fail-closed corrupt/missing counters)
- ADR-0014 SecureStore for opaque device private material (algorithm not selected)
- P0-1 bench segment mechanics (`.open` / `.seg`, length-prefixed records, truncated-tail recovery) as **evidence**, not accepted wire format
- cryptexd atomic metadata replacement patterns

## 2. Decision summary (proposed)

| Topic | Proposed selection |
|---|---|
| Logical event envelope | Minimal attributed envelope in §3 |
| Event body serialization | Canonical UTF-8 JSON (LF) object bytes |
| Segment record framing | Versioned length-prefixed binary frame around JSON body (§4) |
| Content hash | BLAKE3-256 over canonical signed bytes (§5) |
| Device signature | Ed25519 over the content hash (§5) |
| Libraries | `blake3`, `ed25519-dalek` (exact pins at implementation; no custom crypto) |
| Integrity chain | Per-device `prev_content_hash` linking (§5) |
| Schema version | Positive `u32` `schema_version`; M2 ships `1`; unknown exact-match policy in §6 |
| Quarantine layout | Under app-data, outside replication trust (§7) |
| Segment naming / layout | Per-device sealed packs under canonical events root (§8) |
| Seal-roll default | Configurable; suggested default 1000 events **as tunable config only** (§8) |
| Schema/bindings generation | Rust types → JSON Schema + TypeScript via pinned generators (§9) |
| Writer boundary | `cryptexd` only (§10) |

## 3. Logical event envelope (proposed)

M2 canonical events are immutable once accepted. Corrections are additional events, never in-place edits.

### 3.1 Required envelope fields

| Field | Type / encoding | Role |
|---|---|---|
| `schema_version` | `u32` | Event schema version; M2 initial value `1` |
| `event_id` | UUID string | Stable event identity |
| `brain_id` | UUID string | Must match local Brain identity (ADR-0015) |
| `device_id` | path-safe string | Trusted-device principal (ADR-0015) |
| `device_seq` | `u64` | Per-device sequence from cryptexd allocator; first usable `1`; never `0` |
| `created_at` | RFC 3339 string | Display/timeline only — never security authority |
| `event_type` | string enum | Discriminator for typed payload |
| `classification` | `public` \| `internal` \| `private` \| `restricted` | Privacy boundary |
| `payload` | JSON object | Type-specific body; untrusted subset rules still apply |
| `prev_content_hash` | 64-char lowercase hex BLAKE3 | Prior accepted event on same `device_id`, or 64 zero hex for the device’s first event |
| `content_hash` | 64-char lowercase hex BLAKE3 | Hash of canonical signed bytes (§5) |
| `signature` | lowercase hex Ed25519 signature | Device signature over `content_hash` bytes (§5) |

### 3.2 Initial `event_type` values (M2-minimal)

M2 needs a closed initial enum so schema generation and validation are testable without pulling later milestones:

| `event_type` | Purpose in M2 |
|---|---|
| `memory.note` | Minimal durable memory note body (`text` string) for store/round-trip tests |
| `system.integrity_probe` | Synthetic fixture-only type for integrity/quarantine tests (never personal/Cortex data) |

Later milestones may extend the enum via schema_version / compatibility rules. M2 must not implement knowledge consolidation, attachment product storage (M9), or purge product events (M10).

### 3.3 Alternatives considered

| Alternative | Decision | Reason |
|---|---|---|
| Defer all field names to “discover in code” | Rejected | Violates “do not invent without ADR”; blocks reviewable schema generation |
| Full V1 memory/knowledge event taxonomy in M2 | Rejected | Scope creep into M4/M5/M9/M10 |
| Binary-only logical model with no JSON envelope | Not selected | Harder TS/JSON Schema generation; conflicts with existing UTF-8 JSON metadata practice |

## 4. Serialization and segment framing (proposed)

### 4.1 Event body

- Serialize the logical envelope (excluding `content_hash` and `signature`) to **canonical UTF-8 JSON with LF** and a fixed key order defined by the Rust serializer contract in implementation tests.
- `content_hash` / `signature` are computed from that canonical body and then stored with the accepted record.

Canonicalization rule (proposed, must be locked by tests):

1. UTF-8, LF only (no CR)
2. Object keys in the explicit field order listed in §3.1 (payload keys sorted lexicographically)
3. No insignificant whitespace outside JSON string values
4. Numbers as minimal JSON integers/floats without leading zeros

### 4.2 Segment record framing

Evolve P0-1’s length-prefixed idea into a **versioned** production frame (P0-1 `seq + u32 len + payload` remains benchmark-only and is not the product format):

```
magic:     b"CXE1"          (4 bytes)
frame_ver: u16 LE = 1       (2 bytes)
body_len:  u32 LE           (4 bytes)
body:      body_len bytes   (accepted JSON record including hash+signature fields)
```

Active packs use `<index:020>.open`; sealed immutable packs use `<index:020>.seg`. Sealed objects must not contain truncated records. Active `.open` may detect a truncated final frame and recover the complete prefix (ADR-0013 recovery evidence).

### 4.3 Alternatives considered

| Alternative | Decision | Reason |
|---|---|---|
| Adopt P0-1 framing verbatim as production | Rejected | No magic/version; payload was synthetic bytes, not events |
| CBOR / protobuf bodies | Not selected | Extra schema pipeline complexity vs required JSON Schema/TS generation |
| One JSON file per event | Rejected | ADR-0013 |

## 5. Integrity: hash, signature, chain (proposed)

### 5.1 Content hash

- Algorithm: **BLAKE3** producing 32 bytes (hex-encoded lowercase for JSON fields)
- Input: canonical signed JSON body bytes from §4.1
- Library: official `blake3` crate (exact pin at implementation)

### 5.2 Device signature

- Algorithm: **Ed25519**
- Signed message: raw 32-byte `content_hash` (not the hex string)
- Private key: device signing key material in OS SecureStore (ADR-0014); public key retained in daemon-owned public device metadata as needed for local verify
- Library: `ed25519-dalek` (exact pin at implementation)
- M1 stored opaque secrets without defining algorithm; **M2 acceptance of this ADR also pins Ed25519 as the device event-signing algorithm for new keys created under M2 paths**. Migration of any pre-existing opaque M1 synthetic blobs is out of scope unless they were already Ed25519 material in tests.

### 5.3 Integrity chain

- Each accepted event on a `device_id` includes `prev_content_hash` of the immediately prior accepted event for that device (zeros for the first).
- Verification must fail closed on break, mismatch, bad signature, or unknown key.
- Wall-clock must not authorize acceptance.
- Replication transport success is not semantic acceptance (M8 still owns trust workflows; M2 owns local validate/quarantine).

### 5.4 Alternatives considered

| Alternative | Decision | Reason |
|---|---|---|
| SHA-256 + Ed25519 | Viable alternative | More ubiquitous; slower hashing. Owner may select this instead of BLAKE3 without changing envelope shape. |
| HMAC-only without public signatures | Rejected | Weakens multi-device attribution/tamper-evidence goals |
| Custom hash/sign constructions | Rejected | Violates “no custom cryptography” |

**Owner choice requested if preferring SHA-256:** keep §3 envelope; replace BLAKE3 with SHA-256 and hex length 64 remains.

## 6. Schema-version behavior (proposed)

- Field: `schema_version` positive `u32`
- M2 writer emits `1` only
- Reader policy for M2:
  - `1` → validate against v1 schema
  - any other value → **do not trust**; quarantine as incompatible (no silent interpret-as-v1)
- Compatibility windows beyond exact `1` are deferred to a later versioning ADR/milestone unless this ADR is revised before acceptance

## 7. Quarantine (proposed)

Root (outside the Git repo; under the selected Cryptex app-data / runtime root):

```
<app-data-root>/quarantine/canonical-events/<device_id_hex>/<utc_timestamp>_<reason_code>/
  object.bin          # preserved unretrusted bytes
  quarantine.json     # UTF-8 LF metadata: reason, detected_at, source_path, schema_version_if_parsed, hash_if_computed
```

Rules:

- Never silently drop corrupt / incompatible / unverifiable canonical input
- Quarantine is local operational state; not a substitute for canonical history
- Do not treat quarantined objects as trusted application state
- Logs redacted: no memory bodies, secrets, or recovery material by default

## 8. Physical layout and seal policy (proposed)

### 8.1 Layout

```
<app-data-root>/canonical/events/<device_id_hex>/
  00000000000000000000.open   # at most one active pack per device
  00000000000000000000.seg    # sealed immutable packs
  ...
```

- `device_id_hex` uses the existing ADR-0015 path-safe lowercase-hex filename encoding
- No symlink dependence for canonical data
- Portable across Windows and macOS

### 8.2 Seal-roll policy

- Seal when the active pack reaches `seal_event_threshold` complete records **or** an explicit seal/roll request from the daemon
- Suggested **default configuration value:** `1000`
- This default is **not** a frozen production protocol constant; it must remain configurable and must not be hardcoded as an unchangeable architectural constant
- P0-1 profile value is evidence context only (ADR-0013)

## 9. Schema and TypeScript generation (proposed)

- Rust domain types in `crates/events` are source of truth
- Generate JSON Schema into `packages/schemas/` (path may be refined at implementation if workspace layout requires it)
- Generate TypeScript bindings consumed by Desktop/packages without hand-maintained drift
- Proposed toolchain candidates (pin exact versions only after acceptance + lockfile update):
  - JSON Schema: `schemars`
  - TypeScript: `ts-rs` **or** schema→TS via a small checked generator — **prefer one path**; recommended default is `schemars` + `ts-rs` both derived from the same Rust types, with CI drift checks failing on dirty generated output
- CI drift check becomes mandatory once generation exists (`docs/19`, `docs/26`)

## 10. Writer boundary and atomicity (proposed)

- Only `cryptexd` may append/seal canonical event packs
- Desktop/CLI/MCP/Obsidian/AI request writes through daemon APIs/IPC
- Append path: write complete frame → `sync_data`/`sync_all` as required by platform tests → publish sequence only after durable success
- Seal path: sync → atomic rename `.open` → `.seg`
- Failed/interrupted writes must not cause incomplete frames to be treated as valid sealed history
- Existing sealed bytes are never edited in place

## 11. Explicit non-goals

- M3 projection/search
- M8 pairing/revocation/replication product acceptance workflows
- M10 purge/forget product semantics (pack-scoped purge complexity remains acknowledged only)
- Attachment blob product store (M9)
- Claiming Syncthing measurements as Cryptex semantic replication

## 12. Acceptance consequences

If ACCEPTED:

1. M2 product slices in `evidence/m2/m2-kickoff-plan-20261007.md` §5 become authorized under this decision.
2. Dependency pins for selected crates are added in the implementing slice (not this ADR text alone).
3. ADR index updated to ACCEPTED.

If REJECTED / REVISED:

1. Dependent product code remains blocked.
2. Owner revisions supersede the proposed selections above.

## 13. Owner review asks

Please accept, revise, or reject the following blocking choices:

1. Envelope fields in §3 (including minimal `event_type` set)
2. UTF-8 JSON body + versioned `CXE1` segment framing (§4)
3. BLAKE3-256 + Ed25519 (+ libraries) vs SHA-256 alternative (§5)
4. Exact `schema_version == 1` only for M2 readers (§6)
5. Quarantine layout (§7)
6. Per-device `.open`/`.seg` naming + configurable seal default 1000 (§8)
7. `schemars` + `ts-rs` generation approach (§9)
