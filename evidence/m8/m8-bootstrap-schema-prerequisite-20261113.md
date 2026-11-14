# Bootstrap parser/schema prerequisite — November 13

Verified source: `c060d5ce59a70c7efd64305a4ec6a3c14b5083ad`.
This is preparation for complete schema generation, not new schema artifact
acceptance, protocol activation or M8 completion.

The bootstrap reader now consumes closed Rust offer/proof/approval payload
types that also derive JsonSchema. The same field definitions can therefore
serve the parser and future generator. Existing hex widths/case, required
fields and valid approval tokens remain enforced. Writers, signed preimages,
signature checks, transcript ordering and trust activation are unchanged.

Two Serde-specific wire hazards are explicitly covered:

- Struct deserialization can accept positional arrays. The original JSON-object
  guard remains mandatory before deserialization, preserving the old wire set.
- A derived unit-enum reader can accept externally tagged objects such as
  `{"initiator":null}`. Independent source review found this regression in the
  first candidate. The corrected role reader deserializes a String first and
  then matches the two exact tokens. Object-shaped roles never reach signature
  or approval processing as valid payloads.

New tests require unknown/missing/null/wrong-type refusal, positional-array
refusal, both object-shaped roles, boolean/number/array roles, invalid role
strings and nonce case/width rejection. Derived schema tests require closed
objects, every required field, fixed-width lowercase-hex patterns and exactly
the initiator/responder string enum. Existing independent exact signed-wire
fixtures remain unchanged and must pass native CI.

Independent agent source review verified8206295 with no remaining actionable
finding; c060d5c is native formatting only. This is not an external human audit.
Local Rust execution remains blocked by Application Control4551. No bypass was
attempted. CI43558108304/60cab84 and43558273292/8206295 stopped at formatting;
the superseded43558241251/3e004af run was cancelled after the role finding.
These attempts are not passing execution evidence.

Full [CI43558441754](https://github.com/gungortarik/cryptex/actions/runs/43558441754)
passed Mac and Windows readiness plus Windows cross-account IPC ACL. Each
platform passed both new tests,88events unit tests total and all three independent
bootstrap/v3/mixed-chain exact signed-wire integration tests. Required formatting,
strict lint, release Desktop checks and frozen v1/v2 schema drift passed. Existing
ignored tests are not counted as passed. The
[machine receipt](m8-bootstrap-schema-native-c060d5c.json) pins jobs and source.
No unchanged manual Mac/VM package or default-off signed native suite is claimed
as newly executed by this ordinary readiness run.

Still open: complete closed bootstrap envelope/transcript and v3 wire schema
generation, checked-in JSON Schema and TypeScript, and full reader/schema
conformance checks. No new artifact or cryptographic-authority claim follows
from these payload definitions alone. The product gates remain closed.
