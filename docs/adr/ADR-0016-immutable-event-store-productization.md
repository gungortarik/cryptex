# ADR-0016 — Immutable Event Store Productization

**Status:** PROPOSED (revised; awaiting owner review)

**Date drafted:** 2026-10-07

**Date revised:** 2026-10-07 (canonicalization, signing-key lifecycle, resource limits, quarantine safety, single schema→TS path)

**Milestone:** M2 — Immutable Event Store

**Authority:** `docs/00_MASTER_SPEC.md`, `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`, `docs/15_PLATFORM_MACOS_WINDOWS.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/19_REPOSITORY_CURSOR_AND_CI.md`, `docs/21_DEFINITION_OF_DONE.md`, `docs/26_VERSIONING_MIGRATIONS_AND_UPDATES.md`, `docs/milestones/M2_IMMUTABLE_EVENT_STORE.md`, ADR-0013 (ACCEPTED), ADR-0014 (ACCEPTED), ADR-0015 (ACCEPTED), `AGENTS.md`, `evidence/m0/p0-1/`, `evidence/m2/m2-kickoff-plan-20261007.md`

This record proposes the **M2 productization choices** left explicitly unresolved after ADR-0013 and the event/storage/security specs.

It deliberately does **not**:

- reopen ADR-0013’s packaging architecture (sealed per-device segments remain accepted; one-file-per-event remains rejected; hybrid remains unselected)
- freeze P0-1 benchmark profile `segment_events=1000` as a protocol constant
- implement M3 SQLite/search, M8 pairing/revocation/replication product workflows, or M10 purge product workflows
- invent custom cryptography
- select exact Cargo/npm dependency pins (pins belong to the implementing slice after acceptance)

**Not accepted.** Status remains **PROPOSED**. Dependent product implementation must not proceed until this ADR (or an accepted narrower successor covering the blocking choices) is ACCEPTED by the owner.

## 1. Context

M2 must productize sealed per-device segment packaging, Rust-sourced JSON Schema + TypeScript bindings, atomic durable canonical writes, hashes/signatures/integrity-chain behavior, quarantine, and schema-version behavior.

Authoritative docs require those behaviors but leave field names, serialization, crypto libraries, quarantine layout, segment filenames, and seal-roll policy unsettled (`docs/04` §22, `docs/06` §16, `docs/08` §21). No authoritative Cryptex document selects a JSON canonicalization standard; none conflicts with adopting RFC 8785 (JSON Canonicalization Scheme). `docs/08` explicitly leaves exact key-rotation policy beyond later accepted specifications unresolved.

## 2. Decision summary (proposed)

| Topic | Proposed selection |
|---|---|
| Canonical JSON for hashed/signed bytes | RFC 8785 JCS (§4) |
| Hash | BLAKE3-256 with fixed domain-separated preimage (§5) |
| Signature | Ed25519 over domain-separated preimage of the content hash (§5) |
| Signing keys | Daemon-owned Ed25519; SecureStore private seed; immutable public binding to `device_id` (§6) |
| Frame/event/payload limits | Explicit maxima before allocation (§7) |
| Truncation vs mid-pack corruption | Recover complete `.open` prefix; quarantine mid-pack / sealed corruption (§7) |
| Quarantine | Collision-resistant, no-replace; safe for unattributed IDs; relative source only (§8) |
| Schema → TypeScript | Single path: Rust → `schemars` JSON Schema → TS from that schema (§10) |
| Production `event_type` | `memory.note` only in production schema; no fixture-only production types (§3) |
| Segment layout / seal default | Per-device `.open`/`.seg`; configurable default 1000 (§9) |
| Writer boundary | `cryptexd` only (§11) |

## 3. Logical event envelope (proposed)

Canonical events are immutable once accepted. Corrections are additional events, never in-place edits.

### 3.1 Required envelope fields

