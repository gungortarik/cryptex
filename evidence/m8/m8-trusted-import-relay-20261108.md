# M8 trusted-import relay preparation — 2026-11-08

## Observed preparation receipt

The owner supplied `GUVEN-HAZIRLIK.zip`, 4,120 bytes, SHA-256
`04BF510CCD9A6284DF2B2AA27E9107AB4D2254BB6F13F036882AB438DF2BADD4`.
Probe: `c223e671-3c5e-413d-a82c-419ed88a508f`.

The archive contains exactly:

- `trusted/37346162383031382d666338322d343831352d383463362d343864373238353738313834/00000000000000000000.seg`:
  6,557 bytes; SHA-256 `379ACC69362D355C764C6ADB8F2E3E4CDD423212E15BB62E74C1DED7AB2170CB`.
- `unapproved/35373164636333332d663262342d343462302d393562322d393332616631666361303865/00000000000000000000.seg`:
  723 bytes; SHA-256 `50C5CAF319D1B1E0A1C8076B5595D24A508D27D88B8AAAE4B590BCE56E64FC20`.
- `trusted-probe.json`, 1,307 bytes, SHA-256
  `2FB14EAF67618F3C232291D810F5CECDF31D08A970E1C6AC52939A804BDFAB41`.
- `unapproved-public.json`, 288 bytes, SHA-256
  `3000973E7DE01DF5E44A71BC14BBC0F4B1235A26350E86DE6661F5BB659614C2`.

The manifest reports reciprocal approval verified and publication false for:

- Brain `99e94c1a-0c62-48a0-88d2-61b9a146e021`.
- Source `74ab8018-fc82-4815-84c6-48d728578184`.
- Receiver `a61f5092-6a37-4f7c-acc6-e470d6f438c2`.
- Unapproved fixture `571dcc33-f2b4-44b0-95b2-932af1fca08e`.

The independent daemon signature-only check accepted the negative fixture's
signature. That establishes a genuine signed negative input, not membership in
the receiver's trusted roster. Signing seeds remain in VM OS secure storage.

## Final package and expected test

`%USERPROFILE%\Downloads\cryptex-guven-test-c223e671.zip` pins daemon SHA-256
`EC790763B389C98CC06139DD89DF25F3010550B758CEB6A59381B0AAC80EF59C`.
It contains nine allowlisted executable/configuration/instruction members and
no source bundle, canonical segment or key. The payload is the original receipt
archive, named `trusted-c223e6713c5e413da82c419ed88a508f.zip` in the native source.

The retained native carrier session is `87a57c018d63481cb71a9a0f6abdd1f1`:

- Host native ID `PE5KOBQ-NYBMJI6-IV2IRDR-FZZ2XNP-7L6VPMX-YVX7CHF-ESXDVHB-SWZLIAN`.
- VM native ID `U3R6QXX-33JPHLF-3MTQ3TG-FB25IOW-PX6WUXX-467FCB5-QCOF5VQ-B5V3YQW`.
- Host listener `192.168.177.1:1393`; receiver initiates the private-network connection.

These carrier identities are distinct from canonical Cryptex identities. Both
owner-approved Cryptex source and receiver profiles live inside the same VM;
the host acts only as an untrusted relay. No new trust enrollment is inferred.

After native receipt/hash verification and native shutdown, the runner invokes
the actual unchanged `replication validate-incoming --environment dev` policy
path on the original receiver. Expected cases:

1. Approved source batch accepted as `validated_only`.
2. Same-Brain unapproved signer rejected by the retrieval/trust safety gate.
3. Separate corrupted incoming inode rejected with `integrity_failure`.
4. Restored original batch accepted again.
5. Canonical `.seg`/`.open` inventory/hashes and approval journal unchanged;
   unknown device absent from canonical directories; received ZIP unchanged.

The corruption case renames the original incoming file aside and restores it;
it does not edit canonical bytes or overwrite possible hardlink aliases.
Every report must retain `published: false`. This is validation, not durable
canonical import, managed production transport, or milestone completion.

## Local verification

- `test-trusted-import-validation.ps1`: actual returned archive/schema/hashes;
  actual daemon signature verification; mocked CLI orchestration covering all
  four calls, restoration/no-change checks, refusal to mistake an unrelated CLI
  failure for policy rejection, and archive extra/traversal refusal.
- `test-signed-peer-transfer.ps1`: real local native carrier regression plus
  actual signature/tampered-copy checks; no trust/publication enabled.
- Rust `trusted_import_probe::tests::unapproved_same_brain_signer_and_corruption_reject`:
  passed with exact safety-gate and integrity rejection assertions.
- Strict daemon all-target Clippy, workspace format, PowerShell parsing,
  exact package member/daemon digest checks and `git diff --check`: passed.

The Rust change only strengthens test assertions. The packed release daemon
is the existing preparation implementation; its production policy is unchanged.
Mocked orchestration and local native regression do not establish the final VM
policy result.

## Resume / outstanding evidence

Run the host sender with the retained session, `vm-public-request.json`, package
directory `trusted-package-c223e6713c5e413da82c419ed88a508f`, and
`-WaitSeconds 1800`. Owner extracts the final ZIP in the same VM, opens
`GUVEN-TEST.cmd`, and returns `GUVEN-SONUC.zip` or `AKTARIM-HATA.zip`.
Recheck sender readiness after any restart or expired wait window.

Final VM policy receipt is pending. Match its session/probe, peer IDs,
4,120-byte payload hash, clean shutdown, all policy/no-change booleans, and
`validated_only`/publication-false results to settings and sender completion.
VM no-change fields are runner observations, not remote attestation.
Normal publication, managed-adapter integration, macOS, power-loss testing and
independent review remain separate open gates.
