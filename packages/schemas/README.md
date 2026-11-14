# @cryptex/schemas

Generated JSON Schema and TypeScript bindings. V1 remains the frozen ADR-0016
contract; v2 is the separate ADR-0017 typed reader schema. M8 adds three separate
candidate contracts without modifying either frozen artifact:

| Contract | Root | Scope |
|---|---|---|
| `cryptex-bootstrap-v1` | One signed message | Closed offer, proof and both approval roles; required null peer fields on offers |
| `cryptex-bootstrap-transcript-v1` | Four-message tuple | Exact offer/proof/initiator-approval/responder-approval order |
| `cryptex-event-v3` | Security event | Three closed security variants, internal classification and exact decimal-string u64 bounds |

The M8 generator starts from the actual Rust Deserialize wire definitions and
adds their protocol constraints. The new documents use Draft7 tuple/conditional
syntax for cross-validator and TypeScript-tool compatibility. They have no
external references. Mac and Windows generation must produce identical bytes.
These are candidate artifacts: their presence does not activate a writer,
transport, publisher image policy, or ordinary Mac product route.

## Regenerate

```bash
cargo +1.98.1 run --locked -p cryptex-events --bin cryptex-events-schema-gen
pnpm --filter @cryptex/schemas install
pnpm --filter @cryptex/schemas generate:ts
```

## Drift checks

```bash
cargo test -p cryptex-events schema
pnpm --filter @cryptex/schemas check:drift
```

Rust domain types in `crates/events` are the source of truth. Do not hand-edit generated artifacts.

The generator also accepts `--output-dir DIRECTORY` for isolated tooling. It
rejects unknown arguments. The normal destination stays `packages/schemas`.

## M8 validation boundary

Schema acceptance is structural only. TypeScript types are not runtime
validators. Neither proves signatures, public-key/key-ID bindings, message hash
links, canonical JCS/raw-byte grammar and limits, duplicate-key absence, exact
author/Brain/transaction agreement, canonical target-history positions, local
owner consent or trust. Format validation must be explicitly enabled by the
consumer; Rust remains authoritative for timestamps and all protocol semantics.

The low-level v3 reader authenticates the outer envelope. Complete security
history verification additionally checks every embedded peer transcript against
the actual authenticated author's full key, including remote approval metadata.
Remote B→C approval never adds C to A's directly approved roster.

## Independent conformance

`tools/m8-protocol-vectors/check_schemas.py` uses pinned Python jsonschema with
RFC3339 format checking and the existing independent RFC8785/BLAKE3/OpenSSL
reference tools. Its 495 public synthetic cases include required/unknown/null
fields, wrong types, Serde unit-map/positional-array hazards, exact u64 boundaries,
tuple order and deliberately shape-valid invalid signatures/bindings. Mutations
are independently re-signed where appropriate, not merely rejected for stale
outer signatures. Rust executes the same corpus through its actual readers.

Run it with `python -B tools/m8-protocol-vectors/check_schemas.py` after installing
`tools/m8-protocol-vectors/schema-requirements.txt` outside production/runtime.
`--write-cases` regenerates public test data only when the contract intentionally
changes; do not regenerate expectations merely to make a test pass. The original
independent wire-vector fixture remains unchanged.

The native `schema_candidate_only` CI input generates/checks all five contracts,
tests both implementations, and exercises the actual historical product-service
guard against the unchanged historical library in an exact pre-v3 checkout.
Its typed whole-Brain hold is
separate from a frozen envelope-only refusal and from installed old Desktop or
OS-secure-store acceptance.