| Field | Type / encoding | In JCS signable object? | Role |
|---|---|---|---|
| `schema_version` | JSON number, integer `1` for M2 | yes | Event schema version |
| `event_id` | UUID string | yes | Stable event identity |
| `brain_id` | UUID string | yes | Must match local Brain identity (ADR-0015) |
| `device_id` | path-safe string (ADR-0015) | yes | Trusted-device principal |
| `device_seq` | JSON number, integer ≥ 1 | yes | Per-device sequence; first usable `1`; never `0` |
| `created_at` | RFC 3339 string | yes | Display/timeline only — never security authority |
| `event_type` | string enum | yes | Payload discriminator |
| `classification` | `public` \| `internal` \| `private` \| `restricted` | yes | Privacy boundary |
| `signing_key_id` | lowercase hex string (32 chars = first 16 bytes of SHA-256 of the 32-byte Ed25519 public key) | yes | Which device key produced `signature` |
| `payload` | JSON object | yes | Type-specific body |
| `prev_content_hash` | 64-char lowercase hex | yes | Prior accepted event on same `device_id`, or 64 zero nibbles for the device’s first event |
| `content_hash` | 64-char lowercase hex | **no** (computed) | §5 |
| `signature` | 128-char lowercase hex (64-byte Ed25519 signature) | **no** (computed) | §5 |

The **signable object** is the JSON object containing exactly the “yes” fields above (and no others). `content_hash` and `signature` are attached after hashing/signing to form the **accepted on-disk record**.

### 3.2 Production `event_type` values (M2)

| `event_type` | Payload | Notes |
|---|---|---|
| `memory.note` | `{ "text": <string> }` | Sole production type in the M2 v1 schema |

**Fixture-only / synthetic probe types must not appear in the production event schema or generated bindings.** Integrity, quarantine, and corruption tests construct synthetic bytes, truncated frames, or invalid records in test code without extending the production `event_type` enum.

Later milestones may extend types via schema evolution rules. M2 must not implement knowledge consolidation, attachment product storage (M9), or purge product events (M10).

### 3.3 Alternatives considered

| Alternative | Decision | Reason |
|---|---|---|
| Defer field names to implementation | Rejected | Blocks reviewable schema generation |
| Full V1 memory/knowledge taxonomy in M2 | Rejected | Scope creep |
| Production `system.integrity_probe` type | Rejected | Fixture-only types must stay out of the production schema |
| Binary-only logical model | Not selected | Conflicts with required JSON Schema/TS generation |

## 4. Canonical JSON (RFC 8785 JCS) (proposed)

All hashed and signed event bytes use **RFC 8785 JSON Canonicalization Scheme (JCS)**. No Cryptex authoritative document conflicts with this choice. Ordinary M1 pretty-printed metadata is unrelated and is not the event hashing standard.

### 4.1 Parser requirements before canonicalization

Before producing JCS bytes or trusting a record:

1. **UTF-8 only.** Reject invalid UTF-8.
2. **Duplicate keys:** reject any JSON object with duplicate member names (fail closed; do not last-key-wins).
3. **No insignificant JSON extensions:** reject trailing commas, comments, `NaN`, `Infinity`, `-Infinity`, and non-JSON numeric forms.
4. **Numbers accepted in signable objects:**
   - `schema_version`: integer in range `[1, 2147483647]`
   - `device_seq`: integer in range `[1, 2^53−1]` (JCS/I-JSON safe integer range for interoperable JSON numbers)
   - Payload numbers, if any in later types, must be finite and within `[-2^53+1, 2^53−1]` unless a later ADR widens this with an explicit encoding
5. **Unicode:** decode JSON strings per RFC 8259; JCS output follows RFC 8785 (UTF-8; escapable characters escaped as required by JCS). Canonical text remains UTF-8 with LF for non-JCS human metadata elsewhere; **JCS event bytes themselves contain no insignificant whitespace** (JCS compact form).
6. **Nesting / size:** enforce §7 limits before allocating unbounded structures.

### 4.2 Hash preimage and domain separation

Let `J` be the RFC 8785 JCS byte string of the signable object (§3.1).

**Content hash preimage** (exact bytes):

```
preimage_hash = UTF-8("cryptex-event-content-v1") || 0x00 || J
content_hash_raw = BLAKE3-256(preimage_hash)   # 32 bytes
content_hash_hex = lowercase hex(content_hash_raw)  # 64 chars
```

**Signature preimage** (exact bytes signed by Ed25519):

```
preimage_sig = UTF-8("cryptex-event-sig-v1") || 0x00 || content_hash_raw
signature_raw = Ed25519.Sign(device_private_key, preimage_sig)  # 64 bytes
signature_hex = lowercase hex(signature_raw)  # 128 chars
```

Verification recomputes `J` from the parsed signable fields, recomputes `content_hash_raw`, checks equality with the record’s `content_hash`, then verifies Ed25519 over `preimage_sig` with the public key immutably bound to `device_id` + `signing_key_id` (§6). Fail closed on any mismatch.

