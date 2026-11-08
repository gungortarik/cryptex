# M8 isolated Windows VM platform checks — 2026-11-08

## Purpose and limits

After the selected Mac native tests and corrected strict Clippy passed, the
available next platform target is the owner's existing Windows VM. The Windows
development host's five owned-HTTP failures remain open; its TCP diagnostic
observed six exact-established-owner mismatches in 60 successful before/after
loopback round trips. These observations are not permission to relax endpoint
ownership, established-state, identity, redirect or deadline checks.

The new portable package compiles pinned source on that VM and exercises the
same current tests through native Windows APIs. It does not reuse an unreviewed
port, adopt a caller PID, alter trust, activate ordinary peer transport or use
the owner's earlier paired Cryptex runtimes. No real secure-store account is
accessed. All local canonical fixtures and API credentials are synthetic.

## Runner and source binding

`cryptex-win-test-20261108.zip` contains exactly six members:
`WIN-TEST.cmd`, `WIN-TEST.ps1`, `WIN-TEST-BASLA.txt`, `SOURCE-COMMIT.txt`,
`SOURCE-SHA256.txt`, `source.zip`. The packager uses one committed source revision
for both wrapper scripts and archive, not dirty working-tree files. It rejects
source symlinks, unsafe paths and expansion beyond 128 MiB, then verifies outgoing
members/bytes. CMD is CRLF; script/source bindings are normalized text.

The VM script rechecks source SHA-256 and bounded archive entries (including
case aliases, root/traversal/ADS paths and symlinks) before extraction into a
fresh `%LOCALAPPDATA%\Cryptex-Dev\win-test\<random>` runtime. Tests create
additional exclusive synthetic temporary directories outside that source tree.
Build output is redirected to a sibling target directory. Initial compilation
uses existing Rust 1.98.1/C++ tools and locked Cargo dependencies, which may
download; later test runs are offline. No Git/Node/pnpm or toolchain installer.

Only child-process script execution uses `-ExecutionPolicy Bypass`; system
policy, Application Control, antivirus and firewall configuration are unchanged.
Blocked execution must be reported as a failure, not bypassed or marked passed.
The package includes source rather than shipping a host-built test executable.

## Required native checks

1. `tools/m8/windows-tcp-state-probe.ps1 -Iterations 10 -Summary` and `-SelfTest`.
   Both exit statuses/logs are retained. Synthetic loopback is not Cryptex trust.
2. Build daemon library tests with `cargo test --locked ... --no-run`.
3. `instance::tests::`: exactly nine passing entries, including hardlink
   refusal, bounded PID observation and Windows live lock rename refusal.
4. `incoming_publication::tests::`: exactly nine passing entries, including
   signed approved fixtures, immutable replay, negative/stale batches, committed
   refresh failure and abrupt-process transaction recovery. FakeSecureStore is
   used; no real owner profile is read or updated.
5. `owned_process::tests::http::`: exactly eight passing entries, including
   IPv4/IPv6/owned child, unowned server zero bytes, dead root, redirect/auth/
   version/size/encoding handling, identity mismatch and total deadline.

Native stdout/stderr are drained asynchronously; exit status is preserved.
Test groups continue on nonzero exit so all relevant failures can be inspected.
No empty/missing summary or wrong count can pass. A build/setup failure still
returns metadata and the logs available at that point.

## Verification and next receipt

Host wrapper checks: PowerShell AST parse; real hidden child-process smoke with
synthetic stdout/stderr and exit seven; ordered report metadata updates; Python
packager parse and outgoing pinned archive verification. Existing Windows strict
all-target Clippy had already passed for the source correction. These are not
the VM's native test result. The five failing host HTTP tests were not weakened
or repeatedly rerun as a substitute for the clean VM comparison.

Owner extracts the package in the existing Windows VM, opens `WIN-TEST.cmd` and
returns `WIN-TEST-SONUC.zip`, including failure cases. Expected report members:
`result.json` and created members of `tcp-summary.log`, `tcp-parser.log`,
`build.log`, `instance.log`, `publication.log`, `owned_http.log`. The JSON scope
is `isolated_windows_platform_tests`, probe `m8-windows-platform-20261108-v1`.
Require source commit/hash, all five check records, build exit zero, expected
nonzero group counts, outcome pass and runner exit zero before marking it passed.
Any other outcome remains diagnostic evidence only.

Mac selected native checks and corrected lint are complete at their recorded
scope. This package does not close managed-adapter/controller wiring, Mac
production publication parity, physical power-loss durability or independent
security review. Normal publication remains disabled; M8 is not complete.

## Exact outgoing package

- Source commit: `c55f3b70c763aef7541481e3897dba348d300c47`.
- Embedded source ZIP SHA-256:
  `7ef0213a8eb9045a0fb1112e3485e24a352957af4d59fceaf453e44f276fe493`.
- `cryptex-win-test-20261108.zip`: 2,278,986 bytes; SHA-256
  `F06A601E114677AC62C434FC902A528CD4C5CA5647FE7BDE0EEA35C5DDC3109F`.
- Six exact members, source/script byte bindings, source digest, CRLF launcher
  and LF PowerShell checked before delivery. Source passes the runner's entry,
  case-alias, path, symlink and expansion checks. Native VM results are pending.
