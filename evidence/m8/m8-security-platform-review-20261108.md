# M8 security self-review and platform checks — 2026-11-08

## Decision and scope

**Normal publication activation: NOT READY.** This is a self-review by the
implementation author of base `2705330`, followed by scoped corrective changes.
It is not an independent security approval. No CLI, IPC, watcher or managed
adapter publication entry point was activated; event/journal schemas and trust
policy are unchanged.

Authority reviewed: Master Spec, accepted sync/platform contracts, Definition
of Done, and owner-approved Windows implementation slice in ADR-0019 protocol
addendum §5.1.1. Scope: runtime ownership, incoming staging, exact-replay admission,
lock-bearing M3/M4/trust proof lifetime, prepared/commit error boundaries,
cleanup/restart behavior, and available native-platform checks.

The transport candidate and runtime metadata are untrusted. A genuine signature
does not grant admission; no public key supplied by a transport fixture enters
the trusted roster. The scenario below requires a pre-existing local filesystem
alias; it is not a demonstrated unauthenticated remote network attack.

## Fixed findings

### P1 — aliased lifecycle lock could truncate immutable bytes

Before correction, `instance::try_acquire_instance` opened `daemon.lock` with a
normal following writable open, acquired a lock, then called `set_len(0)` before
writing the PID. Safe import-root directory pins do not prohibit a hardlinked
lock leaf. In an isolated synthetic directory, linking the lock name to a
synthetic immutable file made acquisition succeed and modify the target.
`hardlinked_instance_file_cannot_truncate_an_immutable_file` failed before the
fix. No owner profile, private key or real canonical event was used.

Correction: `platform::instance_file` opens without truncation and refuses
anything except an ordinary single-link file through the opened handle before
PID mutation or inspection. Windows opens the reparse point itself, checks its
attributes/link count and omits delete sharing; Unix uses `O_NOFOLLOW`,
`O_NONBLOCK`, ordinary-file/link-count checks and private creation mode.
The caller revalidates after obtaining the exclusive file lock. This does not
claim to sandbox a malicious same-user process or replace import ancestor pins.

Microsoft documents that omitting delete sharing prevents delete/rename access
while the handle remains open; the latest Windows rename-refusal regression
also passed. See [CreateFileW sharing and reparse semantics](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).
Mac/Unix execution of the new adapter remains pending; compilation on Windows
is not proof of that platform's behavior.

Regression evidence on Windows:

- Acquisition and observation refuse a hardlinked lock and preserve target bytes.
- Persistent import refuses a runtime whose lock aliases existing canonical
  bytes before staging or changing those bytes.
- An open instance lock cannot be renamed; after guard release rename succeeds.
- Existing duplicate-instance, live observation, stale lock and restart cases pass.

### P2 — unbounded PID observation and second path open

PID observation previously unlocked and reopened the path, then read the entire
file. It now reads at most 129 bytes through the same locked, validated handle
and rejects more than the 128-byte bound. An oversized synthetic file is rejected
without modification. This bounds malformed local metadata and removes the
unlocked reopen window; lock/status contents are never echoed into diagnostics.

### Concurrency hardening — exclusive caller-owned instance capability

The crate-private daemon-owned publication entry now takes `&mut InstanceLock`.
Two safe-Rust import/recovery operations cannot overlap through the same guard
without an explicit serialization decision. Matching ownership is reused without
reacquisition; a foreign-runtime guard rejects before staging. This is not the
whole live-controller admission barrier: other readers and runtime services must
still be held during startup recovery and after recovery-required outcomes.
No live caller exists in this slice, so that activation prerequisite remains open.

## Checks and limits

