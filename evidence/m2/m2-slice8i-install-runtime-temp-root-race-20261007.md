# M2 Slice 8i — install_runtime test temp_root exclusive allocator

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `e5568bd0a0e4ebd21caa8be33c19e6c2e106370d`
**Prior tip:** `1f9d7a4` / behavior `942569d`

## CI failure (macOS)

- Actions run: [37678360756](https://github.com/gungortarik/cryptex/actions/runs/37678360756) (cancelled after failure; **not** treated as final gate)
- **FAIL:** `install_runtime::tests::prepare_m1_runtime_foundation_binds_install_and_sequence_dir`
- Panic: `prepare: Core(UnsupportedSchema { kind: "install identity", found: 3, expected: 1 })` at `install_runtime.rs` (pre-fix tip)

## Root cause

Test helper `temp_root()` named fixtures `cryptexd-install-{nanos}` and called `remove_dir_all` before use. Under `--test-threads>1`, concurrent tests (notably `load_fails_closed_on_future_schema`, which writes `schema_version: 3`) could collide on the same nanosecond path: one test’s future-schema fixture was observed by `prepare_m1_runtime_foundation_*`.

Production install identity / prepare behavior was not at fault.

## Fix (test-only)

`services/cryptexd/src/install_runtime.rs` `tests::temp_root`:

- Name: `cryptexd-install-{pid}-{seq}` with process-local `AtomicU64`
- Exclusive `fs::create_dir`; on `AlreadyExists`, retry with a new sequence
- **Never** pre-delete a candidate path
- Production code and test assertions unchanged

## Local verification

### Windows (this host)

```
cargo test --locked -p cryptexd --lib -- --test-threads=16
→ run1..run5: ok. 126 passed each (finished ~1.5–1.8s)

cargo test --locked -p cryptexd
→ lib 126; lifecycle_harness 9; mode_b_supervisor 3; update_rollback 2 — all PASS

cargo test --locked -p cryptex-storage
→ ok. 35 passed

cargo fmt --all -- --check
→ PASS

cargo clippy --locked --workspace --all-targets -- -D warnings
→ PASS
```

### macOS (local host)

**Not run on a local Mac** — no reachable macOS checkout/host from this Windows implementer environment. Mac CI reproduction was the Actions failure above; the fix matches the exclusive-fixture pattern already used for pilot_runtime (slice 8e).

Final dual-OS Actions is reserved for the fixed tip after this push (owner-gated; not dispatched from this slice).

## Non-goals

- GitHub Actions dispatch
- Production install/prepare behavior changes
- Weakening schema fail-closed assertions
