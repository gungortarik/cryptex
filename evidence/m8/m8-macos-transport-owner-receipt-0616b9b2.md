# Mac transport identity: owner-returned result

The owner returned `MAC-AKTARIM-SONUC-0616b9b2.zip` for reviewed source
`0d98a16d075ca317b208c6e3dd6b0c66b64df707`. The archive contains exactly one
399-byte `result.json`, with no runtime directory or credentials. Archive SHA-256:
`b9ec5641117ee3b2c676ce2750d625aa36abac48f9058b6a90ddf89d5141bea3`.

The [structured receipt](macos-transport-owner-0d98a16-0616b9b2.json) preserves the
returned report and its validation. The exact source matches the reviewed
one-click package and [passing scoped CI](https://github.com/gungortarik/cryptex/actions/runs/43334196206).
The closed result schema has `outcome: passed` and all five boolean checks true:

- Identity restart and refusal controls.
- Native pairing and same-profile reopen.
- Daemon loss and unresolved replacement refusal.
- Fixture credential cleanup.
- Temporary runtime cleanup.

The reviewed collector sets these fields only after the corresponding bounded
drivers and cleanup checks succeed. This closes the owner-Mac execution gate for
this default-off, isolated local-engine/native-pairing candidate. Do not request
an unchanged package rerun or return to manual signed-JSON exchange.

This is an owner-supplied aggregate receipt, not independent device attestation.
It includes no OS version, architecture or raw runtime journal to verify
separately. It does not prove cross-device replication, a human WebView pairing
flow, combined supervisor loss or normal product activation. Earlier CI and
independent agent source review remain separate evidence.

Next: complete combined daemon/helper supervisor-loss acceptance in the existing
disabled scope, preserving unresolved-state exclusion and refusal to adopt orphan
engines. Other endpoint/exec races, product publisher/support floor, activation
and final M8 protocol/platform/security/human acceptance remain open. Any new
owner package must exercise a concrete new gate; no user action is needed for
the receipt recorded here.
