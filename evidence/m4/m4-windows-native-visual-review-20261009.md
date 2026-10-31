# M4 Windows native visual review — PASS

**Branch:** `m4/knowledge-engine`
**Source HEAD:** `79e03d6c4cad8f0171b8b7d3e4bcbfada20fe5d4`
**Code equivalence:** `79e03d6` adds documentation only after CI-tested tip `7aaa08a`; no product-code delta.
**Host:** Windows 11 x64 in VMware; exact Windows build was not captured.
**Runtime:** isolated empty M4 pilot under `%LOCALAPPDATA%\Cryptex-Dev\m4`; no personal or Cortex memory used.

## Build and launch

The isolated package copied all 574 source files successfully. The host supplied
Node.js 24.19.0 and Rust 1.98.1; the runner installed pinned pnpm 11.28.0 in
LocalAppData. The `cryptexd` release sidecar and frontend TypeScript/Vite build
passed. The first Tauri attempt could not replace an existing derived
`cryptexd.exe` in the Cargo release target (`tauri-build` 2.7.0,
`fs::remove_file`, Windows error 5 / Access is denied). After the stale target
sidecar was removed, the native Desktop build and isolated pilot launch
succeeded. No OS security setting was changed.

## Visible behavior

- The native window reached **Active**.
- Searching synthetic text `test` displayed the expected fail-closed message:
  “Cryptex could not verify the complete local event history. Search is paused
  to protect your memory.” No results were displayed. This is the expected
  RetrievalSafetyState hold for the empty pilot, not the earlier generic IPC
  error.
- The owner reports closing through the system-tray **Quit Cryptex** action.
  A post-exit process snapshot was not captured, so shutdown is owner-reported.

Screenshots and the runner log were supplied in the task conversation; binary
screenshots are not copied into the repository. No automated test suite was run
as part of this native UI check.

## Gate status

**Windows native build and guarded-search visual check: PASS.** This does not
complete M4. The macOS visual review and final M4 DoD/security/owner review
remain open. The final dual-OS Actions run `37987708059` is on code-equivalent
tip `7aaa08a`; no Actions rerun is needed unless product code changes.
Production v2 event writing remains disabled.
