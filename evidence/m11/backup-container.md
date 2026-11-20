# M11 encrypted backup container rehearsal

The P1 trial selects a stored ZIP archive inside an RFC 9580 OpenPGP message. The fixture runner uses the streaming public-key encryption and decryption APIs of OpenPGP.js 6.3.2, pinned by npm integrity hash. The reader requires a version 2 integrity-protected encrypted data packet and consumes the stream through its final authentication check before extracting the archive. Upstream reports two Cure53 audits of the library. The published historical audit does not establish coverage of the pinned 6.3.2 artifact. [OpenPGP.js audit disclosure](https://docs.openpgpjs.org/#security-audit) · [Published audit scope](https://github.com/openpgpjs/openpgpjs/wiki/Cure53-security-audit) · [RFC 9580](https://www.rfc-editor.org/rfc/rfc9580.html)

## Rehearsal

The runner uses an OS temporary directory. It frames the repository's existing signed v1 synthetic note with the CXE1 header and writes 67,108,987 random bytes under their SHA-256 blob name. Before backup it records the expected member names, lengths and digests independently. A disposable search database sentinel sits outside the archive. Each encryption or decryption runs in a fresh process.

| Condition | Assertion |
| --- | --- |
| First backup | Decrypt and reconstruct both files in a fresh directory. Compare both digests. |
| Writer process killed while replacing the first backup | Wait for ciphertext on disk before terminating. Verify the previous complete file is byte-identical. |
| Retry with abandoned output still present | Write another complete backup to the target directory and restore it independently. |
| Source profile and plaintext archive removed | Recover using the saved private decryption key. Re-read the complete restored tree and compare its inventory, sizes and SHA-256 values with the originals. |
| Incomplete output, altered ciphertext, unrelated private key | All three decryption attempts fail. The restore destination stays absent. |
| Restore destination already exists | Refuse to start a restore into it. |

## Scope and continuation

The runner uses its fixture inventory as a trusted comparison oracle. Anyone with the recovery **public** key can produce ciphertext. The next step is for the daemon to establish provenance and completeness from authenticated backup inventory, signed event history and attachment references before accepting a reconstructed profile. The cryptographic envelope alone cannot make those decisions.

The temporary private decryption key models the backup-decryption portion of the Recovery Kit. M11 still needs first-device authorization with the independent recovery signing secret, derived-state rebuild, designated scheduling, retention and manual export. The interrupted-write case covers losing a writer process. Power-loss durability and provider-specific target semantics need separate evidence before release. Plaintext staging and key files in this isolated trial are test fixtures.

The [native CI run for b11b29e](https://github.com/gungortarik/cryptex/actions/runs/44681965273) executes the same rehearsal independently on hosted Windows and macOS. To repeat it in a checkout with repository Node and Python 3.12, run `npm ci --prefix tools/m11-backup-spike --ignore-scripts` and then `python -B tools/m11-backup-spike/rehearse.py`.
