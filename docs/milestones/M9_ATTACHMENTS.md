# M9 — Attachments

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M9 productizes canonical attachment/blob storage and integrity behavior.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/09_FORGET_PURGE_AND_DATA_LIFECYCLE.md`, `docs/13_OBSIDIAN_INTEGRATION.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M9 includes:

- SHA-256 blob store
- deduplication
- reference tracking
- integrity repair
- projection copy/hard-link behavior

## Boundary with M6

M6 proves the **derived projection side** of attachment handling using synthetic attachment fixtures and a stable projection boundary/interface.

M6 must not implement:

- the canonical attachment blob store
- SHA-256 blob-store ownership
- deduplication
- canonical reference tracking
- integrity repair

Those responsibilities begin here in M9.

### Signed link to a note

An `attachment.link` uses the ADR-0017 v2 signed envelope and payload schema
version 1. Its payload contains exactly three fields. `note_event_id` is the
target v1 memory note's UUID, `note_content_hash` is that note's signed
content hash, and `blob_sha256` is the SHA-256 of the raw attachment bytes.
Both hashes use 64 lowercase hexadecimal characters. The envelope carries
the same Brain ID and classification as the note. The original note remains
immutable and the link contains no filename or source path.

The M9 reader resolves the link and note from one authenticated history.
The daemon verifies the local blob before reporting that the link resolves
to local bytes. Missing bytes and integrity failures have distinct outcomes.
Resolution alone grants no access to attachment content. The first writer is
exercised only in isolated tests. Production writes remain subject to the
compatible-reader and downgrade gates of ADR-0017 sections 4.5 and 8.2.

### Desktop capture in disposable profiles

The protected note detail can advertise file attachment to Desktop. The
selected-file request carries the exact note ID and content hash plus up to
512 KiB of binary data. This limit bounds the initial IPC implementation.
Only the authenticated Desktop process with the attachment capability can
submit it. The daemon checks the note and author against locked history,
stores the verified bytes and appends a signed link using the note's
classification. Retrying the same bytes for the same note acknowledges the
existing link. The guarded detail supplies the link count after indexing.

Only the test daemon contains the link writer. The automated browser test
creates its own enrolled profiles and exercises both settings of the gate.
Ordinary binaries omit the file action from note detail and reject direct
attachment requests. No filename or source path enters the IPC request.

### Attachment transfer trial

On each native CI runner two disposable profiles sign a pairing using the
transport IDs of two pinned Syncthing processes. The test driver moves reader
messages, a sealed event pack and attachment bytes through private test
folders over loopback. The sender waits for a live reader proof. The receiving
daemon imports the pack using its local trust history, resolves the signed
note/link pair and checks the incoming bytes against the link's SHA-256 before
placing them in its canonical blob store. The trial checks missing and damaged
bytes, an unrelated target, duplicate delivery and verification after reopening.
The trial's import and export permissions exist only in the test executable.
Automatic blob delivery in the installed product remains a separate step.

In the paired trial the receiver can also restore a damaged local blob from
the peer. It requires the admitted signed reference, verifies replacement bytes
in staging, and moves the original into private quarantine before publishing the
replacement under its expected SHA-256. Unrelated links, corrupt replacement
bytes and multiply linked files stop repair. A retry after interruption can fill
the missing canonical name from a verified delivery. Ordinary capture and
read-only integrity checks continue to report local corruption.

For a newly prepared M9 profile, the daemon exclusively creates a fresh
profile directory. Before the first v2 record, it reserves `daemon.lock` as
a directory and creates the regular PID lock file `daemon.lock/reader-m9.pid`.
M8 requires the outer entry itself to open as a writable regular file at
startup, even with the newer daemon stopped. Current readers reopen the
existing inner file and use the normal single-instance lock. A missing
inner file holds startup. Legacy profiles retain their ordinary lock file.

The initializer refuses an existing destination. The explicit offline
`cryptexd lifecycle migrate-profile-m9 --runtime-root <absolute-profile>`
command upgrades an existing M8 profile in place on Windows and the
controlled Mac product path. Canonical packs, device identity, signing
keys, sequence state and transport keep their original locations. The
daemon prepares and flushes the new lock directory before retiring the
old PID file under exclusive access. It publishes the directory without
replacing an intervening M8 lock. A retry can finish an interrupted
transition or confirm an already protected profile.

The PID carries no trust or canonical data. Backup rejoin must prepare a
protected destination before restoring v2 records. Production attachment
writing remains gated by ADR-0017 §8.2.

