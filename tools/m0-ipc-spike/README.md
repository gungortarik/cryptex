# M0 P0-3 local IPC spike

This is an isolated candidate prototype. It does not wire IPC into `cryptexd`,
Desktop, CLI, MCP, or canonical memory, and does not select or accept ADR-0007.

## Candidate rationale

[`interprocess` 2.4.4](https://docs.rs/interprocess/2.4.4/interprocess/local_socket/index.html)
exposes one local-socket API that maps to Windows named pipes and Unix-domain
sockets, and it does not add framing bytes to the stream.
Its synchronous API fits the current `cryptexd` proof without introducing an
async runtime. The Windows API accepts a typed security descriptor. The
alternative [Tokio named-pipe API](https://docs.rs/tokio/latest/tokio/net/windows/named_pipe/struct.ServerOptions.html)
also covers both transports, but custom Windows security attributes require an
unsafe raw-pointer call. These are spike observations, not a production library
decision.

The Windows prototype reads the current process token SID and supplies an
explicit owner and protected DACL granting access to that SID. It does not rely
on the broader [default named-pipe ACL](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights),
which grants read access to Everyone and anonymous users. A second-account
denial proof is still needed before the user-only ACL acceptance criterion is
marked verified.

On Unix, the prototype makes a short per-run socket path inside a directory
created with mode `0700`, then verifies that an existing directory is owned by
the current UID and has no group/other permissions. This avoids relying on a
long Application Support path for the macOS socket. macOS mode/path behavior
still requires an actual Mac run.

## Windows run

```powershell
cargo test -p m0-ipc-spike --locked
cargo build -p m0-ipc-spike --locked
powershell.exe -NoProfile -NonInteractive -File tools/m0-ipc-spike/scripts/test-windows-ipc-spike.ps1
```

The cross-process script checks Desktop/CLI/MCP role handshakes, rejects an
incompatible protocol and unapproved capability, rejects malformed and
oversized frames, and reconnects a client after restarting the server. Its
logs and JSON summary are kept under `%LOCALAPPDATA%\Cryptex-Dev\p0-3`.

### Windows cross-account ACL test in CI

`scripts/test-windows-ipc-acl-ci.ps1` runs only on disposable GitHub-hosted
Windows runners. It creates a temporary non-admin account, starts a pipe client
as that account, verifies its actual SID differs from the owner and checks for
`ERROR_ACCESS_DENIED`. Owner handshakes must succeed before and after that
attempt. Owned processes and the temporary account are removed before PASS is
reported; cleanup errors fail the test. Credentials are generated in memory
and never printed or placed in command arguments.

The separate CI job runs this test in parallel with the main readiness jobs.
The script refuses local workstation and self-hosted runner execution before
creating any account. GitHub documents the disposable runner and Windows
administrator environment in its [hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
This test's execution result is pending; preparation alone does not verify ACL
denial.

## macOS run

Run this on the Mac host from the repository root:

```sh
cargo test -p m0-ipc-spike --locked
cargo build -p m0-ipc-spike --locked
python3 tools/m0-ipc-spike/scripts/test_unix_ipc_spike.py
```

The process-level script checks socket-directory ownership and mode, path
length, Desktop/CLI/MCP role handshakes, invalid protocol/capability/role,
malformed and oversized frames, and client reconnection after the server
process is restarted. It stores JSON and server startup evidence under
`~/Library/Application Support/Cryptex-Dev/p0-3`. This is candidate-spike
evidence only; it does not accept ADR-0007 or prove application integration.

## Protocol slice

The proof accepts only `health.ping` with the `status.read` capability for the
three named roles. Requests are one UTF-8 JSON object per LF-terminated frame,
with a 64 KiB maximum. The client verifies the response protocol version,
request ID, and accepted operation before trusting it. This narrow read-only
protocol does not grant memory write, purge, recovery, or arbitrary tool capabilities.
