# M6 P1 decision — copy-only derived attachment projection

**Decision:** M6 generated attachment output uses an independent copy. Do not use hard links for projected attachments.

## Evidence

- Windows same-volume hard links and copies passed for 0 B, 4 KiB, 1 MiB, and 64 MiB synthetic fixtures. A write through a hard link changed its source. Copy outputs matched source hashes and cleanup left source bytes unchanged.
- Windows C:→E: hard-link attempt failed as expected across NTFS volumes; the copy fallback produced matching bytes.
- Owner-supplied Mac report says same-volume links/copies completed through 64 MiB, hard-link mutation changed the shared inode, and copy failure/interrupted staging preserved source bytes. A follow-up on a separate temporary APFS volume confirmed `same_volume=false`, hard-link `EXDEV` (errno 18), and size/SHA-256-verified copy fallback. See `m6-p1-attachment-projection-macos-cross-volume-20261014.md`. Timing and raw benchmark artifacts were not supplied.

Evidence files: `m6-p1-attachment-projection-windows-20261012.md`, `m6-p1-attachment-projection-macos-20261013.md`.

## Rationale and limits

Canonical attachment bytes are immutable. A hard link aliases the same file record and permitted writes to the projection changed the source on both reported platforms. A copy gives the generated vault an independent file. Windows and macOS cross-volume copy fallback are verified. Timings were small single-host observations and do not set a size threshold.

## M6 implementation constraints

- Use synthetic fixtures behind a stable projection interface; this does not implement M9 canonical blob storage, deduplication, references, or repair.
- Copy to a staging file, verify expected length and content hash, then atomically publish within the generated vault. A failed/interrupted staging operation must not expose a final attachment.
- Rebuild/cleanup may remove only paths owned by the generated projection and must never modify source fixtures.
- Surface typed errors for source read, destination write, validation, and publish failures; do not silently treat failed copies as success.