### Checking a paired reader before export

During normal enabled sync, a daemon on a protected M9 profile checks every
eligible paired reader before considering a pack that contains an attachment
link. Each daemon reads a fixed signed synthetic note and v2 link with the
production event verifier and link resolver. It checks the target note, the
classification and the digest of the referenced bytes. The vector runs in
memory and contains no user data.

The adapter carries separate signed messages under
`transport/incoming/m9-reader-v1`. Each direction has its own slot, named
by the SHA-256 of the canonical route. The Ed25519 signature covers a
protocol-specific prefix and the canonical JSON body, including Brain ID,
both device and transport IDs, a fresh nonce, the peer nonce being answered,
the daemon executable digest and the resolved blob digest. The verifier
uses the signing key from the current locked, approved device binding.
The inbox importer skips this directory.

Live challenges expire after five minutes. Restarting the controller,
changing connection settings or changing verified security history starts
a new round. An internal transport restart after an ordinary import can
reuse the unexpired challenge once the daemon rechecks the profile and
peer bindings. Only a signed reply to the current challenge counts toward
live verification.

On each fresh exchange, the daemon keeps the peer's signed reply and the
challenge it answered. It
signs that observation with the local device key and stores one item per
direction in Windows Credential Manager or macOS Keychain. The context
binds the Brain, the protected profile lock's native file identity, the
local daemon digest and the signed security-event hashes. Ordinary notes
can be captured offline without invalidating the evidence.

On a later enabled sync check, every eligible peer must have a live reply
or a matching retained observation before the daemon reports coverage.
For each retained observation it verifies both signatures, the two device
and transport bindings, the challenge, the reader result and the current
context under the canonical locks. Peers can answer in different sessions.
A signed conflicting build or reader result invalidates the previous
observation. A missing item, changed context, invalid signature or store
error prevents coverage through that item. A restored profile staged
behind a new protected lock must obtain fresh proof.

The daemon reports historical coverage as `previously_verified`. It
describes the build last demonstrated under the locally verified roster.
Remote changes can only be observed after reconnection.

### Reconnecting an already enrolled reader after restore

Before reconstruction, the daemon maintenance command
`cryptexd lifecycle prepare-reader-rejoin-m9 --runtime-root <absolute-new-directory>`
exclusively prepares a new protected destination under an existing private
parent, including a private connection-settings directory and session lock.
Restore saved preferences into that directory using its native permissions.
Success reports that the destination is prepared. Restore staging
requires the enrolled device and its signing and transport credentials. The
controller opens the restored profile through ordinary startup recovery and
uses its saved connection settings. Enabled sync runs the reader exchange
automatically. Paused sync waits for the owner to resume it.

New locally signed observations also retain the last authenticated sequence
and content hash seen for the local and peer device streams at that exchange.
Before issuing a fresh challenge or answering another peer, the controller
checks those boundaries for every currently eligible peer with surviving
evidence. A device behind a witnessed boundary waits for verified incoming
history and withholds outgoing canonical packs. A conflicting hash at the
same sequence also holds the check. The next controller tick retries after
ordinary import. Observations from earlier releases carry no such boundary.
The reader exchange alone cannot establish history the device never observed.

A new round clears the requested diagnostic receipt left by a previous
session. Both peers retain the new signed exchange even if the daemon builds
are identical and the surviving peer has stayed open. Fresh reader evidence
grants no write authority. Attachment writing and transfer are still closed.
Encrypted backup reconstruction and recovery after loss of credentials
belong to M11.

M9 connects the real canonical attachment/blob subsystem to the projection boundary already proven in M6, using the accepted hard-link/copy fallback evidence rather than redesigning the projection contract.

## Required invariants

- Attachment blobs are canonical durable data.
- Derived attachment projections are not canonical.
- Dedup/reference logic must not delete still-referenced data.
- Cross-platform projection behavior must follow accepted attachment-projection evidence.
- Integrity failures must be explicit rather than silently ignored.
- M9 must preserve the M6 projection boundary rather than making the Obsidian vault canonical.

## Non-goals

M9 does not silently add universal OCR/PDF/DOCX extraction as a V1 requirement.

Full attachment reference-aware purge behavior belongs to M10.

## Verification

M9 must verify relevant dedup/reference behavior, integrity checking/repair, projection behavior and platform-specific copy/hard-link cases.

Verification must demonstrate that the real canonical attachment backend can drive the already-proven M6 projection boundary without making generated projection output canonical.

## Completion gate

M9 is complete only when scope items and relevant tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M10 begin.
