# M8 Windows startup/status guest artifacts follow-up

Date: 2026-10-22. Evidence class: owner-supplied guest outputs, locally checked
against the staged package and source manifest.

## Package identity

- Package: `C:/Users/Tarik/Downloads/Cryptex-M8-StartupStatus-20261022.zip`
- SHA-256: `1c9ba03b53801ce4b0a5cbc9a49fbdb82a3bb12d12339bb10ffc8bd27b814016`
  (matches the staged hash recorded in the guest-receipt note).
- All 33 normalized source hashes in
  `m8-startup-status-source-sha256-20261022.txt` match both the ZIP's `source/`
  entries and the current checkout. Line-ending normalization is applied as
  specified by the Windows verifier.

## Guest results supplied by the owner

`results.json` reports exit code 0 for all eight runner steps: `fmt`,
`package-tests`, `clippy`, `build`, `native-owned`, `native-private`,
`native-pinned-package`, and `native-provision`.

`native-provision.log` identifies the targeted test
`managed_profile::native::tests::real_pinned_profile_provision_reopen_and_collision_recovery`;
that test reports 1 passed, 0 failed. The standard Cargo stderr progress lines
were logged as `NativeCommandError` records by PowerShell 5.1, but the runner
captured native exit status 0 and continued through all eight steps.

The supplied `native-provision-result.json` records these checks as true:

- Windows Credential Manager used; the synthetic credential was deleted.
- Loopback isolation and identity preservation held after injected faults.
- The raw generation home was never served; API-port collision was refused
  before API bytes; an explicit retry after collision was clean.
- Authentication rejections were `[403, 403, 403]`; foreign endpoint collision
  was refused.
- Worker duplicate-profile and missing-receipt recreation attempts were refused.
- Managed daemon start/stop was clean and the daemon instance lock was released
  after worker join.

The result also records the observed process IDs and bounded controller restart
and exhaustion timings. Those values are test-run observations, not production
performance guarantees.

## Scope and disposition

This closes the previously blocked Windows guest execution gate for the staged
startup/status and owned-profile verification package, at the source hashes
above. It does not establish authenticated Desktop `transport.status` behavior
with a native GUI peer, peer admission, incoming replication, offline
convergence, cutoff/restriction policy, protocol acceptance, or full M8
completion. No Actions, canonical trust writes, commit, push, or merge occurred.

Next M8 trust work remains gated on a successful independent security review of
the proposed ADR-0019 addendum and explicit owner acceptance. The attempted
Cursor review is blocked by its local IDE hook (`Cortex privacy guard is
unavailable; protected read blocked`); no security/privacy settings were
changed. M7 managed-note write-back remains disabled.