| Check | Observed result |
| --- | --- |
| New hardlink refusal reproducer before fix | Failed as expected; defect reproduced |
| Latest instance group after fixes | 9 passed on Windows |
| Latest incoming publication group after fixes | 9 passed on Windows |
| Daemon binary/CLI group | 15 passed on Windows |
| Broad daemon library run after lock-file fix | 553 passed, 5 failed, 4 ignored |
| Strict daemon all-target Clippy | Passed |
| Workspace format / diff checks | Passed |
| Mac preflight Bash syntax and package allowlist/mode/LF | Passed on host |
| Mac runner invoked on non-Mac host | Refused, exit 2 |
| Actual Mac readiness, native tests and secure store | Pending owner receipt |
| Physical power loss / marker-parent durability | Not established |
| Independent security review | Not performed by this self-review |

The broad run preceded the final exclusive-guard hardening and last rename
regression; the final focused groups and Clippy cover those changes. No full
green library-suite result is claimed. Test logs remain outside Git beneath
`%LOCALAPPDATA%\Cryptex-Dev\m8-security-review-20261108`.

### Windows endpoint failures

Host OS: `10.0.26300.0`, revision `26300.9818`, display version `26H2`.
Runtime scratch volume C: and repository volume D: report NTFS. Five failed
tests under `owned_process::tests::http`:

- `owned_http_identity_mismatch_and_total_deadline`
- `owned_http_ipv4_ipv6_and_owned_monitor_child`
- `owned_http_rejects_redirect_auth_version_duplicate_oversize_truncation_encoding`
- `owned_http_shutdown_refuses_wrong_identity_before_post`
- `owned_http_unowned_server_receives_zero_application_bytes`

All stopped at `Endpoint(PeerNotEstablished)` rather than the intended later
identity/redirect/ownership result. A fresh ten-iteration synthetic
`tools/m8/windows-tcp-state-probe.ps1 -Summary` run observed 60 samples,
six exact-established-owner mismatches (IPv4), raw states 0/1/5, with all
60 before/after application round trips succeeding. Its parser/bounds/exact
tuple self-test passed. This is consistent with the historical host TCP-table
issue, but cannot establish that every production endpoint case is correct.
The five failures remain open; ownership/ESTABLISHED checks were not loosened.

An intermediate build could not execute because Windows Application Control
returned 4551. Native Code Integrity recorded 3077/3033 and Smart App Control
3118. After the genuine API hardening change, an ordinary Cargo build ran both
focused groups; no protection, signing policy or antivirus settings changed.
This intermittent observation does not certify distribution/install readiness.

## Mac package and next native evidence

Owner confirmed a real Mac is available. Readiness archive:
`%USERPROFILE%\Downloads\cryptex-mac-kontrol-20261108.zip`.
SHA-256: `36B33F6C1FCCD206850DEBF86EB042B751F1188EA6CF6851E395109EEBC778BD`.

Exact members: `MAC-KONTROL.command` (Unix regular executable 0755) and
`MAC-BASLA.txt` (0644), UTF-8/LF. Reproducible packager:
`scripts/m8/package-macos-readiness.py`. It includes no binary, Cryptex data or
credentials. The command neither installs/updates tools nor uses the network;
`rustup run 1.98.1 rustc --version` only queries an existing toolchain. It reads
limited OS/tool readiness, creates new private synthetic scratch files and
returns `MAC-SONUC.zip` containing `platform.txt`. Full disk information is
filtered to a recognized filesystem type, with `unknown` as an honest fallback.
Its native utility behavior has not yet been verified on the owner's Mac.

Mac readiness is not canonical publication, native secure-store integration,
signed/notarized distribution, crash recovery, or power-loss evidence. Return
the receipt before choosing architecture-specific artifacts/tool prerequisites.
If opening is blocked, inspect the warning; do not disable platform protection.

Before normal activation: obtain independent review of corrected code; resolve
supported Windows endpoint evidence; implement/review the live controller's
runtime-wide failure barrier; verify native Mac adapters and parity; and close
the existing durability/platform gates. The Windows-only dormant import service
must not simply be enabled on Mac by removing `cfg(windows)`.
