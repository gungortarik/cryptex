# Mac compile check for Windows console suppression

Executed on 2026-10-03 at `83a770a`: format and compile PASS; strict Clippy
FAIL (unused `mac_product_runtime_root`). The lint gate remains open. See
`macos-background-command-check-evidence-20261003.md` for exact results.

Scope: only the newly added `apps/desktop/src-tauri/src/background_process.rs`
and its three daemon-command call sites. On Windows the adapter sets
`CREATE_NO_WINDOW`; on macOS it returns the ordinary command unchanged.
This does not require another physical login, tray, restart or Stop test.

Mac Codex should inspect the latest source and run:

```sh
cargo fmt --all -- --check
cargo check -p cryptex-desktop --locked
cargo clippy -p cryptex-desktop --all-targets --locked -- -D warnings
cargo test -p cryptex-desktop --bin cryptex-desktop --locked mac_runtime_path_tests
```

Record source commit and exact results in a dated evidence file and the
handoff; commit/push evidence only. Do not run GitHub Actions or rebuild a
release package for this compile check. If a command fails, retain its error
and leave the relevant check open. Windows physical console suppression is
separate and cannot be verified on the Mac.

Follow-up after `643ddcc`: the helper's condition now matches its callers
(`macos` plus release or test). The debug production binary no longer includes
the release-only helper; debug tests still include it. The two targeted runtime
tests check the product path and the missing-HOME error. Rerun the commands
above on the new fix commit and append results to the existing evidence.
No physical lifecycle suite or release package rebuild is requested.
