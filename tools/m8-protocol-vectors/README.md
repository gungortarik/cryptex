# Independent M8 wire reference

These fixed fixtures pin protocol bytes without invoking any Cryptex Rust code.
All seeds, identities, notes and nonces are **public synthetic test data**.
Never use them for real identities. This tool is not a canonical writer.

Install the pinned `requirements.txt` into a disposable environment outside the
repository, provide Node.js, and run `python tools/m8-protocol-vectors/generate.py
--check` from the repository root. Without `--check`, it regenerates the checked-in
JSON fixture. Fixture updates require protocol review; never regenerate expected
bytes merely to accommodate a failing candidate.

The reference assembles each domain-separated preimage independently, uses
[RFC8785](https://github.com/trailofbits/rfc8785.py) for canonical JSON,
[the official BLAKE3 Python binding](https://github.com/oconnor663/blake3-py) for
hashing, and Node's built-in OpenSSL Ed25519 for signatures (also independently
verified by Node). BLAKE3 shares the official primitive implementation; this is
independent wire/preimage assembly and a different JCS/signature implementation,
not an independent cryptographic algorithm or a formal protocol audit.

The Rust contract tests compare exact offer/proof/two-approval writer bytes,
four message hashes, agreement/transcript hashes, all three v3 event types,
cutoff strings `0`, `1`, `u64::MAX`, operation digests, four owner snapshot
digests, and a fixed v1→v2→v3→v1 chain with permanent sequence gaps. Negative
checks cover altered signed messages, order, wrong signer, sequence overflow,
impossible self-retirement at `u64::MAX`, replay and a missing chain prefix.
The frozen v1 reader rejects v3. These are wire/envelope contracts, not a claim
that the entire trust/cutoff reducer, old product Brain hold, durability matrix,
schema artifacts or final M8 acceptance have passed.

## Schema and historical-reader compatibility

Install `schema-requirements.txt` into an isolated test environment and run
`python -B tools/m8-protocol-vectors/check_schemas.py`. It checks 495 fixed
public synthetic cases: 188 messages, 303 events and four transcripts, using
Draft7 validation with RFC3339 format checking. Mutations are independently
re-signed where appropriate. Rust executes the same corpus through actual
protocol readers; structurally valid bad signatures/bindings must still fail
protocol verification. `--write-cases` intentionally regenerates this additional
corpus and the v1→v3 historical fixture; it does not rewrite the original golden
wire corpus.

Native schema-candidate CI checks all five generated contracts and runs
`run_old_reader_compat.py` against the exact historical source recorded in that
script. Only a test driver and fixture are added; historical tracked source and
dependencies remain unchanged. The old services must first expose trusted v1
content, then hold search/timeline/detail/vault/rebuild after valid signed v3,
preserving canonical bytes, database state and vault contents. This is a real
historical product-service guard check, not installed old Desktop/secure-store
acceptance. See the [source-bound review](../../evidence/m8/m8-protocol-schemas-compat-review-20261114.md).
