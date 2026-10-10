# M5 Slice 1 — Capture / reconciliation contract (finalized)

**Status:** ACCEPTED for Slice 1 implementation (owner-authorized 2026-10-10).  
**Branch:** `codex/m5-capture-timeline`  
**Base:** M4 PR #10 merge `a6c32bd30f883e546e2eb976787a59057fae211b`  
**Authority:** `docs/00_MASTER_SPEC.md`, `docs/milestones/M5_CAPTURE_TIMELINE.md`,
ADR-0013 / 0015 / 0016 / 0017 / 0018, `evidence/m5/m5-kickoff-plan-20261010.md`.

This record freezes the Slice 1 daemon capture/reconciliation contract. It does
not authorize Desktop capture IPC/UI, timeline APIs, projection wiring,
extraction, production v2 writing, or later milestones.

## Invariants

- Strict v1 `memory.note` `{text}` bytes/schema/crypto unchanged.
- Production v2 writing remains OFF.
- `cryptexd` is the only canonical writer; Desktop must not write packs/SQLite/keys.
- Explicit Brain + local DeviceId enrollment; never silent mint/replace.
- Explicit `Classification` on every capture request (no invented default).
- No content-based dedupe of intentional identical notes.
- Tip/seal/sync/receipt failures stay typed; never demoted to best-effort success.
- Canonical note recovery must not depend on receipts.

## Request

```text
CaptureRequest { capture_id, text, classification }
```

`brain_id` / `device_id` come only from daemon enrollment. Caller-supplied
trust/device IDs are not authoritative.

## Outcomes

| Outcome | Meaning |
|---|---|
| `Committed` | Predetermined `event_id` integrity-verified in local packs, fingerprint matches, pack durability confirmed, and `COMMITTED` receipt durable. |
| `Rejected` | This call did not commit under a code below. |
| `Uncertain` | Commit neither proved nor disproved. |

### Rejected codes

| Code | Meaning |
|---|---|
| Pre-boundary validation / enrollment | Invalid input or missing identity before boundary work. |
| `capture_id_conflict` | Fingerprint disagrees with existing receipt for the same scope key. Rejects **this** request only; does not prove the original capture was never committed. |
| `not_committed` | `resolve_capture` only: complete verified scan shows receipt `event_id` absent for an existing `PENDING`. |

Write/sync/seal/tip/receipt/verification failure at or after the durability
boundary → `Uncertain`, never `not_committed` merely because an operation failed.
Readable bytes alone must not produce `Committed`.

## Fingerprint v1

Existing BLAKE3-256 with domain separation (`0x00`) and `u32` little-endian
length-prefixed fields:

```text
CAPTURE_FINGERPRINT_DOMAIN = b"cryptex-capture-fingerprint-v1"

preimage =
    DOMAIN || 0x00
    || u32le(len(brain_id)) || brain_id UTF-8
    || u32le(len(device_id)) || device_id UTF-8
    || u32le(len(capture_id)) || capture_id UTF-8
    || u32le(len(class)) || class UTF-8   // public|internal|private|restricted
    || u32le(len(text)) || text UTF-8

fingerprint = lowercase_hex(BLAKE3-256(preimage))
```

## Idempotency epoch and receipt loss

**Guarantee:** While a receipt for `(brain_id, device_id, capture_id)` remains
durable and the fingerprint matches, at most one canonical append occurs for
that key. Concurrent matching requests serialize on the device canonical lock.
Matching replay returns `Committed` or resumes one `PENDING`.

**Missing receipt on `capture_memory_note`:** start a new idempotency epoch
(exclusive-create `PENDING`, then append path). Indistinguishable from reuse
after receipt loss.

**Missing receipt on `resolve_capture`:** `Uncertain { receipt_missing }`. No
append. Not `Rejected`.

**After receipt loss:** The selected design binds `capture_id` → `event_id` only
through the derived receipt. That random mapping cannot provide duplicate
prevention once the receipt is gone. Unchanged v1 packs remain authoritative for
note recovery. This contract does not claim that any post-loss duplicate
prevention would necessarily require a v1 change—only that this mapping does not
provide that guarantee after receipt loss.

**Client flow:** After `receipt_missing`, automatic retries with the same
`capture_id` are prohibited. Keep the draft. A later deliberate Save uses a new
`capture_id` (identical text allowed as a new note).

Receipts live under derived app-data (`derived/capture_receipts/...`), never a
second canonical store and never a recovery prerequisite.

## Durability before Committed

### Unix directory-entry rule

For each newly created directory entry (device dirs, segment/pack files, tip
objects, receipt objects, and intermediate dirs), sync its **containing parent**
directory. Apply on the live commit path and on reconciliation. Sync failures
remain typed and return `Uncertain`.

### Live write path (device canonical lock held throughout)

1. Durable `PENDING` receipt (temp + file sync + replace + Unix parent sync;
   create parent dirs with parent syncs as needed).
2. Ensure device directory exists; persist new device-dir entries via parent
   syncs as needed.
3. Append via writer with predetermined `event_id` (existing typed append/seal/tip).
4. After successful append return: Unix `sync_parent_directory` on the pack that
   holds the event (`.open` or `.seg` if sealed). Seal/tip typed failures from
   the writer map to `Uncertain` (not demoted).
5. Durable `COMMITTED` receipt with the same sync discipline.
6. Only then return `Committed`.

### Windows

File `sync_all` / write-through replace per existing platform helpers. Directory
sync remains the accepted no-op on Windows.

### Reconciliation → Committed

1. Complete verified scan finds `event_id` with matching fingerprint.
2. `sync_file` on that pack, then Unix parent sync. Sync failure → `Uncertain`.
3. Durable `COMMITTED` receipt. Failure → `Uncertain`.

## State machine

```text
capture_memory_note(req)
  load Brain + local DeviceId → Rejected if unavailable
  validate text + explicit classification → Rejected if invalid
  acquire device canonical lock (held for reconcile + append)
  fp = fingerprint_v1(...)

  no receipt → create PENDING → append_path
  receipt, fp mismatch → Rejected { capture_id_conflict }
  COMMITTED + fp match → durably_confirm → Committed | Uncertain
  PENDING + fp match → resume_pending

resume_pending / append_path
  verified scan(event_id):
    blocked → Uncertain
    found + fp match → durably_confirm → COMMITTED → Committed
    found + fp mismatch → Uncertain (no append)
    verified absent → append once with same event_id
                      then live durability ordering → Committed | Uncertain

resolve_capture(capture_id)    // never appends
  no receipt → Uncertain { receipt_missing }
  found → durably_confirm → Committed | Uncertain
  PENDING + verified absent → Rejected { not_committed }  // PENDING retained
  otherwise → Uncertain
```

Verified-absent `PENDING`: `resolve_capture` returns `not_committed` without
appending; matching `capture_memory_note` may append once with the same
`event_id`.

## Out of Slice 1

Desktop capture IPC/UI, timeline APIs, projection-as-saved wiring, extraction,
M6+, and GitHub Actions for this slice.
