# macOS background-command compile and lint evidence

Date: 2026-10-03. Overall result: **NOT YET VERIFIED**; format and compile
checks PASS, strict Clippy FAIL remains open.

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
