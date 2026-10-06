# macOS background-command compile and lint evidence

Date: 2026-10-03. Latest scoped result: **PASS** at `808bb1c`; format,
compile, strict Clippy and the two targeted runtime tests pass. The Mac lint
gate is closed by this revalidation. The initial failure is retained below.

## Initial run at `83a770a` — historical NOT YET VERIFIED

## Source and scope

- Branch: `m0/p0-2-cursor`.
- Source: `83a770abcd513b55f1a37661acc66f85a37f4f1a` (clean checkout).
- Host: macOS `27.0.1`, `arm64`.
- Compiler: `rustc 1.98.1 (48a229cea 2026-09-01)`.
- Plan: `macos-background-command-check-plan-20261003.md`.

Inspected `apps/desktop/src-tauri/src/background_process.rs` and the three
daemon-command call sites in `main.rs`: status query, launch preflight and
held daemon launch. Windows creation flags are under `cfg(target_os =
"windows")`; the macOS path returns the ordinary command unchanged.

## Executed commands

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | PASS; no output |
| `cargo check -p cryptex-desktop --locked` | 0 | PASS with one dead-code warning |
| `cargo clippy -p cryptex-desktop --all-targets --locked -- -D warnings` | 101 | FAIL; unused `mac_product_runtime_root` |

Check and Clippy were also repeated individually with the same results.
The check warning is at `apps/desktop/src-tauri/src/main.rs:64:4`:
`function mac_product_runtime_root is never used`.

The exact repeated Clippy diagnostic was:

```text
   Compiling cryptex-desktop v0.0.0 (/Users/tarikgungor/github_repos/cryptex/apps/desktop/src-tauri)
error: function `mac_product_runtime_root` is never used
  --> apps/desktop/src-tauri/src/main.rs:64:4
   |
64 | fn mac_product_runtime_root(home: Option<&std::ffi::OsStr>) -> Result<PathBuf, String> {
   |    ^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `-D dead-code` implied by `-D warnings`
   = help: to override `-D warnings` add `#[expect(dead_code)]` or `#[allow(dead_code)]`

error: could not compile `cryptex-desktop` (bin "cryptex-desktop") due to 1 previous error
```

Source inspection explains the target mismatch: the helper is enabled for
all macOS targets, while its production caller is release-only. Its other
use is in a test module. The ordinary debug binary therefore reports dead
code. This failure is outside the new command adapter; no warning allowance
or application-code change was made. Strict lint must be rerun after a
separately authorized source fix; this run does not close that gate.

## Boundaries

Evidence and handoff only were changed. No GitHub Actions, release package
rebuild, runtime configuration change, or repeated physical login/tray/
restart/Stop test was performed. Windows physical console suppression cannot
be verified by this Mac check and remains a separate gate. ADR-0012 and
overall P0-2 completion are not accepted by these results.

## Revalidation at `808bb1c` — PASS

- Source: `808bb1c039ab3735e3a6a777fafa6eae49790b10`.
- Branch: `m0/p0-2-cursor`; clean checkout before and after commands.
- Host: macOS `27.0.1`, `arm64`.
- Compiler: `rustc 1.98.1 (48a229cea 2026-09-01)`.

Inspected the fix: `mac_product_runtime_root` is now gated by
`all(target_os = "macos", any(not(debug_assertions), test))`, matching
release and test callers without a dead-code allowance. The background
command adapter and all three daemon-command call sites remain intact;
Windows flags remain Windows-only and macOS returns the ordinary command.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | PASS; no output |
| `cargo check -p cryptex-desktop --locked` | 0 | PASS; dev profile finished in 1.51s, no warnings |
| `cargo clippy -p cryptex-desktop --all-targets --locked -- -D warnings` | 0 | PASS; dev profile finished in 1.05s, no warnings |
| `cargo test -p cryptex-desktop --bin cryptex-desktop --locked mac_runtime_path_tests` | 0 | PASS; 2 passed, 0 failed, 2 filtered out |

Exact test result:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.87s
     Running unittests src/main.rs (target/debug/deps/cryptex_desktop-387698ebdfa53a1c)

running 2 tests
test mac_runtime_path_tests::missing_home_is_reported_for_product_runtime ... ok
test mac_runtime_path_tests::release_runtime_root_uses_hidden_application_support ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s
```

This supersedes the initial strict-lint failure for the tested source and
closes only the Mac compile/lint gate. No application code was changed in
this revalidation. No physical login/tray/restart/Stop test, release package
rebuild, runtime configuration change or GitHub Actions run was performed.
Windows physical console suppression remains separate; ADR-0012 acceptance
and overall P0-2 completion are not implied.
