# Local MCP bridge

From the repository root, install the pinned SDK with `npm ci --prefix tools/local-mcp --ignore-scripts`.
An MCP client can launch `node` with the absolute path to `tools/local-mcp/stdio.mjs` as its only argument.
The bridge communicates with the client over stdio. Discovery is local. Tool calls use daemon IPC.

The tool takes an empty arguments object. A version-matched, correlated
`health.pong` from the configured local IPC endpoint yields
`daemon_responding`. Connection failure, timeout or an invalid reply yields
`daemon_unavailable` with the tool error flag. The result describes that
single IPC exchange. Provider readiness and device synchronization need their
own evidence.

`cryptex_search_public_notes` takes a single `phrase` (up to 1024 UTF-8 bytes)
and returns up to ten visible public notes as event IDs and 240-character
excerpts. Install the package in the MCP host's environment and launch its
`cryptex-mcp` executable with `--public-notes` for this workflow. Without
that argument the bridge advertises health alone.

1. Call `cryptex_request_public_read` with empty arguments.
2. In Cryptex Desktop open **Connection settings → Review AI access**. Compare
   the complete code with the code returned to the MCP client. The process
   number identifies the local bridge.
3. Select **Allow one search** or **Deny**. Escape also denies the request.
4. On approval, call `cryptex_search_public_notes` in the same MCP session.
   Another search requires another explicit decision.

The daemon accepts decisions only on authenticated Desktop IPC. The grant
belongs to the requesting connection, expires 120 seconds after the request
and is consumed by one search. Restarting the daemon drops pending decisions.
Review and search do not write to the profile.
The reader opens the existing index read-only and verifies
canonical visibility on each call. A recovery binding or failed verification
withholds the entire result. A verified empty search returns an empty list.

On Windows and macOS, exercise the isolated reader after installing the SDK:

```sh
cargo test --locked -p cryptexd --lib ipc::m12_health_tests::approved_bridge_reads_only_visible_public_notes -- --ignored
```

The SDK client launches the normal bridge entry to test discovery and denied
operations. A test-only entry supplies an isolated pipe or socket for health
calls. The native daemon dispatcher answers one probe inside a temporary
profile, then the same client probes the closed endpoint. Additional tests
exercise an absent listener, a mismatched reply, an oversized frame and a
silent peer. The content test uses two distinct bridge processes, four privacy
classes, a stale index after a signed forget, and a damaged canonical record.
The hosted installed-package test installs a tarball in a fresh prefix, drives
the installed SDK client and compiled Desktop review UI, and verifies both
decisions through the daemon's native Desktop IPC using synthetic notes.
Run that test on a disposable GitHub-hosted runner with the M12 workflow.
The check uses a native test host for the webview command and does not exercise
a particular third-party AI application's UI.
