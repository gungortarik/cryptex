# M8 Windows owned endpoint verification

Date: 2026-10-21. Active isolated worktree:
`C:/Users/Tarik/.codex/worktrees/m8-device-trust/cryptex`,
`codex/m8-device-trust`, uncommitted on base `74bc3b1`.
Boundary: `m8-owned-endpoint-boundary-contract-20261021.md`.

## Observed results

| Check | Result |
| --- | --- |
| Format check | PASS |
| Non-test daemon library check | PASS; legacy URL REST adapter excluded from product |
| Strict daemon Clippy all-targets | PASS |
| Daemon build | PASS |
| New owned-HTTP test group | 8/8 PASS (multiple cases within entries) |
| SDK TCP-table parser entries | 3/3 PASS in scoped owned-process run |
| Legacy REST regression entries | 19/19 PASS |
| Final broad recheck: events | 61/61 PASS; schema-generator target ran (0 tests) |
| Final broad recheck: daemon library | 352 PASS / 2 opt-in native entries ignored |
| Broad invocation continuation | BLOCKED before daemon CLI test executable launch, OS error 4551 |
| Explicit real private-profile/OS-store/owned-endpoint fixture | 1 PASS, two native starts/stops, details below |
| Diff check | PASS |

Do not claim that all package/CLI/integration/doc targets ran in the final broad
invocation, or that Application Control is resolved. No security policy was
changed; no test executable was relocated, renamed or run through a different
loader to evade that policy. Earlier VM receipts remain separate source runs.

## Retained failures and open intermittent test

The first broad run failed in the new dead-root/live-child test's fixture
cleanup: remove_dir_all saw sharing violation 32 after forced job cleanup.
The test's process/ownership assertions passed before cleanup. The exclusive
fixture destructor now retries only sharing/lock errors 32/33 for at most two
seconds, keeping resolved-path checks and every functional assertion intact.
A persistent lock still fails; this changes test cleanup only, not product
deadlines, ownership checks or clean-exit classification.

The next broad run failed once in the pre-existing managed-profile abrupt
ready/crash test: immediately after CredDeleteW reported success, a read still
returned the synthetic credential instead of Missing. The exact missing-item
assertion remains unchanged; it was NOT replaced with retries or ignored.
Eight isolated unchanged reruns passed; the final parallel library recheck
also passed 352/352. The cause remains UNRESOLVED and must be investigated
before M8 closeout. Do not infer eventual consistency or blame the OS as a
proven explanation, and do not treat a passing recheck as closing this issue.
The failure printed fixed synthetic test bytes, not real user credentials.
No product SecureStore implementation was changed on this evidence.

An intermediate test addition omitted the existing force_cleanup timeout
argument; compilation caught it. The test was corrected to use TEST_TIMEOUT.
No expected outcome was weakened. All earlier logs are retained.

## Actual native integration

Final source's private-profile variant uses the retained unmodified Windows
Syncthing 2.1.5 binary SHA-256
`36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344`.
Only generated synthetic config/cert/key files are used; no personal/global
Syncthing profile or vault is used.

| Root PID | Exit | Forced | Observed wrong-key status |
| --- | --- | --- | --- |
| 22900 | 0 | false | 403 |
| 30040 | 0 | false | 403 |

Recorded flags: `established_endpoint_attested=true`,
`credential_store=windows_credential_manager`,
`synthetic_credential_deleted=true`. Each actual status code is carried through
the typed Unauthorized(code) result, not filled in from an assumed value.
Version/status/config/shutdown use the same verified-connection mechanism;
native log checks still reject database-stop failure and API-key disclosure.
The private config endpoint is test-only. This is local process/REST proof,
not device pairing, cross-host replication, cryptographic peer trust or
power-loss durability. A final process snapshot found no Syncthing process;
no global profile-digest audit was performed this turn.

## Proof and continuation

Proof root:
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/owned-endpoint-20261021-133848-50f3146312d04317b07fdbbfe69bf595`.

- `package-tests.log`: first fixture cleanup failure.
- `package-tests-final.log`: retained intermittent credential-deletion failure.
- `credential-isolated-1.log` through `credential-isolated-8.log`: unchanged isolated passes.
- `package-tests-recheck.log`: library/event PASS then blocked CLI launcher.
- `fmt-final.log`, `clippy-final.log`, `build-final.log`: successful static/build gates.
- `native-owned-endpoint-final.log` and `native-private-owned-endpoint-result.json`: actual native PASS/results.
- `SOURCE_SHA256SUMS_LF.txt`: same 22-file manifest as
  `m8-owned-endpoint-source-sha256-20261021.txt`, UTF-8 CRLF→LF with existing BOM retained.

Built daemon SHA-256:
`687fae8d91c496681f8f969998898bc54335e51fa923d3dd526435e6656e0be8`.
The current Windows batch runner points to the new manifest; historical ZIPs,
16-file source manifests and guest receipts retain their original meaning.
The owner has no new interactive step for this local implementation result.

Next: package/private native-config provisioning and controller lifecycle
integration, plus investigate the open credential-deletion test. Keep Windows
first and consolidate Mac work later. ADR-0019 activation gates stay closed.
No Actions, commit, push, merge, production startup/IPC activation, M7 write-back
or M8 completion.
