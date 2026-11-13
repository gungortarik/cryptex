# Mac combined supervisor loss: owner-returned result

The owner returned `MAC-BIRLIKTE-SONUC-7c8abb94.zip` on November 13 for reviewed
source `2890ad24359705ef34775422bccf6198d274a171`. The 494-byte ZIP contains
exactly one 374-byte `result.json`; no runtime profile or credentials. ZIP SHA-256:
`75285290e4349b91b20fa41738a20412e6d8cb76573baa124b83f7fcc970f3b1`.

The [structured receipt](macos-combined-loss-owner-2890ad2-7c8abb94.json)
preserves the report and validation. It has the exact closed result schema,
without duplicate JSON keys, exact source/scope, `outcome: passed` and four
strict boolean checks true:

- Combined daemon/helper loss with unresolved replacement refusal.
- Own-group fixture cleanup when the result reader is closed.
- Profile-lease-fenced fixture credential cleanup.
- Temporary runtime cleanup.

The exact source matches the independently reviewed one-click package and
[passing scoped CI](https://github.com/gungortarik/cryptex/actions/runs/43364276809).
The expected package SHA-256 is
`f8d844c2d6a14b49d0471a274e6f26e04d858892c64808fec99db0d2787a3d8f`.
The reviewed collector sets these fields only after both new variants and
corresponding cleanup checks succeed. This closes the owner-Mac execution gate
for combined supervisor-loss acceptance in this optimized default-off fixture.
No unchanged owner rerun or manual signed JSON is needed.

This is an owner-supplied aggregate receipt, not independent cryptographic device
attestation. No OS version, architecture, run timestamp or raw runtime journal
is supplied for independent verification. It does not establish production
orphan cleanup, a separate EOF/watchdog native case, cross-device replication or
normal product activation. Native CI and independent technical agent source
review remain separate evidence.

Next: bound and inspect the next remaining endpoint/exec race acceptance case
within the existing default-off scope. Other races, product publisher/minimum
OS decisions, normal activation and final M8 protocol/platform/security/human
acceptance remain open. No owner action is required for this recorded result.
