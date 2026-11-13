# M8 independent wire vectors and Windows crash fixture review

Date: 2026-11-13. Implementation `6574462d82ee7728902ac5431d0ceb76fb63fec5`;
native-format-only follow-up `e49e3182bcbced7080b841b7fa3a2c7812793533`.

## Independent source review

The existing owner-authorized independent M8 review agent inspected the exact
generator, fixtures, Rust public API calls, owner snapshot tests, Windows crash
fixture and terminal-retention proposal without edits, test execution or CI
dispatch. Final source review at `6574462` found no remaining actionable issue
in that slice. Agent source review is not an external human protocol audit.

Two review observations were corrected before acceptance:

1. The crash test's readiness publication and exit checks must share the same
   original ten-second budget. The deadline is now created immediately after
   spawn and passed through readiness, owner exit and retained-child wait.
2. Fixtures must not silently lose required cases. The tests now require exactly
   four owner snapshots with their operation/role order and exactly five v3
   envelopes with their type/cutoff inventory, including `0`, `1`, `u64::MAX`.

The Windows fixture captures the original descendant's non-inherited SYNCHRONIZE
handle before triggering owner exit, proves both processes are initially live,
and then requires owner exit 97/nonclean plus original-child handle signaling
within that single unchanged budget. It sends no cleanup/termination request
before assertions. This removes post-exit PID-generation sampling and observes
actual termination; it does not change production lifecycle code or deadlines.
Microsoft documents process termination as asynchronous and prescribes waiting
on the process handle when completion is required:
[TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess),
[WaitForSingleObject](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject).

## Reference independence and exact scope

The developer-only generator calls no Cryptex executable or Rust library. It
assembles wire/preimage data independently with RFC8785 0.1.4, official BLAKE3
Python binding 1.0.11, standard SHA-256 and Node built-in OpenSSL Ed25519. The
canonicalizer/signature implementation differs from the candidate Rust path;
the official BLAKE3 primitive is shared, not an independent algorithm audit.
Node independently verifies each signature it produces. All seeds/identities,
nonces and memory content are explicitly public synthetic test data.

Checked-in fixture SHA-256:
`a61357dd60be32d5d0a6cda6c8df2c0167562e8c4984961e01bcf1d8defd3b2e`.
Local deterministic regeneration with `--check` passed twice. Existing v1/v2
TypeScript drift checks and `git diff --check` passed. Reference dependencies
were installed into a disposable external TEMP directory, not repo runtime or
the global environment. Application Control4551 blocks local Rust executables;
no policy bypass was attempted. Native test status is recorded separately.

The three events integration tests plus one daemon snapshot test cover exact
bootstrap writer bytes, four message hashes, agreement/transcript hashes, v3
writer bytes/content/operation digests, both admission roles, restriction and
retirement display digests, and an independent mixed v1→v2→v3→v1 chain with
permanent sequence gaps. Negatives exercise signed message tampering/order,
wrong signer, sequence overflow/leading zero, impossible self-retirement at
maximum sequence, replay and missing chain prefix. A frozen v1 envelope reader
refuses the v3 records without changing its contract.

Cutoff nonzero content hashes in the envelope vectors are placeholders. They do
not prove a real signed target history or before/at/after policy behavior. The
legacy envelope refusal does not prove whole-product Brain hold or on-disk byte
preservation. Strict v3/bootstrap schema artifacts, remaining durability/rebuild
cases, terminal journal migration, complete protocol acceptance and ordinary Mac
product integration/activation remain open. The default-off candidate's prior
native/owner receipts retain their exact original source scope.

## Broader baseline finding

Full readiness run [43502575940](https://github.com/gungortarik/cryptex/actions/runs/43502575940)
on `06b0c29` passed Mac readiness and Windows cross-account IPC ACL. Standard-user
Windows daemon tests reported 633 passed, one failed, five ignored: the old
immediate-PID crash assertion failed. No other failure was reported there; later
Windows steps were skipped after it. This is preserved as failed baseline
evidence, not relabeled green or attributed to antivirus/OS failure.

Both readiness runs `43505083629` and `43505385350` stopped at new-test Rust
formatting. Their native formatter output was applied exactly, with the final
single tuple-format correction in `e49e318`; no test was disabled or expected
wire bytes regenerated to accommodate a candidate failure.

Final full [CI43505609309](https://github.com/gungortarik/cryptex/actions/runs/43505609309)
on `e49e318` passed Mac readiness, Windows readiness and Windows cross-account
IPC ACL. All four new reference tests passed on each platform; the retained-child
Windows crash test passed. Daemon library: Mac514passed/7ignored and
Windows635passed/5ignored, zero failures. Events86unit+3reference integration
tests passed per platform. Both jobs passed release Desktop build/tests/strict
lint and Node recovery/packaging checks. The
[machine receipt](m8-protocol-vectors-native-e49e318.json) records exact job IDs,
fixture hash and explicit non-executed gates. Ignored/scoped-native/owner tests
are not silently included in ordinary readiness acceptance.

The terminal journal issue was independently identified in the same review:
all terminal rows count toward the sixteen-pending-entry cap. The concrete
[retention proposal](../../docs/proposals/m8-terminal-journal-retention.md) is
selected for implementation after the owner's November13 delegation. The owner
also selected macOS26+ as sufficient. Migration and OS-floor enforcement are
not implemented by this test-only commit. The complete protocol and ordinary
product activation remain unaccepted.
