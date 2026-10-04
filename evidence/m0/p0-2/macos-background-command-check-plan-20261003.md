# Mac compile check for Windows console suppression

Scope: only the newly added `apps/desktop/src-tauri/src/background_process.rs`
and its three daemon-command call sites. On Windows the adapter sets
`CREATE_NO_WINDOW`; on macOS it returns the ordinary command unchanged.
This does not require another physical login, tray, restart or Stop test.

Mac Codex should inspect the latest source and run:

```sh
cargo fmt --all -- --check
cargo check -p cryptex-desktop --locked
cargo clippy -p cryptex-desktop --all-targets --locked -- -D warnings
```

Record source commit and exact results in a dated evidence file and the
handoff; commit/push evidence only. Do not run GitHub Actions or rebuild a
release package for this compile check. If a command fails, retain its error
and leave the relevant check open. Windows physical console suppression is
separate and cannot be verified on the Mac.
