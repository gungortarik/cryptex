# ADR-0016 — Immutable Event Store Productization

**Status:** ACCEPTED

**Date drafted:** 2026-10-07

**Date accepted:** 2026-10-07 (owner acceptance of record `5cc89b709dcf236698c3253b8582c873de7c7f0f`)

**Date revised:** 2026-10-07 (canonicalization, signing-key lifecycle, resource limits, quarantine safety, single schema→TS path; full-`u64` `device_seq` decimal string; crash-safe key init; ordered `.open` recovery)

**Milestone:** M2 — Immutable Event Store

**Authority:** `docs/00_MASTER_SPEC.md`, `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`, `docs/15_PLATFORM_MACOS_WINDOWS.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/19_REPOSITORY_CURSOR_AND_CI.md`, `docs/21_DEFINITION_OF_DONE.md`, `docs/26_VERSIONING_MIGRATIONS_AND_UPDATES.md`, `docs/milestones/M2_IMMUTABLE_EVENT_STORE.md`, ADR-0013 (ACCEPTED), ADR-0014 (ACCEPTED), ADR-0015 (ACCEPTED), `AGENTS.md`, `evidence/m0/p0-1/`, `evidence/m2/m2-kickoff-plan-20261007.md`

This record proposes the **M2 productization choices** left explicitly unresolved after ADR-0013 and the event/storage/security specs.

It deliberately does **not**:

- reopen ADR-0013’s packaging architecture (sealed per-device segments remain accepted; one-file-per-event remains rejected; hybrid remains unselected)
- freeze P0-1 benchmark profile `segment_events=1000` as a protocol constant
- implement M3 SQLite/search, M8 pairing/revocation/replication product workflows, or M10 purge product workflows
- invent custom cryptography
- select exact Cargo/npm dependency pins (pins belong to the implementing slice after acceptance)

**Accepted.** Owner accepted this ADR as recorded at `5cc89b709dcf236698c3253b8582c873de7c7f0f` on 2026-10-07. M2 product slices may proceed under this decision. Exact crate/npm version pins remain an implementing-slice concern (see §13).

## 1. Context

M2 must productize sealed per-device segment packaging, Rust-sourced JSON Schema + TypeScript bindings, atomic durable canonical writes, hashes/signatures/integrity-chain behavior, quarantine, and schema-version behavior.

Authoritative docs require those behaviors but leave field names, serialization, crypto libraries, quarantine layout, segment filenames, and seal-roll policy unsettled (`docs/04` §22, `docs/06` §16, `docs/08` §21). No authoritative Cryptex document selects a JSON canonicalization standard; none conflicts with adopting RFC 8785 (JSON Canonicalization Scheme). `docs/08` explicitly leaves exact key-rotation policy beyond later accepted specifications unresolved.

## 2. Decision summary (proposed)

| Topic | Proposed selection |
|---|---|
| Canonical JSON for hashed/signed bytes | RFC 8785 JCS (§4) |
| `device_seq` encoding | Canonical decimal **JSON string**; strict parse to full ADR-0015 `u64` (§3.1, §4.1) |
| Hash | BLAKE3-256 with fixed domain-separated preimage (§5) |
| Signature | Ed25519 over domain-separated preimage of the content hash (§5) |
| Signing keys | Daemon-owned Ed25519; crash-safe SecureStore + public metadata init; immutable binding (§6) |
| Frame/event/payload limits | Explicit maxima before allocation (§7) |
| `.open` recovery ordering | Quarantine untrusted suffix durably → truncate to last verified frame → sync → then append (§7.2) |
| Quarantine | Collision-resistant, no-replace; safe for unattributed IDs; relative source only (§8) |
| Schema → TypeScript | Single path: Rust → `schemars` JSON Schema → TS from that schema; `device_seq` as string (§10) |
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
| `device_seq` | JSON **string**: canonical decimal digits of a `u64` in `[1, 2^64-1]` (ADR-0015 full counter range; see §3.1.1) | yes | Per-device sequence; first usable `1`; never `0` |
| `created_at` | RFC 3339 string | yes | Display/timeline only — never security authority |
| `event_type` | string enum | yes | Payload discriminator |
| `classification` | `public` \| `internal` \| `private` \| `restricted` | yes | Privacy boundary |
| `signing_key_id` | lowercase hex string (32 chars = first 16 bytes of SHA-256 of the 32-byte Ed25519 public key) | yes | Which device key produced `signature` |
| `payload` | JSON object | yes | Type-specific body |
| `prev_content_hash` | 64-char lowercase hex | yes | Prior accepted event on same `device_id`, or 64 zero nibbles for the device’s first event |
| `content_hash` | 64-char lowercase hex | **no** (computed) | §5 |
| `signature` | 128-char lowercase hex (64-byte Ed25519 signature) | **no** (computed) | §5 |

