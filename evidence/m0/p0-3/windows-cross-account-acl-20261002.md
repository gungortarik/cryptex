# Windows IPC cross-account ACL proof

Date: 2026-10-02 EDT (2026-10-03 01:17 UTC).
Source: `7668d11`. Environment: disposable GitHub-hosted Windows runner.
[Successful focused CI run](https://github.com/gungortarik/cryptex/actions/runs/37085465368),
job `111094715959`.

## Method and observed result

The existing Rust IPC candidate runs as a separate server process owned by
the runner account. The test creates a temporary non-admin local Windows
account, authenticates it using `LogonUserW` with interactive logon type, and
uses `WindowsIdentity.Impersonate` on the synchronous client thread. The helper
verifies the effective thread SID before attempting the named-pipe connection.
This is an actual authenticated distinct-account token, not an ACL-string check.

| Observation | Result |
|---|---|
| Owner SID | `S-1-5-21-1643835476-1616584234-1346609752-500` |
| Authenticated client SID | `S-1-5-21-1643835476-1616584234-1346609752-1003` |
| Different-account connection | `System.UnauthorizedAccessException` |
| HRESULT | `-2147024891` (`0x80070005`, access denied) |
| Owner handshakes before and after | PASS; same server PID required |
| Owner identity restored after impersonation | PASS; explicitly checked |
| Temporary account removed | true |
| Cleanup errors | none |
| Final result | `PASS_WINDOWS_CROSS_ACCOUNT_ACL_SPIKE_ONLY` |

Endpoint: `\\.\pipe\cryptex-m0-p0-3-acl-fa6cb7e2bdf8`.
Runner runtime root:
`C:\Users\runneradmin\AppData\Local\Cryptex-Dev\p0-3\ipc-acl-ci-fa6cb7e2bdf8`.
The structured summary is retained in the CI job log; the runner filesystem is
ephemeral. Runtime files were outside the checkout.

The script refuses local workstation and self-hosted execution before account
creation. Its refusal was checked on the physical Windows host. The C# helper
compiled under Windows PowerShell 5.1; PowerShell syntax parsing and Git diff
checks passed. Credentials are never logged or passed in process arguments;
the native authentication password buffer is zeroed immediately after use.

## Earlier harness failures

- Run `37084572187`: credentialed process launch failed with Win32 error 87.
- Run `37084902895`: the child recorded the expected distinct SID and access
  denial, but process exit-status validation failed. Overall result was FAIL.
- Run `37085233376`: credentialed child process timed out. Overall result was FAIL.

These runs did not satisfy the acceptance test. The final helper eliminates
credentialed PowerShell child startup while retaining real OS authentication,
effective SID verification, the actual kernel access check, owner connectivity,
and cleanup requirements. It does not accept a timeout or arbitrary exception
as evidence of access denial.

## Scope

The Windows candidate's current-user DACL denial criterion is verified on a
Windows CI VM. Physical Windows transport/reconnect evidence and physical Mac
socket ownership/mode evidence remain in `windows-ipc-spike-20261002.md`.
This does not prove a second interactive login on Tarik's workstation, production
Desktop/CLI/MCP integration, or completion/acceptance of ADR-0007 or M0.
