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

## Guarded reads after restore

The next rehearsal creates two disposable paired devices. The daemon writes a note and a signed attachment link, then holds the canonical snapshot locks while the trial archives the packs and blob. A separate fixture writer key signs the entire ZIP, including its file inventory. The expected public signing key and the trusted device anchor come from outside the backup. The harness checks the complete signature before opening the ZIP. It rejects an unsigned encrypted ZIP and verification with a different key.

After reconstruction the trial removes the source profile and rebuilds SQLite from the recovered event streams. The normal guarded detail read returns the original note and content hash. The normal attachment reader opens the exact signed note/link pair and returns the original bytes. It refuses the same link for a different existing note, reports a missing blob and detects a same-length modified blob. Changing a hex digit in the note signature also blocks projection rebuild, guarded note detail and attachment access despite the populated index. [Windows and macOS CI for 75916b0](https://github.com/gungortarik/cryptex/actions/runs/44682712870)

This test uses a previously trusted public anchor and an explicitly pinned disposable backup signer. The remaining disaster-recovery step is to establish trust on a first new device from the Recovery Kit after all ordinary trusted devices are lost. Backup-writer key enrollment, rotation and snapshot freshness also remain open for the product.

## First replacement device after loss of both source devices

The daemon rehearsal at `5505f24` completes backup creation under the snapshot locks, drops the original fake credential store and both paired device profiles, and checks that both roots are absent. A separate process decrypts the backup and verifies the backup writer's signature. The trial provisions a fresh candidate identity and credential store. Synthetic owner confirmation is represented by a signature from an offline Ed25519 recovery key whose public key was pinned before loss. Backup decryption uses a separate OpenPGP key.

The signed request binds the operation, Brain, Kit, fresh restore challenge, SHA-256 of the selected encrypted archive, historical device identity and proposed replacement identity, including both public keys. The daemon test verifies the receipt against independently expected values and checks possession of the replacement credential via the normal secure-store resolver. It exercises a missing receipt, decryption-only Kit purpose, ordinary-device signature, substituted request fields, an empty credential store and a damaged receipt. Identical retries reuse the receipt. A competing signed request for the same local restore slot cannot replace it.

After the offline signer is dropped, the candidate signs a new note with its own key. A rebuilt index and guarded search return that note with the new device ID. The archival reader derives its historical anchor from the verified grant, rebuilds the recovered projection and opens the original note and attachment. Normal event verification and the wrong-note, missing-blob, altered-blob and altered-signature cases are rerun. [Native Windows and macOS run](https://github.com/gungortarik/cryptex/actions/runs/44684314482)

The rehearsal separates the writable replacement profile from the historical read sandbox. Combining the two histories in one guarded view after restart is the next integration step. Owner interaction, a portable Kit format, independent backup freshness and coordination across separately reconstructed copies of a restore remain outside this trial. The receipt and its verifier are test-only.

## Searching both histories after restart

The updated disposable grant binds the selected archive's ordered authenticated history. Each restarted reader opens both projections from disk, verifies the signed receipt against the separately provisioned public test context, and reloads the replacement credential from a temporary file-backed test store. The offline signer is gone. The writable stream and historical archive remain separately owned.

The reader holds the two canonical retrieval guards while building a temporary SQLite view from admitted, visible notes and executing one text query. It checks the archive checkpoint, disjoint device streams and event IDs, and admits only notes from the replacement device. FTS filtering, date ordering and the result limit apply to the combined view. The fixture exercises a shared query matching both known note IDs, both authors, the original bodies and a global one-result limit.

Separate child processes exercise a damaged receipt, a modified signature in each history with populated indexes, and an archive shortened by one otherwise valid signed record. The shortened archive can rebuild its own projection but fails the grant checkpoint. Restoring the original pack and rebuilding the index permits the same search again. This is a daemon test entry point. Desktop IPC, production credential persistence, and performance on large archives need separate verification.

Both hosted platforms passed Clippy and the complete M11 rehearsal at `328e067`. [Windows and macOS CI](https://github.com/gungortarik/cryptex/actions/runs/44796685105)

## Desktop Search after reopening a recovered profile

The disposable recovery host at `ec403c0` binds the admitted replacement profile to the guarded reader for both histories. The Desktop form sends only the bounded search query. The ordinary native `content.search` dispatcher checks the local peer, capability, admission, identity and credential before calling the recovery reader. The request deadline also applies to both canonical checks and the combined FTS search.

Playwright clicks **Search** in the built Desktop interface and checks both signed event IDs in the native reply and both note bodies on screen. It closes the browser, native client and child daemon, opens fresh processes on the same files, and repeats the query. The operating system identifies the connecting client as the daemon’s parent. With the successful results visible, the test damages the disposable recovery receipt signature and clicks **Search** again. The host withholds all hits and the interface clears both notes.

The browser supplies a test Tauri invoke bridge to the native search client, which communicates over the platform IPC transport. This covers the compiled renderer and daemon search path. Installed application launch and production recovery enrollment remain separate. The recovery reader binding is available only in a test build.

Windows and macOS passed the Desktop restart check, invalid-signature check, native Clippy and earlier M11 recovery cases at `ec403c0`. [Windows and macOS CI](https://github.com/gungortarik/cryptex/actions/runs/44800505466)

## Installed Desktop recovery search

The CI-only package runs the built React interface in a Tauri system WebView. Its unique application identifier and per-launch IPC address isolate it from the user's Desktop. The test package supplies the existing native recovery reader as a sibling helper and checks the installed Desktop parent's identity before serving the guarded search. The rehearsal registers the regular Desktop search command and a fixed pass/fail observer.

The Windows runner installs the NSIS package. The macOS runner copies the application bundle into a temporary Applications directory. Each runner launches the installed executable twice on the same disposable recovered profile. On each launch, a fixed WebView action enters `prefix OR recovered`, clicks the form's Search button, and requires both complete note bodies in visible result cards. The first Desktop and reader must exit successfully before the second launch. Production recovery enrollment, packaging for distribution and real-profile writing are outside this check.

Both native jobs passed at `d421d3c`. Each recorded `both_notes_visible=true` on launches 1 and 2. The macOS ARM rehearsal package targets macOS 11.0 or later. [Windows and macOS installation and restart results](https://github.com/gungortarik/cryptex/actions/runs/44803749696)
