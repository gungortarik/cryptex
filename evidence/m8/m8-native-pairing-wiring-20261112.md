# Disabled native pairing wiring — 2026-11-12

The separate signed candidate UI now registers the existing pairing frontend
command names. Brain identity, transport identity, offer/proof, peer preparation,
confirmation, cancellation and canonical completion all use the content-bound
native owner thread. There is no ordinary Unix fallback or product activation.
Generating a random transaction ID is backend-local and grants no authority.

The existing daemon exchange and approval domain services remain authoritative.
Desktop checks request/reply IDs, protocol and method correlation; only explicit
empty rejection replies can use the domain's rejection operation. It verifies
the signed role-specific prefix and compares the full returned approval snapshot
before retaining a private grant. Followups retain the exact original transcript,
request ID and grant; the renderer supplies only an opaque local handle. Handles
cannot cross owner/peer operations. Existing worker deadlines, cancelled-caller,
delivery-failure and daemon-loss invalidation apply to the added commands.

The transport-identity read is a closed read-only native method with no supplied
identity, transaction, transcript or grant. Its getter is shared with offer/proof
validation. The Mac authenticated managed-transport status backend is not yet
implemented: read, offer and proof explicitly return
`transport_identity_unavailable`. No fake adapter or test override was added to
the feature executable. Successful end-to-end native pairing is still open.

## Validation

- Local Rust formatting and daemon all-target strict clippy passed.
- Python Mac tooling suite: 72 tests, 5 platform skips, no failures.
- Local daemon native-channel tests compiled, but Windows Application Control
  refused to execute the test binary (OS error 4551). No policy was changed.
- Shared decoder tests reject supplied identities/grants for the transport read.
- Feature Mac unit tests cover signed prefix order/role, both transport IDs,
  owner/peer handle isolation, frozen followup bytes, and correlation/rejection
  confusion. These require the Mac feature CI run; they are not local Windows
  execution evidence.
- The signed native subprocess check now requires
  `native_pairing_boundary_refusals=passed`: real Brain read; unavailable adapter,
  offer and proof refusals; malformed completion and missing-session refusal;
  unchanged canonical event count/prefix. Existing owner/restart/raw-replay
  checks remain required. This marker does not establish positive peer pairing.

At source `3208d21`, [CI run 43289373287](https://github.com/gungortarik/cryptex/actions/runs/43289373287)
passed Windows, including the new transport-read decoder test. Mac feature lint
and all four new client unit tests passed. The existing native supervised test
failed at its 30-second child deadline immediately after codesign reported
signature replacement (31 passed, 1 failed, 2 ignored); the actual startup and
pairing-boundary driver had not run yet. The failure duration identifies the
signing-child wait, not a native message timeout. A disposable-keychain idle
lock after the long feature build is suspected, not independently established.
The CI harness now explicitly unlocks only its temporary fixture keychain again
after feature build and before feature tests. Existing signing/native deadlines
and refusal assertions remain unchanged.

The followup [run 43291468456](https://github.com/gungortarik/cryptex/actions/runs/43291468456)
at `6ee4893f3441212d13098b1b8d487a631b04ff01` passed both platform jobs.
The Mac log explicitly reports all of:

- `real_signed_desktop_daemon_startup_restart=passed`
- `native_pairing_boundary_refusals=passed`
- `verified_tauri_assets=passed`
- `native_synthetic_approval_sessions=passed`
- `native_lost_session_and_old_handle_refused=passed`
- `native_prepare_and_actor_failures_burn_grants=passed`
- `native_keychain_same_profile_restart=passed`
- `native_keychain_confirmed_history_preserved=passed`
- `native_keychain_old_local_handle_refused=passed`
- `native_keychain_cross_daemon_wire_replay_refused=passed`
- `native_keychain_missing_key_refused=passed`

Remaining ordinary Desktop, approval connection/correlation and runtime admission
checks passed. This confirms the followup source and fixture run; it does not
independently prove the suspected idle-lock cause of the earlier signing timeout.
No new owner device run is requested until a meaningful positive native flow is ready.

## Remaining gates

Authenticated Mac managed-transport status binding; successful native exchange,
peer approval and completion with restart/delivery faults; final independent
security and human review; product publisher, supported OS floor and explicit
activation. M8 remains open.
