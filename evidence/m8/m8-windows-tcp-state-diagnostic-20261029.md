# M8 Windows TCP state diagnostic — 2026-10-29

## Finding and boundary

The failing owned-process HTTP tests are reaching the TCP table gate before
job membership or HTTP response validation. Some exact server socket tuples
are returned with a state other than `MIB_TCP_STATE_ESTAB` (5). Cryptex refuses
these tuples before sending an HTTP request or credential.

An independent C# / Win32 reproducer now lives at
[`tools/m8/windows-tcp-state-probe.ps1`](../../tools/m8/windows-tcp-state-probe.ps1).
It creates ordinary IPv4 and IPv6 loopback socket pairs in one process, accepts
each connection, exchanges fixed synthetic bytes in both directions before
and after each TCP-table snapshot, and compares the exact server address/port,
client address/port, state, and owning PID. It does not use Cryptex, owned jobs,
HTTP, profiles, API credentials, Syncthing, or a remote endpoint.

On this host (Windows build **26300.9818**, DisplayVersion **26H2**), a three-
iteration run produced **18 samples**, with successful round trips before and
after **all 18** snapshots. **6 IPv4 samples** nevertheless reported state 0
or 6 for the exact server tuple; other samples reported state 5. The owning
PID matched the probe process. The probe exited **1**, correctly indicating
inconsistent native attestation. Counts may vary on subsequent runs.

This reproduces the attestation anomaly independently of the Rust HTTP/job
fixture. It does not establish whether the underlying cause is the Windows
build, a local networking component, or another host condition. It is not
evidence that an unestablished TCP row is safe to trust.

## Change

- Added typed `EndpointError::PeerNotEstablished` for an exact socket tuple
  whose native state is not established. Missing tuples and failed process/job
  ownership still report `PeerNotOwned`. No raw endpoint, body, or credential
  logging was added.
- Preserved the existing established-state, exact-tuple, nonzero-PID,
  duplicate-row, live-process, and retained-job requirements. Acceptance is
  unchanged; only the diagnostic rejection is more precise.
- Added IPv4/IPv6 regressions for exact versus unrelated tuples, invalid and
  closing states, scoped IPv6 rejection, and retained closing rows alongside
  an established row.
- The standalone probe's self-test verifies malformed table rejection,
  duplicate preservation, exact tuple matching, and raw state decoding for
  both address families. The live probe uses only bounded loopback operations
  and closes its sockets when done; output is JSON, with no runtime files.

## Reproduce

```powershell
powershell -NoProfile -File tools/m8/windows-tcp-state-probe.ps1 -SelfTest
powershell -NoProfile -File tools/m8/windows-tcp-state-probe.ps1 -Iterations 3
cargo test --locked -p cryptexd --lib owned_process::tests::http:: -- --test-threads=1
```

Probe exit 0 means all sampled rows matched the expected native state and PID;
exit 1 means at least one did not. A probe pass is not a Cryptex/native fixture,
transport, security, or M8 acceptance pass.

Add `-Summary` to emit compact JSON for a second-host comparison.

## Verification of this diagnostic slice

- Native probe: **18 samples; 6 mismatches; all 18 round trips before and
  after snapshots passed**. Exit 1 records the attestation mismatch.
- A subsequent compact-output smoke with one iteration had 6/6 matches; a
  ten-iteration comparison baseline had **60 round-trip passes and 3 IPv4
  state-0 mismatches** (other rows state 5), exit 1. A short clean sample does
  not resolve the intermittent anomaly. Use `-Iterations 10 -Summary` on the
  second host for the same sample size.
- Probe `-SelfTest`: PASS for IPv4/IPv6 bounds, duplicate rows, raw states,
  and exact versus unrelated tuple matching.
