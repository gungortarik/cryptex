# Portable Windows native transport harness

Date: 2026-11-08. Scope: test tooling only, synthetic file, no canonical writes.

The owner chose ZIP/launcher/result files over terminal commands/screenshots.
The returned preflight reports Windows 10.0.26200.0, PowerShell 5.1.26100.9711,
IPv4 192.168.177.131/24, no running Cryptex/Syncthing, no collection errors,
and both prior pairing journals anchored with four messages/one append attempt.
These journal observations are not a new cryptographic verification.

Native pin: Syncthing 2.1.5 Windows amd64,
SHA-256 `36A0F7BC372F64FA7CC4F5654FA324C0DD9F7FEF2E07565E00C6E1CF73F50344`.
The package includes the repository's pinned LICENSE/AUTHORS notices.
Fresh profile creation reuses `prepare-owned-transport-fixture.ps1` and its
protected current-user ACL. Runtime keys stay inside that private fixture;
the copied executable permits resume even if the extracted bundle moves.

Verification command:

```powershell
powershell.exe -NoProfile -File scripts/m8/test-peer-transfer.ps1 -NativeBinary <pinned-exe>
```

The test creates independent host/receiver identities, configures only their
synthetic folder, transfers 32768 random bytes on loopback, checks expected
SHA-256 plus a connected peer, and verifies clean native exits. It also rejects
a request for the wrong session before startup and rejects a public Internet
target. A complete test run succeeded after correcting initial harness failures.
The initial test setup produced cluster-config errors; explicit empty folder
device encryption passwords resolved that setup. Sender shutdown is coordinated
with receiver verification so early completion does not produce a false failure.

Receiver reporting distinguishes file hash match from canonical import, which
is always `not_run`. Sender's completion observation alone is not a receiver
hash proof. The prepared package contains no payload bytes: only the sender
fixture has the random source. Actual VM receipt will therefore require a
network transfer, not extraction of the expected file from the bundle.

No host-to-VM receipt has been obtained yet. Firewall, routing, native VM
execution and endpoint policy can still prevent it. No security settings are
changed automatically. This harness does not establish signed-event admission,
Cryptex managed-adapter integration, canonical publication, macOS behavior,
power-loss durability, independent review, or M8 acceptance.

Continuation: retain the host session under LocalAppData/Cryptex-Dev/peer-transfer.
Read the small public `peer-request.json` from the owner's `ISTEK.zip`; never
execute imported files. Match session, native hash and distinct native ID;
then run `start-peer-sender.ps1` with that session root and request path. The
owner then double-clicks `2-AKTAR.cmd` from the same bundle and returns
`AKTARIM-SONUC.zip` or `AKTARIM-HATA.zip`. Preserve the report's explicit scope.
