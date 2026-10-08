# M4 ADR-0018 — Guarded Desktop search IPC

**Status:** Windows local verification PASS; macOS parity pending
**Code SHA:** `ecbbaf7e41e3e851d6f7fbd8ce98c10af7208fa7`
**Branch:** `m4/knowledge-engine`

## Delivered

- Added a Desktop-only `content.search` capability and bounded, strictly
  deserialized query payload. Health IPC remains compatible with existing
  requests; health and search keep distinct authorization checks.
- Daemon loads the explicit local Brain/DeviceId/key binding and queries M3
  SQLite only through `search_events_with_retrieval_safety`.
- Missing/corrupt enrollment, unknown or unsupported canonical semantics,
  unresolved/stale materialization, and projection mismatch return a Brain-wide
  hold with **no hits**. Search never auto-enrolls or creates signing keys.
- Query size, filter size, time-bound size, result count, and response frame are
  bounded. Oversized results are rejected as a whole; no partial body is sent.
- Registered a typed Tauri command, `p0_4_guarded_search`.

## Explicit setup and renderer follow-up (`ecbbaf7`)

- Added a distinct `local_enrollment_required` hold. The Desktop offers explicit
  local setup only for a genuinely unenrolled installation; corrupt or
  unverifiable identity/key state remains a verification hold with no setup
  shortcut.
- Added an IPC regression proving an unenrolled Brain returns no search hits
  and does not create enrollment metadata during search.
- The Desktop renderer now calls the bounded guarded-search command and the
  separate explicit enrollment command. Search results are cleared when a new
  query is issued or retrieval is held; rendered note text remains escaped by
  React.
- Product v2 event writing remains disabled.

## Windows verification

- `cargo fmt --all -- --check` — PASS
- `cargo test --locked -p cryptexd --lib` — **173/173 PASS**
- Enrollment hold regression — **1/1 PASS**
- Read-only unenrolled identity regression — **1/1 PASS**
- `cargo test --locked -p cryptexd --bin cryptexd` — **2/2 PASS**
- `cargo clippy --locked -p cryptexd -p cryptex-desktop --all-targets -- -D warnings` — PASS
- `cargo check --locked -p cryptex-desktop` — PASS
- `pnpm --filter @cryptex/desktop build` — PASS (`tsc` + Vite)
- `git diff --check` — PASS

IPC tests cover strict query bounds, Desktop-only/exact capability
authorization, missing-Brain and unenrolled-Brain holds that return no content.
Existing retrieval-safety tests cover canonical/inventory, supported-v1,
unsupported-v2, pending, stale materialization, and projection mismatch outcomes.

## Remaining work

macOS verification has not been run on this code SHA. Physical visual review and
macOS parity remain open; backend hold coverage exists, but this record does not
claim every hold reason was manually exercised in the UI. No Actions were run.
This slice is not M4 completion and does not enable production v2 writing.
