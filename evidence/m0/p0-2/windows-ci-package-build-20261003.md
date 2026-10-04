# Windows package build on CI — pending execution

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
against that release's checksum file. Actual job execution remains required.

This single scoped build is justified because the required current Windows
package cannot be produced on the local host. An unsigned CI artifact does
not establish trusted signing, local policy acceptance, physical Windows
tray/login behavior or package update/rollback safety. Preserve any subsequent
host policy rejection and do not disable security controls to run the artifact.
