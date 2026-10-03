# P0-3 Windows local IPC candidate spike

Date: 2026-10-02 EDT (run time 2026-10-03 UTC). Candidate: `interprocess`
2.4.4. This is a comparison-harness result, not ADR-0007 acceptance or
production Desktop/CLI/MCP integration.

## Windows physical result

The repository-controlled Windows script ran separate server and client
processes at:

```text
%LOCALAPPDATA%\Cryptex-Dev\p0-3\ipc-windows-20261003T001835066Z
endpoint=\\.\pipe\cryptex-m0-p0-3-win-001835074
server_pid_before_restart=20132
server_pid_after_restart=23612
result=PASS_WINDOWS_SPIKE_ONLY_MACOS_NOT_TESTED
```

Verified in that run:

- `desktop`, `cli`, and `mcp` role handshakes with `status.read`
- incompatible protocol version rejected
- unknown client role rejected
- unapproved `memory.write` capability rejected
- malformed JSON request rejected
- 65,537-byte request rejected against the 64 KiB frame limit
- client launched while the server was down connected to the restarted server
  and received the restarted process PID
- Windows server was configured with an explicit protected DACL granting the
  current token SID; same-user connection succeeded

The script preserved its JSON summary and process output outside the repository.
The run did not attempt a connection from a second Windows account, so denial
for a different account remains unverified. These test clients exercised the
three role labels through the spike binary; the actual Cryptex Desktop, CLI, and
MCP clients are not connected to this transport.

## Build and static checks

- `cargo fmt --all -- --check`: passed
- `cargo test -p m0-ipc-spike --locked`: 5 passed
- `cargo build -p m0-ipc-spike --locked`: passed on Windows
- PowerShell test script syntax parse: passed
- `cargo check -p m0-ipc-spike --target aarch64-apple-darwin --locked`: passed

The Apple-target check confirms source compilation only. No macOS socket,
permission, path-length, or reconnect runtime proof was performed on this host.

## Scope

`m0-ipc-spike` is a separate tool. It does not alter `cryptexd`, Desktop, the
canonical writer boundary, or runtime supervision. Its protocol accepts only
`health.ping` with `status.read`; it exposes no memory mutation, purge, recovery,
or arbitrary tool capability. `interprocess` remains a P0-3 candidate until
cross-platform evidence and the security gaps above are reviewed.
