# Windows host-to-VM native transport evidence

Date: 2026-11-08. Result: PASS for isolated synthetic native file transfer.
No canonical import or publication claim.

## Bound inputs and results

- Session: `87a57c018d63481cb71a9a0f6abdd1f1`.
- Native binary: pinned Syncthing 2.1.5 Windows amd64, SHA-256
  `36A0F7BC372F64FA7CC4F5654FA324C0DD9F7FEF2E07565E00C6E1CF73F50344`.
- Sender: `PE5KOBQ-NYBMJI6-IV2IRDR-FZZ2XNP-7L6VPMX-YVX7CHF-ESXDVHB-SWZLIAN`.
- Receiver: `U3R6QXX-33JPHLF-3MTQ3TG-FB25IOW-PX6WUXX-467FCB5-QCOF5VQ-B5V3YQW`.
- Original random payload: 32768 bytes, SHA-256
  `26C32D704F37FD01CBD9718E1BDA359A99B269E42507C0C587683212C6478567`.
- Owner-returned archive SHA-256:
  `C0D3D823A7E21A000CA7D9142E1AC6905DF57FA55144659A0B4D5E972017E73D`.
- Receiver report time: `2026-11-08T19:17:36.1156992Z`.

The agent checked the bounded archive's exact single-file membership, then
compared its session, both full native IDs, bytes, hash, scope, result and
shutdown flag against the retained host settings and public VM request. The
hash of the retained original source file was recomputed and matched. Sender
result is `peer_completion_observed`; receiver result is `received_hash_match`.
Both report `clean_shutdown: true`. Host command exited with status zero and
the exact owned host executable was absent from the process inventory afterward.

The host's isolated native log records a secure connection at local
`192.168.177.1:1393`, remote `192.168.177.131:50268`, at 14:17:34 America/New_York,
with TLS 1.3 / TLS_AES_128_GCM_SHA256. Receiver disconnect was observed at
14:17:36, and native host exit at 14:17:37. The VM endpoint was independently
observed in the earlier returned VM preflight. The portable package contained
neither the random source bytes nor a receiver private key.

## Scope and limits

This was an actual Windows host-to-Windows-VM network transfer, rather than
two loopback processes. It demonstrates this pinned native engine can connect
and deliver the expected synthetic bytes across these endpoints without the
harness changing firewall or antivirus settings. It does not establish a
general network/security-policy guarantee.

Reports are local test observations, not signed attestations. Receiver clean
shutdown is reported by the VM runner; the agent independently checked only
the host's remaining-process inventory. Full raw logs, keys/configs, payload
bytes and personal runtime files are not committed as evidence.

The harness directly uses isolated Syncthing profiles and a synthetic file;
it does not use existing Cryptex pairing identities, signed canonical events,
the managed production adapter or the daemon importer. `canonical_import` is
explicitly `not_run`. Normal canonical publication remains inactive. Remaining
integration, durability/platform and independent-review gates still apply.
