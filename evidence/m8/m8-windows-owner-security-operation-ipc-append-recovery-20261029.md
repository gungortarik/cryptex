# M8 Windows owner-security confirmation, append, and recovery (2026-10-29)

## Scope

Implemented the Windows owner-operation slice after owner acceptance of ADR-0019 §2. This is source-level and synthetic-test evidence; it is not a packaged/native Windows UI pass, transport-to-publication completion, or M8 completion.

## Behavior

- Daemon derives the immutable restriction/self-retirement snapshot from a complete verified canonical trust scan. Renderer input selects only the requested operation and parameters; device/key/transport bindings and cutoff evidence are derived from canonical state.
- The exact snapshot is durably journaled before confirmation. Confirmation uses a single-use capability bound to the same IPC connection; cancel is terminal and does not append.
- On Windows, the daemon revalidates the live connected Desktop process identity before consuming the confirmation and appending. macOS remains fail-closed because this peer identity path is not implemented there.
- Confirmed owner operations append through the canonical writer while holding the canonical lock set. A post-append/pre-anchor interruption leaves an append-unknown journal state; startup re-reads the exact canonical history and anchors the stable event without duplicating it.
- Desktop presents the full immutable target/reason/cutoff or self-retirement boundary for owner confirmation. No direct canonical file access was added to the renderer.

## Synthetic regression evidence

- `ipc::tests::owner_security_ipc_binds_confirmation_and_appends_self_retirement_once` — confirmation is bound to the request connection/capability and appends once.
- Extended bootstrap approval IPC test — a synthetic peer admission followed by a device restriction is journaled and anchored using the reducer-derived trusted roster.
- `knowledge_materialization::tests::confirmed_self_retirement_append_recovers_after_durable_append` — an interruption after canonical append is recovered by exact-match reconciliation without a second append.
- `knowledge_materialization::tests::restriction_request_binds_approved_peer_and_exact_verified_cutoff` — restriction requires an approved target and exact verified cutoff.

## Verification on Windows source checkout

- `cargo test -p cryptexd --lib --bins -- --test-threads=1`: library **517 passed, 0 failed, 4 ignored**. The separate `cryptexd` binary test executable did not launch: Windows Application Control blocked it (OS error 4551); binary tests are **not claimed**.
- `cargo clippy -p cryptexd --release --lib --bins -- -D warnings`: PASS.
- `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --release --bin cryptex-desktop -- -D warnings`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `pnpm --dir apps/desktop build`: PASS (TypeScript + Vite).
- `pnpm --dir apps/desktop run tauri:package-smoke`: PASS (Windows release Desktop + daemon sidecar; no installer/bundle).
  - Desktop SHA-256: `EA86B015F747BAF9D63FC366923958D260F9AE38E26381902E6ADA90C0A62A32`.
  - daemon sidecar SHA-256: `B05039386DE7F234021FBBEC2EF3DD27A6BC8354EA5D38FCF80D864BA690929B`.
- `git diff --check`: PASS (Git reports the existing handoff file's CRLF-to-LF normalization warning).
## Remaining gates

- Build/package and run the Windows native Desktop owner flow with a fresh synthetic profile; this turn did not launch the app or claim an interactive pass.
- Exercise nonzero exact cutoff boundaries and denial/cancel/retry/error cases in the packaged flow.
- Complete production Syncthing transport-to-staging and safe canonical publication/replay, including durability and crash-boundary proof.
- Implement macOS authenticated peer identity and equivalent owner-operation path; macOS currently rejects this route.
- Independent security review and remaining M8 Definition-of-Done gates.

No Actions, commit, push, merge, or M8-complete claim.
