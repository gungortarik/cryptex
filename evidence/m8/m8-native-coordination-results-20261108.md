# Native coordination receipt review — 2026-11-08

## Returned artifacts and binding

The owner returned the following ZIPs from Downloads. They were inspected
read-only in memory; no archive content was executed or extracted into the repo.
Both report candidate `5708cb7bad7f3bc35d43df2568414de2267a18a4` and source ZIP
SHA-256 `e7492ce071a38fbdf2453b4d959cbf9b30f13d2316fc97ae98beb84a2c8ba2e1`,
matching the [prepared packages](m8-native-coordination-packages-20261108.md).

| Returned ZIP | SHA-256 |
| --- | --- |
| `MAC-TEST-SONUC.zip` | `ad7509fb70ba1fd1f14933d90d2f04dc0396dacd4093c11f85997369c4a41465` |
| `WIN-TEST-SONUC (2).zip` | `02ee331c1e81b0d05feb988c1b7a86fcaea1b22c7cb45594a35280ea1ca10b26` |

Reported collection times are 2026-11-09 02:05:56 UTC (Mac) and 02:06:22 UTC
(Windows), the evening of November 8 in the owner's local timezone.

Mac ZIP contains exactly nine expected files: report, build log, six test-group
logs and Clippy log. Windows contains exactly ten: report, build log, six
test-group logs and two TCP logs. No duplicate, unexpected or encrypted entries.
Every member is below 1 MB; archives were read without executing their contents.

## Verified results

| Check group | Windows VM | Mac |
| --- | ---: | ---: |
| Instance ownership | 9 | 8 |
| Runtime admission | 5 | 5 |
| Incoming publication | 14 | Not compiled on Mac |
| Managed incoming controller | 4 | Not compiled on Mac |
| IPC | 66 | Not selected |
| Owned HTTP endpoints | 8 | Not selected |
| Import staging/recovery | Not selected | 58 |
| Incoming validation | Not selected | 9 |
| Trusted import probe | Not selected | 3 |
| Native Keychain | Not selected | 2 |
| Total selected tests passed | **106** | **85** |

All selected groups report zero failed and zero ignored tests. Every individual
test entry was matched to the summary count and report. Mac staging's two
subprocess interruption tests insert `running 1 test` output before their parent
`ok`; these were inspected and counted as parent tests, not extra successes.
No compile/test warnings or errors appeared in the inspected build/group logs.
Both build logs show completed test-harness compilation; build/runner exits are
zero. Mac strict all-target daemon Clippy log shows completion without warnings
or errors, with zero failed checks in the report. Windows package does not run
Clippy; earlier author lint remains separate evidence.

The five admission cases passed on both targets: draining all lease clones;
foreign-runtime/timeout refusal; panic/abandoned-exclusive recovery holds;
shutdown retaining actual ownership while workers remain; poison never reopened.
The four Windows controller cases passed for bounded queues/holds, rejection
retry suppression, readiness/publication/idle, and runtime/binding/approval checks.
Controller readiness observations are synthetic, as declared by the runner.

Windows reports OS 10.0.26200.0; the TCP probe adds build 26200.9711, 25H2.
Work and test-temp filesystem observations are NTFS. TCP summary reports 60
samples, 60 successful before/after round-trip samples, zero mismatches, only
established state 5; parser self-test passes. All eight owned-HTTP tests pass on
this candidate. This does not diagnose the separate development-host 26300 case.

Mac reports arm64, macOS 27.2/build 26B123, Rust 1.98.1 and APFS for its temporary
work volume. Filesystem observations identify the tested environment; they are
not power-loss durability evidence.

## Closure and remaining boundaries

The prepared native coordination round is complete for the exact candidate.
No package rerun, pairing repetition or persistent VM/Mac session is needed for
this checkpoint. The receipt review changed documentation only and did not rerun
the test suites locally. Results are owner-returned native logs, not independent
human certification, live machine inspection or proof of every product path.

The next bounded implementation checkpoint is an explicit isolated Windows
pilot combining actual owned transport readiness/delivery with the dormant
managed-worker publisher and concurrent IPC, including replay, restart and
failure behavior. Existing prior transfer tests and this synthetic-readiness
controller test cannot be combined into a claim that this integrated path ran.
Normal startup/watchers/renderer publication remain disabled.

Mac production ancestor-handle, no-replace publication and owned-endpoint parity,
physical filesystem/marker durability, remaining cross-device offline and
restriction/recovery behavior, development-host TCP compatibility, required CI
and human review remain open. No gate is waived. M8 remains incomplete.
