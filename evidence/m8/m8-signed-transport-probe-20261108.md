# Portable signed-event transport probe

Date: 2026-11-08. Owner-requested test tooling; independent VM execution pending.

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
and canonical publication remain separate work. The VM's signed receipt has not
yet been obtained; basic host-to-VM synthetic transfer evidence is recorded in
`m8-windows-host-vm-native-transfer-20261108.md`. No macOS, power-loss or independent
review claim is made.
