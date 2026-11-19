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

For a newly prepared M9 profile, the daemon exclusively creates a fresh
profile directory. Before the first v2 record, it reserves `daemon.lock` as
a directory and creates the regular PID lock file `daemon.lock/reader-m9.pid`.
M8 requires the outer entry itself to open as a writable regular file at
startup, even with the newer daemon stopped. Current readers reopen the
existing inner file and use the normal single-instance lock. A missing
inner file holds startup. Legacy profiles retain their ordinary lock file.

The initializer refuses an existing destination. Any future migration or
backup rejoin must prepare a protected destination before restoring v2
records into it. The inner PID is local runtime bookkeeping. It carries no
trust or canonical data. Production attachment writing remains gated.

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
