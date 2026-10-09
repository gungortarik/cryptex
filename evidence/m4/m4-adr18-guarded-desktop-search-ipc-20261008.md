# M4 ADR-0018 — Guarded Desktop search IPC

**Status:** bounded full-history search and non-Desktop health-only startup are Windows-verified at `b6d652d`; exact-tip macOS parity, visual review and final M4 gates pending
**Code SHA:** `652b4def1b71e9ba47323d3e51955cd3d777091a`
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

## Search UI hardening (`652b4de`)

- Whitespace-only searches are rejected in the renderer; the UI no longer
  converts an empty phrase into an unfiltered list request.
- A request-generation gate prevents a response for an older query from
  replacing the screen after the user edits the query. Prior plaintext is
  cleared immediately, and stale responses are ignored.
- `pnpm --filter @cryptex/desktop test:search-policy` — **2/2 PASS**;
  frontend `tsc` + Vite build — PASS on Windows.

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

## macOS verification (`de1616c2bb5cca2acee2116768070ddefccf8c81`)

Host: macOS 27.0.1 (26A434), arm64. Working tree clean. This tip contains no
behavior changes after code SHA `ecbbaf7`.

- `cargo fmt --all -- --check` — PASS
- `cargo test --locked -p cryptexd --lib` — **168/168 PASS**
- `cargo test --locked -p cryptexd --bin cryptexd` — **2/2 PASS**
- Strict Clippy for `cryptexd` and `cryptex-desktop` — PASS
- `cargo check --locked -p cryptex-desktop` — PASS
- `pnpm --filter @cryptex/desktop build` — PASS (`tsc` + Vite)
- `git diff --check` — PASS

The lib test count differs from Windows (168 vs 173) due to platform-specific
tests; both full commands exited 0. No app launch, visual review, lockfile
change, or Actions run occurred. The UI-only hardening at `652b4de` postdates
this Mac verification; the two frontend checks below remain pending on macOS.

## Later bounded full-history search follow-up (`3792485`)

The original implementation performed authenticated Desktop requests on the
listener path. The follow-up moves authenticated Desktop connections to
bounded workers and gives expensive full-history search a separate two-worker
limit. Desktop connections are capped at eight; non-Desktop health traffic
retains its independent four-worker pool. This prevents slow searches from
occupying the listener or health capacity.

One 15-second deadline covers canonical inventory/device lock waits, frame
scanning, signature verification, deterministic reduction, materialization
comparison, and SQLite VM execution. Cancellation returns no partial hits and
maps to a Brain-wide RetrievalSafetyState hold. Expired SQLite progress hooks
are removed before the connection is reused. A persisted late cross-device
dependency test proves the safe watermark remains before the unresolved event,
then incremental replay converges to a fresh full rebuild after the dependency
arrives.

Windows at `37924852ad2207f0ab618c6737e5e08f0a6907c8`:

- `cargo fmt --all -- --check` — PASS
- `cargo test --locked -p cryptex-events` — **61/61 PASS**
- `cargo test --locked -p cryptex-storage` — **41/41 PASS**
- `cargo test --locked -p cryptex-projection` — **48/48 PASS**
- `cargo test --locked -p cryptex-knowledge` — **31/31 PASS**
- `cargo test --locked -p cryptexd --lib` — **182/182 PASS**
- `cargo test --locked -p cryptexd --bin cryptexd` — **2/2 PASS**
- `cargo check --locked -p cryptex-desktop` — PASS
- Strict Clippy for events, storage, projection, knowledge, and cryptexd
  `--all-targets -- -D warnings` — PASS
- `git diff --check` — PASS

Owner-reported macOS checks at `946642a459531247091e858c301fc7ea17835ff0`
verify the earlier peer-auth/IPC implementation, not this new worker/deadline
follow-up. An early full-package Windows test then exposed that requiring a
Desktop parent prevented CLI/lifecycle daemon starts. The failure diagnostic
showed `daemon parent executable is not Cryptex Desktop`. The production
startup path now starts retrieval IPC only when the packaged Desktop parent is
authenticated; otherwise it starts health-only IPC, keeping sensitive search
unavailable. This preserves CLI/test lifecycle behavior without weakening the
Desktop peer check.

Windows at `b6d652da07c175d086c625bfb498cf370dfc5fe2`:

- `cargo fmt --all -- --check` — PASS
- `cargo test --locked -p cryptexd` — **198/198 PASS** (lib 182, bin 2,
  lifecycle 9, Mode B supervisor 3, update rollback 2)
- Strict Clippy for events, storage, projection, knowledge, and cryptexd
  `--all-targets -- -D warnings` — PASS
- `cargo check --locked -p cryptex-desktop` — PASS
- `git diff --check` — PASS

Exact-tip Mac parity for `b6d652d` remains required. The report at
`946642a` is earlier and does not verify this follow-up. No Actions were run.

## Remaining work

macOS frontend checks (`test:search-policy` and Desktop build) passed at
`9eea611` as recorded in `evidence/m4/m4-search-ui-macos-recheck-20261008.md`.
The follow-up's Rust behavior still needs exact-tip Mac checks. Physical visual
review and final security/DoD review remain open; backend hold coverage exists,
but this record does not claim every hold reason was manually exercised in the
UI. No Actions were run. This slice is not M4 completion and does not enable
production v2 writing.