Domain separation strings are fixed protocol constants for schema_version `1`. They are labels over standard BLAKE3/Ed25519, not custom primitives.

### 4.3 On-disk accepted record

The accepted frame body is the JCS (or equivalently unique) JSON object that is the signable object plus `content_hash` and `signature` members. For storage convenience, the body MAY be stored as JCS of the full accepted object (signable fields + `content_hash` + `signature`) provided round-trip equality is tested; verifiers must ignore unknown members only if a later schema policy allows them — **M2 verifiers reject unknown members** at schema_version `1`.

### 4.4 Segment record framing

P0-1 `seq + u32 len + payload` remains benchmark-only and is **not** the product format.

```
magic:     b"CXE1"          (4 bytes)
frame_ver: u16 LE = 1       (2 bytes)
body_len:  u32 LE           (4 bytes)
body:      body_len bytes   (accepted JSON record; see limits in §7)
```

Active packs: `<index:020>.open`. Sealed immutable packs: `<index:020>.seg`.

## 5. Integrity algorithms (proposed)

| Item | Selection |
|---|---|
| Hash | BLAKE3-256 |
| Signature | Ed25519 |
| Libraries (names only; **no version pins in this ADR**) | `blake3`, `ed25519-dalek` |
| Chain | `prev_content_hash` links prior accepted event on the same `device_id` |

Wall-clock must not authorize acceptance. Replication transport success is not semantic acceptance. Integrity/signature failures are never silently swallowed.

**Rejected:** HMAC-only (weak multi-device attribution); custom hash/sign constructions; adopting P0-1 synthetic payloads as integrity evidence.

**Not selected here as a second option:** SHA-256. If the owner prefers SHA-256, revise §4.2/§5 explicitly; do not leave dual algorithms in production.

## 6. Device signing-key lifecycle (proposed; M8 workflows out of scope)

This section defines **local daemon-owned event-signing key material** for M2. It does **not** productize pairing, revoke/retire/compromise, or cross-device trust UX (M8). Offline Recovery Kit signing secrets remain out of normal runtime (ADR-0015 / `docs/08`).

### 6.1 Creation

- Only `cryptexd` may create device event-signing keys.
- Algorithm: Ed25519.
- When a local `device_id` first becomes eligible to append canonical events, the daemon generates a new Ed25519 keypair.
- Private material: **32-byte Ed25519 seed** stored via ADR-0014 SecureStore.
- Suggested SecureStore labeling (non-secret): service `Cryptex-DeviceCredentials` / `Cryptex-Dev-DeviceCredentials`; account `device-event-signing-seed:{device_id}` (distinct from any pre-M2 synthetic opaque blobs under `device-private:{device_id}`).
- Public key: 32-byte Ed25519 public key.

### 6.2 Public-key persistence and immutable binding

Daemon-owned public metadata (UTF-8 JSON, LF), exclusive create / no-replace on first write:

```
<app-data-root>/identity/devices/<device_id_hex>/event_signing_key.json
```

Minimum fields:

| Field | Role |
|---|---|
| `schema_version` | metadata format version (`1`) |
| `device_id` | must equal the directory’s device |
| `signing_key_id` | as in §3.1 |
| `public_key` | 64-char lowercase hex (32-byte Ed25519 public key) |
| `created_at` | RFC 3339 display only |

Rules:

- The binding `(device_id → signing_key_id → public_key)` is **immutable** for M2 once written. No in-place overwrite of `public_key` for that `signing_key_id`.
- `signing_key_id` in every event must match this bound key for local accept of locally authored events.
- Path uses ADR-0015 path-safe `device_id_hex` encoding only after `device_id` validates; never place raw untrusted strings into path components.

### 6.3 Missing or corrupt key material (fail closed)

| Condition | Behavior |
|---|---|
| SecureStore seed missing/unreadable | Typed error; **do not** append events; **do not** silently mint a replacement key that would break attribution |
| Public metadata missing while seed exists (or vice versa) | Typed inconsistency error; fail closed |
| Public metadata corrupt / wrong `device_id` / unsupported schema | Typed error; fail closed |
| Signature verify key unknown for `signing_key_id` | Reject event; quarantine as unverifiable |

Deliberate recovery of lost keys is not silent heal; product recovery workflows are later milestones.

