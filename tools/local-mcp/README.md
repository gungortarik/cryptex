# Local MCP bridge

From the repository root, install the pinned SDK with `npm ci --prefix tools/local-mcp --ignore-scripts`.
An MCP client can launch `node` with the absolute path to `tools/local-mcp/stdio.mjs` as its only argument.
The bridge communicates with the client over stdio. It sends a local IPC request only when the client calls `cryptex_health`.

The tool takes an empty arguments object. A version-matched, correlated
`health.pong` from the configured local IPC endpoint yields
`daemon_responding`. Connection failure, timeout or an invalid reply yields
`daemon_unavailable` with the tool error flag. The result describes that
single IPC exchange. Provider readiness and device synchronization need their
own evidence. This slice exposes no memory or write tools.

On Windows and macOS, run the native CI check after installing the SDK:

```sh
cargo test --locked -p cryptexd --lib ipc::m12_health_tests::native_stdio_bridge -- --exact --ignored --nocapture
```

The SDK client launches the normal bridge entry to test discovery and denied
operations. A test-only entry supplies an isolated pipe or socket for health
calls. The native daemon dispatcher answers one probe inside a temporary
profile, then the same client probes the closed endpoint. Additional tests
exercise an absent listener, a mismatched reply, an oversized frame and a
silent peer. Shipping client integrations and classified retrieval remain open.