The **signable object** is the JSON object containing exactly the “yes” fields above (and no others). `content_hash` and `signature` are attached after hashing/signing to form the **accepted on-disk record**.

### 3.1.1 `device_seq` and ADR-0015 `u64` reconciliation

ADR-0015 allocates a durable monotonic **`u64`** sequence per `device_id` (first usable value `1`; `0` reserved for "none issued yet"). M2 must not silently cap that counter at `2^53-1`.

No authoritative Cryptex document requires JSON numbers for sequence fields. This ADR therefore encodes `device_seq` as a **canonical decimal JSON string**:

1. Wire form: JSON string matching `^[1-9][0-9]*$` with no sign, whitespace, decimal point, exponent, or leading zeros.
2. Semantic parse: interpret the digit string as a base-10 integer and accept only if it fits in `u64` and is ≥ `1` (i.e. `[1, 18446744073709551615]`). Reject otherwise with a typed error / quarantine as invalid.
3. Authors obtain values only from the cryptexd ADR-0015 allocator; the event field must equal the allocated `u64` rendered as that canonical decimal string.
4. **Schema / TypeScript consequences:** JSON Schema type is `string` (not `integer`/`number`) with the pattern above plus implementation/runtime `u64` range enforcement; generated TypeScript type is `string`. Domains that need numeric comparison parse with the same strict `u64` rules—never via IEEE-754 JSON numbers.
5. **Rejected alternative:** JSON number for `device_seq` (loses values above `2^53-1` under ordinary JSON number semantics).


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
4. **Numbers and `device_seq` in signable objects:**
   - `schema_version`: JSON number, integer in range `[1, 2147483647]`
   - `device_seq`: **JSON string** per §3.1.1 (full ADR-0015 `u64` range). A JSON number for `device_seq` is invalid.
   - Payload numbers, if any in later types, must be finite and within `[-2^53+1, 2^53-1]` unless a later ADR widens this with an explicit encoding
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

### 6.1 Material and locations

- Only `cryptexd` may create or complete device event-signing key initialization.
- Algorithm: Ed25519.
- Private material: **32-byte Ed25519 seed** via ADR-0014 SecureStore.
- Suggested SecureStore labeling (non-secret): service `Cryptex-DeviceCredentials` / `Cryptex-Dev-DeviceCredentials`; account `device-event-signing-seed:{device_id}` (distinct from pre-M2 synthetic blobs under `device-private:{device_id}`).
- Public metadata path (UTF-8 JSON, LF):

```
<app-data-root>/identity/devices/<device_id_hex>/event_signing_key.json
```

Minimum public fields: `schema_version` (`1`), `device_id`, `signing_key_id` (as in §3.1), `public_key` (64-char lowercase hex), `created_at` (RFC 3339 display only).

Path uses ADR-0015 path-safe `device_id_hex` only after `device_id` validates.

### 6.2 Crash-safe, idempotent initialization

Initialization is a daemon-owned state machine. It must be safe to retry after crash and must **never** silently replace a key that is already bound to existing accepted events.

**Observe first** (every call): read SecureStore seed presence/readability; read public metadata presence/parse; determine whether any local accepted event for this `device_id` already references a `signing_key_id`.

**Fresh create** (seed absent, metadata absent, and no local accepted events for `device_id`):

