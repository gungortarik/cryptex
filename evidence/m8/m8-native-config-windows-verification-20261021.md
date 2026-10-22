# M8 Windows native provisioning and session verification

Date: 2026-10-21. Branch `codex/m8-device-trust`, base HEAD
`74bc3b14127962579ae03838bafea9336612a033`; source remains uncommitted.
Host: Windows NT 10.0.26300.0, x64 process. Original dirty checkouts preserved.
Contract: `m8-native-config-boundary-contract-20261021.md`.

Proof root outside Git:
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/native-config-20261021-96c392fff82e454cbf888c9f4b7cef2a`.

## Final-source checks

| Command / target | Result |
| --- | --- |
| `cargo test --locked -p cryptexd -p cryptex-events` | PASS, exit 0 |
| events library | 61 PASS |
| daemon library | 371 PASS / 4 native fixtures ignored by default |
| CLI | 8 PASS |
| lifecycle integration | 9 PASS |
| Mode B integration | 3 PASS |
| update/rollback integration | 2 PASS |
| schema-generator / doc targets | Executed, zero tests |
| `cargo test --locked -p cryptexd --lib -- --ignored --test-threads=1` | All 4 native fixtures PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` | PASS |
| `cargo build --locked -p cryptexd` | PASS |
| Actual PowerShell 5.1 runner-helper regression | 3/3 PASS |

The new native-config unit group has 15 default entries, including one worker
helper, and one explicit native fixture. They cover strict XML, duplicate and
unsupported data, network/auth weakening, Unicode/escaping, key preservation,
partial outputs, failed enrollment recovery, ready missing credentials, changed
identity/config/ports, exclusion locks, hard links and active/unresolved-owner
replacement refusal. Abrupt worker exit 97 verifies kernel lock release and
byte-identical staged-key recovery without a new generator call; it is not a
physical power-loss test.

The extra generation-complete marker was added during review so complete-looking
files from a failed/unconfirmed generator are never taken as success. Active/
unresolved flags and independent root/home Arc pins also prevent replacement
through the same prepared object or after forgetting a session. Assertions were
added for these cases; no failing test was weakened.

quick-xml 0.42.0 was already locked/cached. The offline dependency edit adds only
the daemon dependency edge, with no release-version upgrade. Its deprecated
attribute method was replaced with the supported XML 1.0 normalization method
before final strict Clippy. The standard parser/writer handles escaping; no
external entity resolver or custom cryptography was added.

## New native fixture

`managed_profile::native::tests::real_pinned_profile_provision_reopen_and_collision_recovery`
uses a copied exact pinned 2.1.5 package/notices and a fresh user-only profile.
The product code itself generates config/cert/key in the quarantine generation
home, prepares a separate native home, enrolls the native API key in Windows
Credential Manager, starts the owned job and attests authenticated API readiness.
It does not copy the old fixture's prepared config, identity or API key.

Three verified starts: PIDs **25596 / 15736 / 39560**. Each explicit stop asserted
exit 0, forced false and a bounded clean native log. The root/private native home
were reopened between starts; native identity stayed unchanged. Native API key
was absent from native logs. Live configuration confirmed one numeric loopback
sync listener, the selected loopback GUI address, empty folders, only self device,
disabled discovery/relay/NAT/browser/crash/audit, upgrade interval 0 and usage -1.
Three actual invalid-key responses were **403 / 403 / 403**.

Before the third successful start, a synthetic foreign listener occupied the API
port. Start returned a foreign-endpoint refusal, no HTTP/API byte reached that
listener, and no adoption/port probing occurred. After removing that listener,
a new explicit start succeeded with the same identity/key. The rejected attempt
is not counted as a clean shutdown.

The unique DEV credential was deleted and an immediate read returned Missing.
A completed profile then refused preparation with the missing secret. Public
JSON receipt: `native-provision-result.json`, copied into proof. All remaining
native fixtures also passed in the final serial ignored-test invocation; their
separate public receipts are copied alongside it. No personal/global Syncthing
profile or Obsidian vault was used by the new fixture.

Final process snapshot: **0 Syncthing processes**. Debug daemon SHA-256:
`e5626e00039e75a0777b32058f375685afe68fae36e92c045e3c8a9df6fa1996`.

## Continuity and limits

New 30-file LF-normalized source manifest:
`m8-native-config-source-sha256-20261021.txt` (prior 27 plus three native files).
The Windows batch runner uses it, runs the new native fixture and copies its
receipt. Historical manifests/ZIPs and earlier guest evidence remain unchanged.
The updated whole batch script was not run here; its guarded commands were run
individually and its actual PS5.1 helper regression passed. No new VM/package
receipt or distributed installer inclusion is claimed.

The earlier independent Credential Manager read-after-delete failure remains
OPEN; its original assertion and the store adapter were not changed. The final
broad run and native exact-account deletions pass, but this does not establish
the earlier root cause or guarantee that it cannot recur.

This closes the scoped Windows native-config/session verification, not Windows
M8, production activation or all supervisor behavior. Automatic restart/status
mapping, ADR-0019 protocol/schema/security review, trust/pairing, incoming
validation/convergence/cutoff behavior, consolidated Mac, CI and human review
remain. M7 write-back stays disabled. No Actions, commit, push, merge or canonical
trust/security-event mutation occurred.
