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
excerpts. The daemon binds the read grant to one live bridge child in the
profile session and checks the connecting process on every request. Only
the native disposable harness provisions that grant today. Ordinary profiles
deny the read. The reader opens the existing index read-only and verifies
canonical visibility on each call. A recovery binding or failed verification
withholds the entire result. A verified empty search returns an empty list.

On Windows and macOS, run the native CI check after installing the SDK:

```sh
cargo test --locked -p cryptexd --lib ipc::m12_health_tests:: -- --ignored --nocapture
```

The SDK client launches the normal bridge entry to test discovery and denied
operations. A test-only entry supplies an isolated pipe or socket for health
calls. The native daemon dispatcher answers one probe inside a temporary
profile, then the same client probes the closed endpoint. Additional tests
exercise an absent listener, a mismatched reply, an oversized frame and a
silent peer. The content test uses two distinct bridge processes, four privacy
classes, a stale index after a signed forget, and a damaged canonical record.
Owner provisioning and shipping client integrations remain open.
