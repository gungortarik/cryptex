# M8 checkpoint publication verification — 2026-10-29

The owner requested committing and pushing the accumulated work on
`codex/m8-device-trust`, with a visible account of implemented and remaining
work. This is an incomplete implementation checkpoint, not milestone acceptance.

## Fresh checks

- `cargo test --locked --workspace --exclude cryptex-desktop --lib --bins -- --test-threads=1`:
  failed at the daemon library: **515 passed, 3 failed, 4 ignored**. Failing
  tests: `owned_process::tests::http::owned_http_identity_mismatch_and_total_deadline`,
  `owned_http_ipv4_ipv6_and_owned_monitor_child`, and
  `owned_http_rejects_redirect_auth_version_duplicate_oversize_truncation_encoding`.
  Actual errors were `Endpoint(PeerNotOwned)` rather than expected owned-server
  responses. Cargo stopped before the remaining executables; this is not a
  workspace pass.
- `cargo test --locked -p cryptexd --lib owned_process::tests::http:: -- --test-threads=1`:
  **4 passed, 4 failed**. The same three cases plus
  `owned_http_shutdown_refuses_wrong_identity_before_post` failed with
  `Endpoint(PeerNotOwned)`. Root cause is unresolved; it is not attributed to
  Windows policy or a code regression without further evidence.
- `cargo test --locked -p cryptexd --bin cryptexd -- --test-threads=1`:
  **12 passed**. Unlike the earlier owner-operation evidence run, the binary
  test executable launched successfully this time.
- `cargo test --locked --workspace --exclude cryptex-desktop --exclude cryptexd --lib --bins -- --test-threads=1`:
  PASS for the remaining workspace libraries and binaries. This does not
  include integration test executables or Desktop Rust tests.
- `cargo clippy --locked --workspace --all-targets --release -- -D warnings`:
  PASS after removing two unnecessary borrows in a bootstrap test helper.
  No production behavior or test assertions changed for this lint fix.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.
- `pnpm --dir apps/desktop build`: PASS.
- `node --test apps/desktop/src/search-policy.test.mjs apps/desktop/src/capture-policy.test.mjs`:
  **18 passed**.

Earlier package-smoke and native fixture results remain historical evidence;
they were not rerun for this publication. No interactive Desktop flow, macOS
parity, transport/publication completion, security-review acceptance, or M8
completion is claimed. No GitHub Actions workflow was manually dispatched.

## Continuation

Investigate the owned-process HTTP failures first without weakening process
ownership checks. Then exercise the Windows native owner flow and remaining
cutoff/cancel/retry/recovery cases. Production transport/publication, durability,
macOS peer authentication/parity, independent review, and M8 acceptance remain open.
