# M8 Windows pre-publication interruption tests

**Date:** 2026-10-25  
**Branch:** `codex/m8-device-trust`

## Change

Added subprocess interruption tests at two durability boundaries in prepared
manifest creation:

1. After the prepared temporary file is synced, before no-replace placement.
2. After the prepared manifest is placed and its parent directory is synced.

Each child exits abruptly through the existing test-only interrupt helper. The
parent then calls startup recovery and checks that the incoming transport file
remains retryable and the uncommitted staging/journal state is removed. This is
process-crash coverage for pre-publication states; it is not power-loss testing
and does not cover canonical publication, which is not implemented.

## Verification

- `cargo check -p cryptexd --tests` — **PASS**.
- `cargo clippy -p cryptexd --all-targets -- -D warnings` — **PASS**.
- `cargo fmt --all -- --check` and `git diff --check` — **PASS**.
- `cargo test -p cryptexd --lib import_staging::tests` — **NOT RUN**: Windows
  Application Control blocked the rebuilt test executable with error **4551**.

The two new interruption tests must run in the Windows development VM before
their behavior is considered verified. Do not treat compile/lint as a test
PASS. No Actions ran.
