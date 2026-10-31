# M4 Windows IPC pipe round trip

**Branch:** `m4/knowledge-engine`
**Failing baseline:** `e55e7651390d89ceca8711291bbb1b0356631d59`
**Fix commit:** `6855c57766c018339a781fabc71a11a810905398`
**Date:** 2026-10-09

## Root cause

The daemon process and `lifecycle.status` can show `instance=held` and `health=active` without the Desktop having completed a named-pipe exchange. `lifecycle.status` is written by cryptexd after the listener is created. The Desktop health and search commands use a separate pipe client.

On Windows, `Stream::set_nonblocking(true)` enables `PIPE_NOWAIT`. interprocess then performs overlapped `ReadFileEx` and maps the immediate "no data yet" result to a zero-length read. The framing layer treats that zero-length read as end-of-stream. The server accepts a connection and returns no response; the Desktop client fails before it can show a retrieval hold. The renderer therefore stays on Attention, and guarded search falls into the generic IPC error. No memory text is included in that failure.

Unix nonblocking reads return `WouldBlock` and keep waiting until the existing deadline, so the same code did not fail there. The previous Windows IPC tests did not perform a framed nonblocking exchange, so they stayed green.

## Fix

Windows framing no longer enables `PIPE_NOWAIT`. Reads and writes use overlapped `ReadFile`/`WriteFile` with a waitable event and `CancelIoEx` at the existing request and response deadlines. A stalled peer still returns `TimedOut` instead of pinning the worker. Retrieval-safety holds, v1 validation, and the disabled production v2 writer are unchanged.

## Windows checks

- `cargo test -p cryptexd --lib`: 185 passed, 0 failed. This includes `desktop_health_frame_round_trips_on_a_named_pipe` and `stalled_pipe_read_returns_at_the_deadline`.
- `cargo clippy -p cryptexd --all-targets -- -D warnings`: passed.
- `cargo fmt -p cryptexd`: applied; `git diff --check` was clean for the fix.

## Native UI

Debug embedded Desktop built with `pnpm exec tauri build --debug --no-bundle` from `apps/desktop`. The window was the real Cryptex webview, not a screenshot-only launch.

Runtime used for the check: `%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\mode-b-desktop`. Before the check it held supervisor and lifecycle control files only. No canonical pack directory was present. No enrollment or memory write was performed.

With the fixed cryptexd started by that Desktop process:

- After reload, the connection status read **Active**.
- A direct health command returned `accepted=true`, operation `health.pong`, and the same daemon pid as `lifecycle.status`.
- `lifecycle.status` showed `instance=held`, `health=active`, `supervision=unresolved`, `registration=not_configured`.
- Submitting a local search did **not** show "Cryptex could not complete a guarded search."
- The window showed the fail-closed hold "Cryptex could not verify the complete local event history. Search is paused to protect your memory." The IPC payload was `hold=canonical_verification_unavailable` with no hits. That hold is expected for this runtime, which has no verified canonical history. The connection label then showed Attention because a retrieval hold uses that state.

The session was closed with the Desktop clean-quit command. This was not a release installer or NSIS package pass.

## Not verified

- macOS local tests, Clippy, and native visual review. No Mac host was available in this session. Those gates stay pending.
- Owner Definition of Done review and human acceptance.
- The single final dual-OS GitHub Actions readiness workflow passed after this record's first draft. Run https://github.com/gungortarik/cryptex/actions/runs/37987708059 at exact SHA `7aaa08a0061c3208c0e45ab7d8f6cd032bf52315`: macOS readiness success, Windows readiness success, Windows IPC ACL success. The unsigned Windows package job was skipped by workflow design. No second workflow was started.
- Production v2 writing remains disabled. M5, M8, M10, and M14 were not implemented.

M4 is not complete.
