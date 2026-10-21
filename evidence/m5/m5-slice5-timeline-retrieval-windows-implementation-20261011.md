# M5 Slice 5 -- guarded timeline retrieval (Windows implementation checkpoint)

**Status:** Windows implementation checkpoint and focused automated checks PASS; macOS/native verification remains open.
**Branch / base:** `codex/m5-capture-timeline` at the recorded continuation after Slice 4 documentation tip `d9b3ca5`.
**Scope:** M5 kickoff-plan item 5, guarded timeline/history/source display over verified local canonical-derived state.

## Contract used by this implementation

- Timeline returns only existing projected v1 `memory.note` rows. It adds no
  event type, canonical field, schema, database, or writer path.
- Each page is newest-first by exact UTC instant, then `event_id` descending for
  deterministic ties. Dates in the UI include local date and time.
- Page size defaults to 20 and is bounded to 1..=50. A keyset cursor contains
  the last row's `created_at` and `event_id`; it is validated and treated only
  as a continuation hint, never as authorization. Each page re-runs the same
  Brain-wide retrieval-safety and projection-consistency gate as search.
- Filters are exact classification, event type, device, and inclusive RFC3339
  instant bounds. The Desktop cannot supply the Brain ID.
- Timeline bodies are bounded UTF-8-safe snippets (at most 384 bytes plus an
  ellipsis). Source details expose existing verified provenance only: event,
  Brain, device/sequence, event type, classification, content hash and signing
  key ID. React renders these values as text.
- A failed/stale safety gate returns no hits, snippet, or source fields. The
  existing daemon catch-up worker is requested on missing/stale projection.
- Opening a timeline row requests full note detail by exact event ID through a
  separate guarded IPC operation. Its response has a separate bounded frame
  allowance sized for the v1 note maximum and worst-case JSON string escaping.

## Implementation

- Added bounded newest-first keyset paging to the derived projection query,
  ordering by parsed UTC instant and deterministic event ID.
- Added guarded `content.timeline` daemon IPC for the authenticated Desktop
  peer, reusing the content-search capability, worker limit, deadline, and
  Brain-wide retrieval gate.
- Added the Tauri bridge and a minimal Desktop Timeline rail view with source
  metadata, full-note detail, and “Load older notes” paging. Search remains a
  separate view.

## Windows checks run

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check --locked -p cryptexd --lib` | PASS |
| `cargo test --locked -p cryptex-projection` | **50/50 PASS** |
| `cargo test --locked -p cryptexd --lib ipc::tests::` | **39/39 PASS** |
| `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` | PASS |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --bin cryptex-desktop -- -D warnings` | PASS |
| `npm run build` | PASS |

The added tests cover exact-instant newest-first ordering, deterministic ties,
keyset page continuation without duplicates, invalid cursor/bounds, authenticated
Desktop IPC, source metadata, snippet response, exact event detail, and
stale-materialization fail-closed behavior. Mac verification, packaged
interactive review, and Slice 5 closeout are not claimed.

No Actions, merge, M5-complete claim, or production v2 activation.
