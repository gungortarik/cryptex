# Portable signed-event transport probe

Date: 2026-11-08. Owner-requested test tooling; independent Windows VM probe passed.

## Actual Windows VM result

The owner returned the bounded single-file archive `IMZALI-SONUC.zip` containing
`signed-transfer-result.json`. Archive SHA-256:
`7FC70E17A4FFDB9BB91CDAE23741B2074108636523E2A10CAE54B752F4C03784`.

- Native session: `87a57c018d63481cb71a9a0f6abdd1f1`.
- Signed probe: `c15cf5f1870648e9b6bdaa5129766cb7`.
- Bytes: 723. Received/original SHA-256:
  `2C421940C8820F49D49569141EFEB0E70BA02960037D1526BBFA2194E1F7A77F`.
- Sender: `PE5KOBQ-NYBMJI6-IV2IRDR-FZZ2XNP-7L6VPMX-YVX7CHF-ESXDVHB-SWZLIAN`.
- Receiver: `U3R6QXX-33JPHLF-3MTQ3TG-FB25IOW-PX6WUXX-467FCB5-QCOF5VQ-B5V3YQW`.
- Receiver collection time: `2026-11-08T19:38:47.3215771Z`.
- Receiver: `received_hash_match`, `signature_verified: true`,
  `tampered_copy_rejected: true`, `clean_shutdown: true`.
- Sender: matching session/hash, `peer_completion_observed`, clean shutdown.
- Scope: `trust_activated: false`, `published: false`, `canonical_import: not_run`.

The agent compared all these fields with retained host settings and public VM
request, recomputed the original sealed canonical source hash as well as the
native transport source copy, and independently reran the daemon verifier on
the original. It returned one signature-verified event with no trust/publication.
The pinned packaged daemon hash is
`7CE484A53F8AB659B1EFB4FB6BCBD79D2E915C479EBB28A3526557513633AF8C`;
public descriptor hash is
`358ACACEBF008F85DEB31E0427D4493CFF9AE13B5D5CA22F95DD83D58946D4AC`.

The host log records TLS 1.3 / TLS_AES_128_GCM_SHA256 from
`192.168.177.1:1393` to the VM's `192.168.177.131:50293` at 19:38:43 UTC,
and native exit at 19:38:46 UTC. The exact owned host executable was absent
from the subsequent process inventory. Receiver verification and clean stop
are observations reported by the VM runner, not a signed remote attestation.
No raw private logs, signing seeds or original event bytes are committed here.

## Data and verification boundary

`cryptexd pilot-fixture signed-transfer-v1 --runtime-root <fresh-marked-path>
--confirm-isolated-pilot` creates a new synthetic Brain/device in the Dev OS
secure-store namespace and appends one public note through the daemon's existing
canonical writer with seal threshold one. It exports only a public descriptor
alongside the sealed frame. Signer keys remain in OS storage. The canonical
source is copied out for the test; PowerShell never writes canonical events.

`cryptexd pilot-fixture verify-signed-transfer-v1 --segment <file>
--descriptor <public-file> --confirm-isolated-pilot` has no store/runtime writes.
It bounds reads, rejects malformed/extra frames and descriptors, uses the existing
v1 record validator for content hash and Ed25519 signature, and binds the full
Brain/device/event IDs, first sequence/zero previous hash, public classification
and fixed synthetic note. It reports `signature_verified`, one event,
`trust_activated: false`, `published: false` and scope
`synthetic_signature_only_not_trust`. It does not authenticate a peer through
the local roster and cannot authorize an import. No custom cryptography added.

The VM receives a fresh `signed-<probe-id>.seg` through the already established
native test folder. The ZIP contains the public descriptor/verifier, never the
source segment or a signing seed. After received SHA-256 and native peer
connection checks, the runner stops transport, invokes the daemon verifier,
modifies a separate copy's synthetic note, requires `probe_integrity_failed`,
and rehashes the unchanged original. One result ZIP records both checks.

## Verification performed

- Four library tests: genuine daemon-sealed event, modified body without a hash
  sidecar gate, altered signature and wrong public key, framing/size/identity
  rejection. No trust/publication success implied by a valid signature.
- Fifteen CLI tests, including explicit confirmation/argument enforcement.
- Strict daemon Clippy and cargo format checks.
- Native PowerShell regression: original random-file transfer and wrong-session/
  public-endpoint rejections, retained-profile reopen, daemon-generated signed
  segment transfer, successful signature verification, modified-copy rejection,
  and clean native shutdown. This ran with two host loopback native processes.
- Release daemon build used Windows Credential Manager successfully once the
  fixture path was shortened. Initial long-path failures were preserved and
  reported, not replaced with a fake store or an exported signing seed.

Reopening the pinned native test profiles exposed encryption-password placeholder
behavior. The scoped harness validates the exact retained test folder/device IDs,
role/path/address and resets its intentionally empty folder-device passwords;
it never round-trips a secret-redacted folder unchanged. An early sender result
race was fixed by comparing completion with the controlled source-file inventory
size, rather than a possibly stale scan-status observation. Receiver hash proof
is still required; sender completion alone is insufficient.

## Remaining gates

This package's expected public key is test metadata supplied with the verifier.
It does not establish trusted Brain membership, peer approval, trust/cutoff
continuity or retrieval policy. The full `replication validate-incoming` path
and canonical publication remain separate work. The VM's signed receipt was
obtained and bound above; basic host-to-VM synthetic transfer evidence is recorded in
`m8-windows-host-vm-native-transfer-20261108.md`. No macOS, power-loss or independent
review claim is made.
