# M8 Windows startup/status guest artifact review

Date: 2026-10-22. Owner-supplied artifacts read directly:
`results.json`, `native-provision.log`, `native-provision-result.json`.
These corroborate the earlier terminal screenshot; this is artifact review,
not an independently operated guest run or proof of guest package provenance.

## Batch result

All eight recorded steps have exit 0: fmt, package-tests, clippy, build,
native-owned, native-private, native-pinned-package and native-provision.
The individual broad-test counts and other native receipts were not supplied;
do not invent their contents from the aggregate result.

The supplied native log reports the exact
`real_pinned_profile_provision_reopen_and_collision_recovery` test PASS:
1 passed, 0 failed, 392 filtered out, 23.77 seconds. PowerShell's wrapper labels
Cargo stderr progress as NativeCommandError, but the final test result and
recorded native-provision exit are successful. The CLI-parent health-only IPC
message is expected and does not claim authenticated packaged Desktop access.

## Native receipt reviewed

| Check | Recorded evidence |
|---|---|
| Direct prepared starts/reopen/collision retry | PIDs 2944 / 8336 / 9900; clean requested-stop cycles |
| Controller clean stops | 7072 / 8436 |
| Forced owned-job loss then replacement/clean stop | 7544 -> 2084; 2917ms including backoff/start/stop |
| Cancel during backoff | 2312 only |
| Finite restart exhaustion | 6832 / 7592 / 5740 / 7588; 10181ms |
| Native identity preserved after injected faults | true |
| Worker clean stop | 936 |
| Duplicate worker/profile refused | true |
| Missing native receipt not recreated | true |
| Actual daemon managed startup/stop | daemon PID 4964 |
| Instance lock released after worker join | true |
| Foreign API endpoint refused before API bytes | true; foreign_endpoint_refused |
| Wrong-key rejection | 403 / 403 / 403 |
| Live loopback isolation / raw generation home never served | true / true |
| Exact synthetic OS credential deleted | true |

Fault scope is explicitly test-only whole-owned-job termination after healthy
observation; forced exits are not clean stops. This closes the scoped Windows
startup/worker/native lifecycle verification gate on the supplied evidence.
No power-loss, organic crash, long soak, whole-daemon-death, real packaged
Desktop status round-trip or cross-platform coverage is inferred.

Guest ZIP hash was not supplied. The exact retained package/source inventory
is documented separately; cryptographic guest-to-package linkage is not newly
claimed. Host Application Control 4551 and earlier intermittent credential/worker
failure observations remain separate/open.

Next: ADR-0019 exact pairing/bootstrap/security-event/issuer review before trust
activation, remaining incoming/convergence/cutoff coverage and consolidated Mac
stage. M8 is not complete. No Actions, commit, push, merge or new tests here.