1. Generate Ed25519 keypair in memory (seed + public key).
2. Persist the 32-byte seed to SecureStore and confirm durability of that put.
3. Persist public metadata with **exclusive create / no-replace** (atomic temp+rename+sync pattern consistent with cryptexd public-metadata durability), binding `device_id` / `signing_key_id` / `public_key`.
4. Re-read both sides and verify seed-derived public key equals metadata before returning success.
5. If step 3 or 4 fails after step 2 succeeded, leave the seed in place and return a typed incomplete-init error. The next init call must follow the recovery table below—**not** generate a different seed.

**Idempotent success:** seed present, metadata present, metadata parses, seed-derived public key equals metadata `public_key`, and `signing_key_id` matches → return ready (no writes).

### 6.3 Partial-state recovery matrix (fail closed; no silent key replacement)

| Observed state | Existing accepted events for `device_id`? | Required behavior |
|---|---|---|
| Seed present; public metadata missing | No | **Complete init:** derive public key + `signing_key_id` from the existing seed; exclusive-create metadata; verify match; never generate a new seed |
| Seed present; public metadata missing | Yes | **Fail closed** with typed error. Do not mint a new seed. Best-effort completion from the existing seed is allowed only if derived `signing_key_id`/`public_key` match those referenced by existing events; if derivation cannot satisfy that binding, refuse authoring pending deliberate recovery (not silent replace) |
| Public metadata present; seed missing/unreadable | No or Yes | **Fail closed.** Do not delete/replace metadata. Do not mint a new seed that would diverge from the bound `public_key`. Authoring remains blocked until deliberate recovery restores the matching seed or a later rotation ADR applies |
| Seed and metadata present but mismatch (derived pubkey ≠ metadata, wrong `device_id`, corrupt/unsupported metadata schema) | No or Yes | **Fail closed.** Do not overwrite either side to "heal." |
| Seed and metadata consistent | Yes, and events' `signing_key_id` equals bound id | Ready; continue authoring with that key only |
| Seed and metadata consistent | Yes, but some accepted event references a different `signing_key_id` | **Fail closed** for local authoring under M2's single-active-key rule; do not silently replace keys. Multi-key history is unanswered (§6.4) |
| Either side missing/corrupt while events exist that need that key | Yes | **Never silently replace** the key bound to those events. Typed hard failure until deliberate recovery |

Signature verify with unknown `signing_key_id` → reject event and quarantine as unverifiable (§8). Deliberate lost-key recovery UX is not silent heal; product recovery workflows are later milestones.

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

### 7.2 `.open` recovery ordering (no data loss of untrusted suffix)

Define:

- **Verified prefix:** longest leading sequence of complete frames that each pass framing, limits, schema/JCS, hash, and signature verification.
- **Untrusted suffix:** every byte after the verified prefix (truncated final frame and/or corrupt complete frame and any bytes after it).
- **Corrupt-frame case:** a complete frame that fails validation, plus all remaining bytes through EOF, form the untrusted suffix. The verified prefix stops immediately before that frame.

**Mandatory ordered recovery for an active `.open` before any further append or seal:**

1. Compute verified-prefix length `L` and untrusted-suffix bytes `[L, EOF)`.
2. If the suffix is non-empty, **quarantine those exact suffix bytes first** under §8 (`object.bin` + `quarantine.json`) using exclusive create / no-replace. Quarantine metadata must note `source_kind=active_open`, relative source, byte offset `L`, and reason (`truncated_tail` or `corrupt_frame` / more specific codes).
3. Confirm the quarantine entry is **durable** (files synced as required by platform durability tests).
4. **Only then** truncate the `.open` file to length `L` (verified prefix retained in place). Do not truncate before durable quarantine.
5. **Sync** the truncated `.open` (and parent directory on platforms where directory sync is required for rename/truncate durability) before returning success.
6. Only after steps 1–5 succeed may the daemon allow further append to that `.open` (or seal it if it ends on a verified frame boundary).

**Fail closed:** if any quarantine, durability confirmation, truncate, or sync step fails, leave the `.open` unchanged for authoring purposes, return a typed error, and **do not append**. Never drop an untrusted suffix without a durable quarantine copy. Never treat an incomplete recovery as success.

