# M4 ADR-0018 — Guarded Desktop search IPC

**Status:** Windows local verification PASS; macOS parity pending
**Code SHA:** `7ee4de5fc8abab85b5d74fe9de5da0e2ef80f34a`
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
- Registered a typed Tauri command, `p0_4_guarded_search`. The renderer does not
  yet call it; UI wiring and visual review remain open.

## Windows verification

- `cargo fmt --all -- --check` — PASS
- `cargo test --locked -p cryptexd --lib ipc::` — **3/3 PASS**
- `cargo test --locked -p cryptexd --lib` — **171/171 PASS**
- `cargo test --locked -p cryptexd --bin cryptexd` — **2/2 PASS**
- `cargo clippy --locked -p cryptexd -p cryptex-desktop --all-targets -- -D warnings` — PASS
- `cargo check --locked -p cryptex-desktop` — PASS
- `git diff --check` — PASS

IPC tests cover strict query bounds, Desktop-only/exact capability authorization,
and a missing-Brain hold that returns no content. Existing retrieval-safety
tests cover canonical/inventory, supported-v1, unsupported-v2, pending, stale
materialization, and projection mismatch outcomes.

## Remaining work

macOS verification has not been run on this code SHA. End-to-end Desktop command
transport, renderer UI integration, visual checks, and explicit no-content
assertions for each hold cause through the IPC response remain required. No
Actions were run. This slice is not M4 completion and does not enable production
v2 writing.