### 6.4 Rotation expectations (bounded)

- **M2 local authoring:** exactly one active event-signing key per local `device_id`.
- **Full key-rotation policy** (multi-key history, grace periods, cross-device distribution of rotated keys, revocation interplay) remains **unanswered** by `docs/08` §21 and is **not invented here**. M8 may extend this with an ADR addendum.
- Until a rotation ADR exists, M2 must not silently rotate or replace the bound public key.

## 7. Resource limits, truncation, and corruption (proposed)

### 7.1 Limits enforced before allocation

| Limit | Value | Enforcement |
|---|---|---|
| Max `body_len` / accepted record bytes | `1_048_576` (1 MiB) | Reject/quarantine before allocating body buffer larger than this |
| Max `payload.text` UTF-8 bytes (`memory.note`) | `524_288` (512 KiB) | Schema + runtime check |
| Max JSON nesting depth | `32` | Parser hard stop |
| Max JSON object/array elements per container | `4096` | Parser hard stop |
| Max total JSON string bytes decoded per record | `1_048_576` | Parser hard stop |
| Max frames examined in one scan batch | implementation may stream; must not load entire multi-MiB packs unbounded into a single growable buffer without the above per-record caps | Streaming parse |

`frame_ver ≠ 1` or `magic ≠ CXE1` → incompatible/corrupt handling (§7.2), never silent reinterpretation.

### 7.2 Truncation vs corruption

| Case | Behavior |
|---|---|
| Active `.open`: final frame truncated (incomplete header or body) | **Recover** all complete prior frames (ADR-0013 crash/recovery evidence). Truncated suffix is not a trusted event. Do not seal a pack that still ends mid-frame. |
| Active `.open`: complete frame fails schema/JCS/hash/signature/duplicate-key/limit checks | Treat as **corruption / unverifiable input**: do not accept; **quarantine** that frame’s raw bytes (and stop trusting subsequent bytes in that pack until operator/repair policy — M2 default: refuse further append until the active pack is quarantined/rolled per daemon repair path). Complete prior accepted frames remain readable. |
| Sealed `.seg`: any truncated record, bad frame, or verification failure | **Quarantine the whole sealed object**; never silently skip bad records inside a sealed pack |
| Limits exceeded | Reject/quarantine; never allocate based on attacker-controlled unbounded lengths |

Existing sealed bytes are never edited in place.

## 8. Quarantine (proposed)

### 8.1 Layout (collision-resistant, no-replace)

```
<app-data-root>/quarantine/canonical-events/
  <attribution_dir>/
    <utc_compact>_<reason_code>_<entry_id_hex>/
      object.bin         # raw rejected bytes (retained)
      quarantine.json    # redacted metadata (UTF-8 LF)
```

`attribution_dir`:

- If `device_id` is present **and** passes ADR-0015 path-safe validation: `device-<device_id_hex>`
- Otherwise (missing, malformed, path-unsafe, or unparsed): `unattributed`

`entry_id_hex`: lowercase hex of BLAKE3-256 over `object.bin` (or over available rejected bytes). Directory creation must use **exclusive create / no-replace**. If the target exists, append a monotonic suffix derived from a daemon counter or nanoseconds **without overwriting** existing quarantine entries.

### 8.2 Retained bytes and metadata

- **Raw rejected bytes are retained** in `object.bin` for operator visibility and diagnostics (explicit failure, not silent drop).
- `quarantine.json` minimum fields: `schema_version`, `reason_code`, `detected_at` (RFC 3339), `attribution` (`device_id` if validated else null), `entry_id`, `byte_length`, `source_kind` (`active_open` \| `sealed_seg` \| `import` \| `other`), `source_relative` (pack-relative path using only validated components, e.g. `canonical/events/<device_id_hex>/<index>.open` or `unresolved`), optional `schema_version_if_parsed`, optional `content_hash_if_computed`.
- **Do not store absolute filesystem paths**, user home prefixes, or other host-sensitive absolute source paths in quarantine metadata or default logs.
- Quarantine is local operational state, not canonical history and not trusted application state.
- Logs must not include memory bodies, secrets, recovery material, or credentials by default.

## 9. Physical layout and seal policy (proposed)

```
<app-data-root>/canonical/events/<device_id_hex>/
  00000000000000000000.open   # at most one active pack per device
  00000000000000000000.seg    # sealed immutable packs
```

