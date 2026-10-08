# M4 ADR-0018 — Explicit local device enrollment

**Status:** Windows local verification PASS; macOS parity pending  
**Code commit:** `8241fbf3ac5c00a8617bea3632245ed9b71b118d`  
**Branch:** `m4/knowledge-engine`

## Scope

Added an explicit daemon library operation for enrolling the current local
installation as one DeviceId bound to the existing Brain and ADR-0016 signing
key. Enrollment is not invoked by lifecycle startup or ordinary reads. It does
not trust another device, create a peer roster, or enable production v2 writes.

First enrollment refuses to infer ownership when canonical event directories or
device signing metadata already exist without a local binding. Identity metadata
uses exclusive durable placement. The local DeviceId is persisted before key
initialization so a SecureStore failure can be retried with the same ID. Reads
are read-only and fail when explicit enrollment has not occurred.

## Verification on Windows

- `cargo fmt --all -- --check` — PASS
- `cargo test --locked -p cryptexd --lib local_device_identity::` — **7/7 PASS**
- `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` — PASS
- `git diff --check` — PASS

Tests cover first enrollment and SecureStore binding, idempotent enrollment,
concurrent first enrollment, SecureStore failure/retry with stable DeviceId,
read-only load, Brain mismatch/corrupt metadata, and refusal of unbound existing
canonical state.

## Remaining work

macOS verification has not been run on this code SHA. The explicit setup command,
read-only key resolver, Desktop-facing guarded search IPC, and retrieval-safety
coverage through that IPC remain to be implemented. This slice is not M4
completion and does not claim cross-platform parity.
