# M8 platform batch delivery plan

**Date:** 2026-10-19  
**Owner direction:** Finish the Windows side before moving to Mac; use two
large platform stages rather than repeated small Mac transfer/check cycles.

## Stage 1 — Windows implementation and verification

Develop the complete M8 scope on `codex/m8-device-trust`: explicit pairing
UI/flow, managed Syncthing controller, authenticated peer admission, guarded
incoming validation, offline convergence, revoke/retire/compromise and sequence
cutoff, and truthful replication protection status.

Internal code units and targeted tests remain useful, but they are not separate
owner handoffs. Run relevant Windows checks during development and the complete
M8 Windows acceptance matrix at the stage boundary. A status-only diagnostic
or mocked peer fixture is not completion of this stage.

Make the currently unspecified peer-key authorization/bootstrap and durable
trust/cutoff design concrete through the architecture/specification process
before writing a trust-changing path. Existing M1 pairing metadata and transport
pairing must not become an implicit source of trust. The batching request does
not waive this architecture requirement.

Use only isolated synthetic runtimes for native verification. Preserve personal
data, unrelated Syncthing profiles, dirty main checkouts, and all existing M7
worktrees. Runtime and build data remain outside source control.

## Stage 2 — One consolidated Mac verification/hardening pass

After the complete Windows candidate has relevant Windows verification,
transfer one source candidate with baseline, hashes, build/test instructions,
and native Mac acceptance coverage. Resolve Mac-specific failures on that
candidate. Prior Slice 1 Mac PASS remains historical scoped evidence; it is
not a substitute for the complete M8 platform stage.

M8 remains incomplete until the accepted cross-platform, security/failure,
Definition of Done, CI and human-review gates are met. The owner has not
authorized Actions; report that final gate explicitly if it remains unavailable.

## Current Windows environment blocker

Normal retry of `cargo test --locked -p cryptexd --lib replication_status::`
still failed before tests with OS error 4551. Read-only Code Integrity event
3077 identifies `VerifiedAndReputableDesktop` policy and the generated
`curve25519-dalek` dependency build script; the policy registry state is 1.
No protection setting, policy, signing identity, or binary was modified to
evade the denial.

The owner opened the existing Windows 11 VM. `vmrun list` confirms
`D:/VMs/Win11/Windows 11 x64.vmx` running; a read-only UI observation confirms
the guest desktop. `vmrun getGuestIPAddress` is still blocked by the VM's
encryption-password requirement. No password was requested or guessed.

The computer-use skill prohibits terminal automation and authentication-dialog
automation. A fixed, owner-started guest verification batch is the supported
setup path if no existing authenticated guest CLI channel is available. This
is a one-time development-environment setup, not a separate M8 feature delivery.

Until guest verification runs, no Windows build/test success is claimed. M7
write-back remains disabled throughout both M8 stages.
