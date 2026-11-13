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