| Case | Behavior |
|---|---|
| Active `.open`: final frame truncated | Quarantine truncated suffix, then truncate to last verified frame, then sync (§7.2 steps). Suffix is not a trusted event. |
| Active `.open`: complete frame fails validation | Quarantine **that corrupt frame plus the remaining suffix through EOF**; retain the valid verified prefix via truncate-after-quarantine; sync; refuse append until recovery succeeds |
| Sealed `.seg`: any truncated record, bad frame, or verification failure | **Quarantine the whole sealed object** (no in-place edit); never silently skip bad records inside a sealed pack |
| Limits exceeded | Reject/quarantine per §8; never allocate based on attacker-controlled unbounded lengths |

Existing sealed bytes are never edited in place. Verified prefix frames already accepted remain readable after successful recovery.

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

1. Rust domain types in `crates/events` are the source of truth (production types only; §3.2). `device_seq` is a `String` in the Rust/JSON Schema/TS surfaces, validated by the strict `u64` decimal rules in §3.1.1—not a JSON number / TypeScript `number`.
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
| ADR-0015: full u64 sequence contract; first usable 1; fail-closed missing/corrupt counters | device_seq decimal string with strict u64 parse (§3.1.1); no 2^53-1 cap |
| ADR-0014/0015: SecureStore private material; fail-closed missing/corrupt | Crash-safe key init + partial-state matrix (§6); never silent key replace when events exist |
| Quarantine / no silent data loss of rejected bytes | Ordered .open recovery quarantines suffix before truncate (§7.2, §8) |
| M2 milestone scope / non-goals | No M3/M8/M10 product behavior |
| Platform: UTF-8; no symlink canonical data; Win/macOS first-class | §4, §8–§9 |
| Schema drift architecture (docs/19, docs/26) | §10 single path + drift check; device_seq as string |
| docs/08 unresolved key-rotation policy | §6.4 leaves full rotation unanswered |

## 13. Remaining unanswered decisions (not invented)

These remain explicitly unanswered after this revision:

1. **Full signing-key rotation / multi-key verification policy** across devices and time (`docs/08` §21) — beyond M2's single active local key and the partial-state matrix in §6.3.
2. **Deliberate recovery procedure** that restores a missing SecureStore seed when public metadata (and possibly events) still exist — must not be silent replace; product UX later.
3. **Cross-device trust acceptance workflows** for foreign device signatures (M8).
4. **Exact crate/npm version pins** for `blake3`, `ed25519-dalek`, `schemars`, and the JSON Schema→TS generator (implementation slice after acceptance).
5. **Schema compatibility windows** beyond M2 exact `schema_version == 1`.
6. **Production seal-threshold tuning guidance** beyond "configurable; default 1000 as config only."
7. **Operator repair UX** beyond the mandatory quarantine-then-truncate recovery path (richer UI later).

Resolved by this revision (no longer open): `device_seq` full-`u64` encoding via canonical decimal JSON string; crash-safe key init ordering; exact `.open` recovery ordering for truncated/corrupt suffixes.


## 14. Explicit non-goals

- M3 projection/search
- M8 pairing/revocation/replication product acceptance workflows
- M10 purge/forget product semantics
- M9 attachment blob product store
- Treating Syncthing transport as semantic acceptance
- Marking this ADR accepted without owner review

## 15. Acceptance consequences

**ACCEPTED** on 2026-10-07 against git record `5cc89b709dcf236698c3253b8582c873de7c7f0f`.

1. M2 product slices in `evidence/m2/m2-kickoff-plan-20261007.md` §5 are authorized under this decision.
2. Dependency pins for selected crates are added in implementing commits (not by this ADR text alone).
3. ADR index status is ACCEPTED.


## 16. Owner review asks (historical; accepted)

The owner accepted the decision table in §2 on 2026-10-07. The review asks that were decided were:

1. RFC 8785 JCS + duplicate-key rejection + numeric/Unicode rules (§4)
2. Exact hash/signature preimages and domain separation labels (§4.2)
3. `device_seq` as canonical decimal JSON string with strict full-`u64` parse (§3.1.1)
4. Crash-safe signing-key init + partial-state recovery matrix (§6)
5. Ordered `.open` recovery: durable quarantine of untrusted suffix before truncate+sync (§7.2)
6. Quarantine safety / retention / no absolute paths (§8)
7. Single schemars→JSON Schema→TS path + drift check; production `memory.note` only (§3, §10)
