# M8 Windows managed profile verification — partial; final launch blocked

Date: 2026-10-20. Worktree:
`C:/Users/Tarik/.codex/worktrees/m8-device-trust/cryptex`,
branch `codex/m8-device-trust`, base `74bc3b14127962579ae03838bafea9336612a033`.
Source changes remain uncommitted. Original dirty checkouts were preserved.
Contract: `m8-managed-profile-boundary-contract-20261020.md`.

## Observed checks and exact limits

| Check | Result |
| --- | --- |
| Final `cargo fmt --all -- --check` | PASS |
| Final `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` | PASS |
| Final `cargo build --locked -p cryptexd` | PASS |
| Profile tests during implementation | 20/20 PASS, including one worker helper |
| Broader implementation run | events 61; schema-generator 0; daemon lib 341 PASS / 2 opt-in native tests ignored; CLI 8; lifecycle 9 |
| That broader run's continuation | BLOCKED launching Mode B test executable, OS error 4551; remaining targets not executed in that run |
| Final-source package invocation/recheck | events 61 and schema-generator target PASS, then daemon test executable blocked by 4551 before execution |
| Early opt-in real native/profile/OS-store integration | 1 PASS; two actual native starts/stops, PIDs 2376/9056, exit 0, forced false, wrong API keys 403 |
| Final-source opt-in integration/recheck | BLOCKED by 4551 before test execution; early result does not certify the final candidate |
| PowerShell guest verification runner parser | PASS; execution not claimed |
| `git diff --check` | PASS |

The initial format check found only an unformatted multiline assertion in the
new test cleanup; formatting was applied and final checks pass. Preserve
`fmt.log` with that original failure. No test was weakened or removed.

The 20 profile entries exercised private creation/reopen, current-user DACL
checks, existing broad-directory refusal without repair, session exclusion,
root/ancestor rename refusal, root/ancestor junction refusal with unchanged
outside sentinel, metadata hard-link refusal, namespace/corrupt/oversized/
duplicate state, store-denial handling, occupied/replaced/missing credential
refusal, interrupted binding completion with only the same key, separate random
credential namespaces for identical folder names, and abrupt exit(97) recovery.
Real Windows Credential Manager roundtrip, loss and crash/reopen cases used
only unpredictable DEV synthetic accounts and explicitly deleted their keys.
These are process-interruption proofs, not power-loss proofs.

After the successful early native test, the credential account namespace was
made independently OS-random to prevent collisions across identically named
folders. The successful 20-entry/broader run includes that change. Final source
also improves the opt-in fixture's cleanup guard and explicit deleted-account
assertion/report; these final opt-in assertions remain unexecuted because of
the launch block. Do not carry the earlier native PASS onto the final source.

## Proof and source identity

Proof root:
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/managed-profile-20261020-230202-981e60b6221d476a9f0144b73ce0eb66`.

- `package-tests.log`: successful library/CLI/lifecycle results, then blocked Mode B launcher.
- `package-tests-verified.log`, `package-tests-recheck.log`: final-source launch blockers.
- `native-private-verified.log`, `native-private-recheck.log`: final-source native test launch blockers.
- `native-private-earlier-candidate.json`: earlier actual native PIDs/results, deliberately labeled as earlier.
- `fmt-verified.log`, `clippy-verified.log`, `build-verified.log`: final successful static/build gates.
- `SOURCE_SHA256SUMS_LF.txt`: final 16-file manifest, mirrored in
  `m8-managed-profile-source-sha256-20261020.txt`. UTF-8 bytes are normalized
  CRLF→LF while preserving any pre-existing BOM before hashing.

Final built daemon SHA-256:
`c20a1ab577d941939a93fe23427a3b6d9f5bc4e86e09d9090f28ad25a11171cf`.

Read-only Code Integrity events 3077/3033 corroborated the 4551 test-launch
refusal. No security settings, signing policy, loader or test-binary location
were changed to get around it. Successful earlier invocations do not establish
a global policy fix. No Syncthing process remained in the final observed
process snapshot; no user-global configuration-digest audit is claimed here.

## Continuation

Run the final candidate normally in the previously authorized Windows
development environment using `scripts/m8/verify-managed-profile-windows.ps1`
and the pinned unmodified native binary. It verifies source hashes, runs the
whole scoped Rust package matrix, fmt/Clippy/build and both explicit native
fixtures, and stops on failure with retained app-data reports. The runner's
execution and the final Windows gate remain OPEN until actual results arrive.
Do not disable host protection or use Actions as a substitute.

Prepared local development ZIP (not executed in the guest):
`managed-profile-20261020-230202-981e60b6221d476a9f0144b73ce0eb66/Cryptex-M8-Windows-ManagedProfile-20261020.zip`
under the app-data verification parent above. Size: 13,862,351 bytes. SHA-256:
`1058be763a7b198113a15d57c25bc3da05ae86fb388c0b3574174d2cd35c775a`.
Contains archived base source plus the scoped working source/docs overlay,
the final runner/manifest, unmodified pinned Windows native binary and upstream
LF notices/source-reference inventory. The first packaging attempt stopped on
incorrect notice filenames (`LICENSE`/`AUTHORS` instead of `.txt`); corrected
names were copied from the repository inventory before ZIP creation. The
completed ZIP was extracted separately: 16/16 source checksums and exact native
binary hash MATCH. This is artifact integrity, not verification execution or
publisher/signing approval. No test executable was copied to another location
and run to bypass host policy; the guest builds/tests normally with Cargo.

Production native-config/package provisioning, established endpoint ownership,
controller lifecycle integration, ADR-0019 bootstrap/security-event review and
peer trust/admission remain open. Mac stays in the later consolidated stage.
No production startup/IPC activation, canonical writes, Actions, commit, push,
merge, M7 write-back or M8 completion is claimed.