- `cargo test --locked -p cryptexd --lib --bins -- --test-threads=1`:
  daemon library **516 passed / 4 failed / 4 ignored**. All five TCP-table
  parser regressions passed. The failures in this run were
  `owned_http_identity_mismatch_and_total_deadline`,
  `owned_http_ipv4_ipv6_and_owned_monitor_child`,
  `owned_http_rejects_redirect_auth_version_duplicate_oversize_truncation_encoding`,
  and `owned_http_unowned_server_receives_zero_application_bytes`, each
  rejected with `Endpoint(PeerNotEstablished)` before its expected result.
  No HTTP assertion was changed to accept this earlier refusal. Cargo stopped
  at the library failure, so binary tests are not claimed from that command.
- Separate `cargo test --locked -p cryptexd --bin cryptexd -- --test-threads=1`:
  **12/12 passed**.
- `cargo clippy --locked -p cryptexd --release --all-targets -- -D warnings`:
  PASS.
- `cargo fmt --all -- --check` and `git diff --check`: PASS.

## Second-host probe — owner-reported result

The owner ran the pinned probe from source commit
`13e8d02a24510bc71e46625f466c140c761eafdc` on the other Windows/VM and supplied
the console output in chat. This is owner-reported native probe evidence, not
an agent-observed Cryptex test run.

| Measurement | Development host | Second Windows/VM |
| --- | --- | --- |
| Build revision | 26300.9818 | 26200.9711 |
| Display version | 26H2 | 25H2 |
| Samples (`-Iterations 10 -Summary`) | 60 | 60 |
| Round trips before and after snapshot | 60 passed | 60 passed |
| Exact state/PID mismatches | 3 (IPv4) | 0 |
| Observed states | 0, 5 | 5 only |

The first second-host invocation was blocked by its script execution policy;
no probe result is attributed to that attempt. The subsequent process-scoped
invocation completed and returned the JSON above:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP\cryptex-tcp-probe.ps1" -Iterations 10 -Summary
```

This supports a host/build-dependent attestation anomaly. It does not identify
the underlying component or establish a general Windows-version regression.
The probe pass alone does not close the four development-host HTTP failures.

## Second-host owned HTTP tests — owner-reported PASS

The owner downloaded the source archive pinned to
`a5f859a34e3b9946527d97d1187c2c5ac72381a8` into a fresh temporary directory on
the same second Windows/VM (26200.9711 / 25H2). Git was unavailable; Cargo and
the required Rust 1.98.1 MSVC toolchain were present. Rustup's optional
self-update failed with access denied, but the pinned toolchain was already
installed. The owner then ran the test command directly:

```powershell
cargo +1.98.1 test --locked -p cryptexd --lib owned_process::tests::http:: -- --test-threads=1
```

The owner reported **`test result ok 8 passed`**. This is a scoped,
owner-reported HTTP-group PASS at the pinned checkpoint. A complete console
transcript was not supplied for that successful invocation; no additional
suite or packaged/native Desktop result is inferred.

Together with the independent 60/60 probe, this strengthens the evidence for
a development-host/build-dependent attestation problem. It does not prove
which OS/networking component is responsible or fix the local failures.
No ownership check, established-state gate, or HTTP assertion was weakened.

## Next gate

Run the complete daemon library and binary tests on the second host with the
same pinned source/toolchain before continuing native owner UI verification.
Keep the development-host networking anomaly open and compare its native
fixture/stack before proposing any production change. Do not bypass state or
job checks, enable a listener-only fallback, or weaken response expectations.
Native Desktop owner-flow verification remains open; no native UI pass is
claimed here. Production transport/publication, macOS parity, independent
review, and M8 completion remain open.

## API references

Microsoft documents state 5 as established and describes the owner-PID table
layout in [MIB_TCPROW_OWNER_PID](https://learn.microsoft.com/en-us/windows/win32/api/tcpmib/ns-tcpmib-mib_tcprow_owner_pid).
The snapshot API and its supported table classes are documented in
[GetExtendedTcpTable](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getextendedtcptable).
