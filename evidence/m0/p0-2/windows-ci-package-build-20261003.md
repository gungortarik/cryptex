# Windows package build on CI — build PASS; installer execution pending

The local release build is blocked by Windows Code Integrity rejecting the
generated `serde_repr` proc-macro DLL. See
`windows-release-desktop-owned-supervisor-20261003.md`. Do not bypass the local
security policy or repeatedly rebuild the same blocked artifact.

The existing CI workflow now provides the manual boolean input
`windows_desktop_build_only`. Setting it to true runs one Windows job; readiness
(including Mac) and cross-account IPC jobs are skipped. Normal push/PR and
IPC-only behavior are preserved. No automatic packaging trigger was added.

The job builds a development NSIS installer, runs sidecar staging tests and
release Desktop helper tests, and uploads the installer with the Desktop and
sibling daemon executables plus source/run identity and SHA-256 hashes.
Artifacts expire after seven days. It does not upload runtime or canonical
data, publish a GitHub release, install software on the owner's PC, or change
the owner's login registration. Timeout is 25 minutes.

Local validation: `actionlint` 1.7.12 and `git diff --check` passed. The
actionlint binary was obtained from its upstream GitHub release and checked
against that release's checksum file.

Manual run: https://github.com/gungortarik/cryptex/actions/runs/37168036553
at source `0867b9f86152ec972419fcd58ffd46aa72005d50`. Only the Windows
Desktop package job started; readiness and IPC ACL jobs were confirmed skipped.
The run completed successfully. Sidecar staging tests (3), the unsigned NSIS
build, release Desktop helper tests (8) and artifact upload passed. No second
workflow was dispatched.

The artifact was downloaded outside Git to
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\ci-package-37168036553`. All three file sizes
and SHA-256 hashes matched the provenance manifest, whose source commit also
matched the requested build:

| File | SHA-256 |
|---|---|
| `Cryptex_0.0.0_x64-setup.exe` | `E3A3A0F6B9C196A6010B8EBC1194673E5667EF0AD263756A663BFC73DAA138E7` |
| `cryptex-desktop.exe` | `7FA368B20A3732658D017EE5A21FCA0049A6AC62DE7386D133A89A88D3AE7E25` |
| `cryptexd.exe` | `30DA42364E2B3B0F9E6C273F9738921EAA01E651C9882E11D27BFB0CC596CE29` |

The loose Desktop executable ran locally as PID `2684` and automatically
started sibling daemon `28884` under the product runtime; the embedded
renderer was visible. No installer was executed. The renderer login toggle
reported enabled and the direct HKCU read showed `Cryptex` pointing to this
Desktop with `--cryptex-background`, but `Win32_StartupCommand` did not expose
that product value. This registry-view mismatch leaves real login preparation
unverified; no reboot/login pass is claimed. The renderer disabled the value
again, direct HKCU confirmed it absent, and renderer Quit clean stopped both
processes with `state=clean_quit`. The exact `stop\n` marker was removed after
process exit. The next check requires launching the executable from the
operator's ordinary Windows session and comparing the registration again.

## Ordinary-session login preparation — registration verified, login pending

The operator subsequently launched the same loose executable through Explorer.
Desktop `12084` had Explorer `13640` as its parent and started daemon `8052`.
The renderer login toggle was enabled again. This time `Win32_StartupCommand`
independently exposed exactly one per-user `Cryptex` Run value targeting
`ci-package-37168036553\cryptex-desktop.exe --cryptex-background` under the
operator's SID. No manually substituted registry entry was used.

Preparation was retained at `2026-10-04T01:46:23.0788469Z` (October 3 locally)
in `%LOCALAPPDATA%\Cryptex-Dev\p0-2\windows-desktop-login-1003` with the exact
registration, source commit and pre-login PID/snapshot records. Login startup
is deliberately enabled for the next real sign-out/sign-in. The earlier
agent-launched context mismatch is historical; it does not establish an
autostart adapter defect.

After the operator signs out and back in, inspect process start time/command,
one Desktop with `--cryptex-background`, one daemon with a held lock, and a
hidden window before any Open action. Then use physical tray Open, check
renderer status/IPC, disable login startup through the renderer, verify the
product Run value is removed, and cleanly Quit. Preserve evidence before
cleanup. Do not trigger CI or repeat Mac checks for this procedure.

This single scoped build is justified because the required current Windows
package cannot be produced on the local host. An unsigned CI artifact does
not establish trusted signing, local policy acceptance, physical Windows
tray/login behavior or package update/rollback safety. Preserve any subsequent
host policy rejection and do not disable security controls to run the artifact.