- `device_id_hex` only from validated ADR-0015 encoding; no symlink dependence; portable Windows/macOS.
- Seal when complete records reach configurable `seal_event_threshold`, or on explicit daemon seal/roll.
- Suggested **default configuration value:** `1000` — **tunable config only**, not a frozen protocol constant (ADR-0013).

## 10. Schema and TypeScript generation (proposed)

**Single generation path** (no parallel `ts-rs` derivation):

1. Rust domain types in `crates/events` are the source of truth (production types only; §3.2).
2. Generate JSON Schema with `schemars` into `packages/schemas/` (exact filenames fixed at implementation; checked in).
3. Generate TypeScript types **from those JSON Schema artifacts** via one checked generator (tool name pinned at implementation; e.g. `json-schema-to-typescript` or equivalent).
4. **Drift check:** regenerating schemas and TypeScript in CI/local must produce a clean `git diff` (fail on drift). Hand-edited generated outputs are rejected.

Fixture-only constructs stay in Rust tests and must not be emitted into `packages/schemas` or production TS bindings.

## 11. Writer boundary and atomicity (proposed)

- Only `cryptexd` may append/seal canonical event packs.
- Clients request writes through daemon APIs/IPC.
- Append: write complete frame → platform sync as required by tests → return success only after durability confirmed; allocate/publish `device_seq` only under ADR-0015 durable-before-return rules.
- Seal: sync → atomic rename `.open` → `.seg`.
- Incomplete frames are never treated as valid sealed history.

## 12. Cross-check against authoritative invariants

| Invariant | ADR-0016 alignment |
|---|---|
| Master Spec: immutable events; cryptexd sole writer; offline-capable; no device master | §3, §11; no master device assumed |
| Master Spec / `docs/04`: attributable, tamper-evident; failures not swallowed | §4–§5, §7–§8 |
| ADR-0013: sealed per-device segments; not one-file-per-event; `segment_events=1000` not frozen | §4.4, §9 |
| ADR-0014/0015: SecureStore private material; sequence foundation; fail-closed missing/corrupt | §6, `device_seq` rules |
| M2 milestone scope / non-goals | No M3/M8/M10 product behavior |
| Platform: UTF-8; no symlink canonical data; Win/macOS first-class | §4, §8–§9 |
| Schema drift architecture (`docs/19`, `docs/26`) | §10 single path + drift check |
| `docs/08` unresolved key-rotation policy | §6.4 leaves full rotation unanswered |

## 13. Remaining unanswered decisions (not invented)

These remain explicitly unanswered after this revision:

1. **Full signing-key rotation / multi-key verification policy** across devices and time (`docs/08` §21) — beyond M2’s single active local key.
2. **Cross-device trust acceptance workflows** for foreign device signatures (M8).
3. **Exact crate/npm version pins** for `blake3`, `ed25519-dalek`, `schemars`, and the JSON Schema→TS generator (implementation slice after acceptance).
4. **Schema compatibility windows** beyond M2 exact `schema_version == 1`.
5. **Production seal-threshold tuning guidance** beyond “configurable; default 1000 as config only.”
6. **Operator repair UX** for quarantined active packs (M2 requires fail-closed + quarantine; richer repair UI is later).

## 14. Explicit non-goals

- M3 projection/search
- M8 pairing/revocation/replication product acceptance workflows
- M10 purge/forget product semantics
- M9 attachment blob product store
- Treating Syncthing transport as semantic acceptance
- Marking this ADR accepted without owner review

## 15. Acceptance consequences

If ACCEPTED later:

1. M2 product slices become authorized under this decision.
2. Dependency pins are added in implementing commits (not by this ADR alone).
3. ADR index status moves to ACCEPTED.

Until then: **PROPOSED — awaiting owner review.** Dependent product code, dependency pins, and GitHub Actions for M2 product slices remain out of scope for this revision.

## 16. Owner review asks

Please accept, revise, or reject the decision table in §2, especially:

1. RFC 8785 JCS + duplicate-key rejection + numeric/Unicode rules (§4)
2. Exact hash/signature preimages and domain separation labels (§4.2)
3. Daemon signing-key lifecycle + immutable public binding + fail-closed (§6)
4. Resource limits and truncation/corruption matrix (§7)
5. Quarantine safety / retention / no absolute paths (§8)
6. Single schemars→JSON Schema→TS path + drift check; production `memory.note` only (§3, §10)
