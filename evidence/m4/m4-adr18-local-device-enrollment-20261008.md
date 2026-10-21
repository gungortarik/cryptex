# M4 ADR-0018 — Explicit local device enrollment

**Historical status at initial enrollment slice:** Windows local verification PASS; macOS parity pending at that time
**Code commits:** `8241fbf3ac5c00a8617bea3632245ed9b71b118d`, test-fixture correction `04a1328ef5ad668b207a968e7cf3f9988f910815`, read-only resolver `fac6e29f13343c116221ca3abca47b67eb0c051c`, explicit setup command `8775a768abd59db72cd68c8eb1692167319cf947`
**Branch:** `m4/knowledge-engine`

> **Historical status superseded:** macOS full library and enrollment IPC
> coverage passed at `946642a459531247091e858c301fc7ea17835ff0` as part of
> `cargo test --locked -p cryptexd --lib` (175/175). See
> `evidence/m4/m4-adr18-ipc-auth-timeout-hardening-20261008.md` for the exact
> host and command record.
>
> **Current status supersedes this snapshot:** the enrollment and authenticated
> IPC path passed the complete local Mac verification at exact behavior SHA
> `b6d652da07c175d086c625bfb498cf370dfc5fe2`. See
> `evidence/m4/m4-adr18-guarded-desktop-search-ipc-20261008.md`.
>
> The later reducer/IPC hardening candidate `c2cf07bf0662589a9932c201f5e9e749e6a1013a`
> has its own exact-SHA Mac PASS (macOS 27.0.1 arm64) recorded in
> `evidence/m4/m4-security-hardening-cancellable-reducer-20261008.md`.

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
- `cargo test --locked -p cryptexd --lib local_device_identity::` — **8/8 PASS**
- `cargo test --locked -p cryptexd --bin cryptexd` — **2/2 PASS**
- `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` — PASS
- `git diff --check` — PASS

Tests cover first enrollment and SecureStore binding, idempotent enrollment,
concurrent first enrollment, SecureStore failure/retry with stable DeviceId,
read-only load and existing-key resolution, exact local DeviceId/key matching,
Brain mismatch/corrupt metadata, and refusal of unbound existing canonical state.
The CLI tests require explicit runtime-root and SecureStore namespace arguments
and reject unknown namespace values before touching paths.

## Remaining work at original enrollment code SHA

macOS verification has not been run on this code SHA. Desktop-facing guarded
search IPC and retrieval-safety coverage through that IPC remain to be
implemented. This slice is not M4
completion and does not claim cross-platform parity.
