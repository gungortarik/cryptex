# M8 owned established connection and REST boundary

Date: 2026-10-21. Windows-first M8 continuation under Master Spec, accepted
ADR-0004 addendum and the M8 contract. No production startup/IPC activation,
canonical writes or peer-trust protocol/schema change is authorized here.

## Why a listening-port check is insufficient

The client connects to an exact numeric `127.0.0.1` or `::1` endpoint and sends
no application bytes. The Windows adapter obtains the server-side established
connection row for the exact local/remote address AND port tuple. It retains a
live process handle and proves membership in the already-owned process job.
It also requires the retained root to remain alive; surviving descendants do
not make an exited monitor healthy. A caller cannot supply an arbitrary PID.

The adapter uses the supported Windows
[GetExtendedTcpTable/owner-PID row](https://learn.microsoft.com/en-us/windows/win32/api/tcpmib/ns-tcpmib-mib_tcprow_owner_pid)
and [IsProcessInJob](https://learn.microsoft.com/en-us/windows/win32/api/jobapi/nf-jobapi-isprocessinjob)
contracts. Address/port decoding honors the SDK network byte-order fields.
The table is bounded to 4 MiB and three size-growth retries, with checked row
counts/lengths and duplicate rejection. Errors never fall back to a port/name
match, arbitrary PID adoption or an unattested request. Other platforms refuse
before TCP connection until their platform adapter is implemented and verified.

The HTTP engine receives THAT connected stream. It cannot reconnect to the
same port after verification, resolve DNS, use a proxy, redirect, or obtain a
different stream from a connection pool. The retained process/job guard stays
alive through request/body processing. A connection closed during verification
cannot be replaced by a new listening process inside that existing TCP stream.

This is OS ownership of our launched process tree, not publisher/package
authenticity or Cryptex peer trust. Same-user privileged manipulation, hostile
kernel actors and injected code are not a new sandbox guarantee. The future
controller must retain the private profile, establish package/config identity,
and satisfy the remaining activation gates before production use.

## Fixed authenticated operations

`OwnedSyncthingRest` supports only version/status observations and an explicit
shutdown request. No arbitrary URL/path/body mutation interface exists.
The old URL-based `SyncthingRest` adapter and reqwest dependency are test-only;
the existing 19 negative fixture entries remain intact. Production cannot use
that adapter as a reconnect shortcut around ownership verification.

Standard hyper HTTP/1 framing runs on the verified stream, using already locked
versions: hyper 1.11.1, hyper-util 0.1.21, http-body-util 0.1.5, bytes 1.12.1,
tokio 1.53.1 and futures-util 0.3.34. No custom HTTP or cryptographic protocol is
introduced, and no package release version was upgraded. The initial mistaken
futures-util 0.3.32/default-macro selection failed offline; the final pin reuses
0.3.34 with only std and requires no new futures-macro dependency.

- Exactly numeric loopback, nonzero port, no scoped/mapped IPv6 alternatives.
- Three-second shared monotonic deadline across version+status; shutdown first
  rechecks version/identity, then POSTs under its own shared three-second budget.
- Pinned version 2.1.5 and expected native identity are checked, not inferred
  from a successful TCP connection. Native identity syntax is not a checksum
  validation, key-possession proof or canonical trust grant.
- 64 response headers, 16 KiB parser buffer and at most 64 KiB streamed body.
- No decompression; Content-Encoding, redirects, unauthorized/error status,
  malformed/truncated/duplicate JSON and oversize bodies refuse.
- Error/Debug surfaces contain fixed diagnostics/status codes, not credentials,
  response bodies, endpoint addresses or expected native identities.
- Every reconnect repeats ownership verification. An expired request closes
  only its client socket; it does not prove cancellation of the native worker.
- HTTP shutdown success is not exit proof: the controller must wait for the
  owned tree, preserve forced cleanup as failure, and inspect native stop logs.

## Verification and remaining work

Eight owned HTTP tests cover direct IPv4/IPv6, an owned monitor child, an
unrelated listener receiving ZERO application bytes, exited root, dead root
with a surviving child, identity-checked shutdown, total deadline, fixed paths,
redirect/auth/version/duplicate/oversize/chunked/truncated/encoding refusal and
endpoint rejection. Three SDK table-parser tests cover exact tuples, malformed
counts/lengths, duplicate candidates and non-established/zero-PID rows.

The real private-profile/Windows Credential Manager native fixture now uses
owned connections for version/status, negative authentication, test-only live
config inspection and shutdown. It retains the real private profile/owned job,
starts/stops twice, verifies isolation/log redaction/clean exits and deletes its
synthetic credential. The legacy fake-store fixture remains for regression.
Only test code exposes config inspection; product operations stay fixed.

Still open: product package/provenance and native config generation/validation,
controller startup/health/restart and lifecycle integration, the observed
intermittent credential-deletion failure, final Windows batch, then the single
consolidated Mac stage. ADR-0019 trust/bootstrap/security-event/issuer-authority
and reader-first compatibility gates remain mandatory. M8 is not complete.
