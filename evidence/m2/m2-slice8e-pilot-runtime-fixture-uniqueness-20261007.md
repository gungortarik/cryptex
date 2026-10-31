# M2 Slice 8e — Pilot runtime test fixture uniqueness

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `1069cf1bf90d78dae1f18c4c93c2c512982180a9`

## Prior CI (near-final)

- GitHub Actions run: [37656380940](https://github.com/gungortarik/cryptex/actions/runs/37656380940)
- **PASS:** Windows readiness (complete); Windows IPC ACL
- **FAIL:** macOS readiness at Desktop Rust foundation — `pilot_runtime::tests::unsafe_root_entry_is_rejected`
  - Panic at `apps/desktop/src-tauri/src/pilot_runtime.rs:121` (`create_dir` unwrap)
  - `Os { code: 17, kind: AlreadyExists, message: "File exists" }`
- Unsigned Windows package job skipped by design

## Diagnosis

- Test helper `fixture()` named temp roots as `cryptex-pilot-runtime-{pid}-{nanos}` and called non-exclusive `fs::create_dir`.
- Concurrent Desktop unit tests (same process) can share one nanosecond stamp and collide; leftover same-named directories from prior runs can also collide.
- Product pilot-root validation and assertions were not at fault.

## Fix (test fixture only)

- Add a process-local `AtomicU64` sequence into the fixture directory name.
- Create the directory with exclusive `fs::create_dir`; on `AlreadyExists`, retry with a new sequence/stamp rather than unwrapping.
- Write the ownership marker only after exclusive create succeeds.
- No assertion weakening; no production CIM/pilot validation changes.

## Local verification (Windows)

- `cargo test --locked -p cryptex-desktop pilot_runtime -- --test-threads=8` — PASS (6/6)
- Same focused filter repeated 5× with `--test-threads=16` — PASS each run
- `cargo test --locked -p cryptex-desktop -- --test-threads=16` — PASS (23/23)
- `cargo fmt --all -- --check` — PASS (exit 0)

## Non-goals

- Dispatching or re-running GitHub Actions from this slice
- Changing production pilot-runtime acceptance rules

## Next

- Owner review of this tip, then **one** dual-OS `ci.yml` readiness run only if still required as the final green gate after run 37656380940 results are considered.
