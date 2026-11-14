# M8 protocol schemas and compatibility — November 14

Verified implementation: `1f5e6dcbc6c855f6a44c614bf415586b23f91882`.
This completes the candidate schema, historical service-guard and signed
target-history compatibility slice. M8 remains OPEN. The complete ADR-0019
addendum remains PROPOSED; its accepted Windows implementation basis is unchanged.

## Result and behavior

Three new contracts cover bootstrap messages, the exact four-message transcript
and v3 security events. JSON Schema starts from actual Rust Deserialize wire
types; generated TypeScript describes the same closed structures. Frozen v1/v2
JSON/TypeScript and the original independently computed signed wire corpus are
unchanged. All ten generated files are byte-identical across Mac, Windows and
the checked-in files. Individual SHA-256 values are in the
[machine receipt](m8-protocol-schemas-native-1f5e6dc.json).

The independent validator checks 495 public synthetic cases: 188 messages,
303 events and four transcripts. It uses pinned Python jsonschema Draft7 with
RFC3339 format validation, independent RFC8785 assembly, official BLAKE3 and
Node/OpenSSL Ed25519 signing/verification. Mutations are re-signed where
appropriate. Rust consumes the same corpus through actual protocol readers.
Shape-valid bad signatures and incorrect Brain/transaction bindings demonstrate
that schemas do not grant authority. No Cryptex Rust writer computes the oracle.

Review found two real validation gaps, now corrected with regressions: Serde
could deserialize v3 payloads from positional arrays, and remote approval
metadata did not verify its embedded complete transcript against the actual
authenticated author's full public key. Full history now checks that binding;
valid remote B→C metadata never grants C transitive trust on A. The low-level
envelope reader remains explicitly distinct from complete history verification.

## Historical product services

Both native platforms execute the unchanged historical libraries and dependency
lock at `ae61603b517f68b63d559ccb5ee77d744176e6a6`. The harness adds only an
untracked integration driver and public fixture, checks the exact commit and
clean tracked source before/after, and emits an outside-repository receipt.

The old services first successfully evaluate/search/page/detail/export trusted
v1 content. The fixture then appends an independently signed, correctly linked
v3 approval accepted by the current complete history reader. The old dispatcher
returns `Unverifiable` with schema 3 and exact raw bytes. Search, timeline, detail
and vault must each return typed `BrainWideHold(CanonicalVerificationUnavailable)`;
rebuild must return its verification error, twice. Existing canonical prefix,
appended pack bytes, SQLite schema and all table contents, plus the entire vault
tree/sentinel remain unchanged. The v1-only positive baseline avoids crediting
an already-held v2 record as a v3 downgrade refusal.

This closes the historical product-service guard/byte-preservation proof. It
does not certify an installed old Desktop, OS secure-store downgrade or every
supported product deployment.

## Signed target-history boundaries

Actual authenticated histories contain sequences 1, u64::MAX−1 and u64::MAX,
with legitimate allocation gaps and exact hash links. Signed restrictions at
0/1/u64::MAX select respectively no content, sequence 1 and the entire prefix.
Re-signed wrong hashes, an unoccupied cutoff position and a decimal u64 overflow
are refused without altering input bytes. These tests exercise full history,
trust reduction and content selection; they do not simulate complete
SQLite/Obsidian product projection or restriction recovery.

Required daemon regressions:

- `remote_approval_metadata_is_verified_but_never_grants_transitive_trust`
- `signed_target_history_cutoffs_zero_one_and_u64_max_filter_exact_boundaries`
- `signed_restrictions_cannot_name_an_unoccupied_or_overflowed_target_boundary`

## Verification and review

[Schema/historical CI43563383271](https://github.com/gungortarik/cryptex/actions/runs/43563383271)
passed on the exact source above, both native platforms. Each passed formatting,
all-five-contract generation/drift checks, strict events clippy, 90 events unit
tests, three original wire integration tests, three new schema/chain integration
tests, the independent 495-case validator and actual historical service test.
The historical Rust driver has its own enforced native formatting check.
The [full readiness run43563385315](https://github.com/gungortarik/cryptex/actions/runs/43563385315)
also passed both platforms and Windows cross-account IPC ACL on that exact
source. Daemon library: Mac 535 passed/7 ignored, Windows 656 passed/5 ignored,
zero failures. All three new daemon regressions and the seventeen-real-anchor
startup regression passed each platform. The machine receipt records job IDs
and which separate manual jobs were not executed; ordinary Mac readiness does
not execute the Windows-only incoming publication suite.

The existing independent read-only agent reviewed the exact final source and
reported no remaining actionable finding in this slice. This is source review,
not an external human audit. Local Rust remains blocked by Application Control4551;
native CI ran the required Rust checks without bypassing local policy.

Earlier failed/superseded attempts remain recorded in the receipt: native
formatting failures, the `&str`/`&[u8]` conformance compile mismatch and the old
vault fixture's missing selected directory were corrected. Tests were not
disabled, assertions were not weakened and native budgets were not increased.
The missing vault directory was fixture setup, not a production behavior change.

## Remaining boundary

Schemas do not verify signatures, duplicate keys, JCS/raw-byte grammar, all
cross-field bindings, target-history authority or owner consent. Complete Rust
verification remains authoritative. Candidate artifacts activate no ordinary
product route. Remaining work is the uncovered §5 durability/rebuild/key-loss
product cases, complete protocol acceptance, configured Mac image policy,
macOS 26+ enforcement, supported product-path integration and final seven-area
platform/security/human acceptance. Existing unchanged owner packages are not
requested again. M9 has not begun.
