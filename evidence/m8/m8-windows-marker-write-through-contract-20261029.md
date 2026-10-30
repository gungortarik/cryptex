# M8 Windows marker write-through contract check

Date: 2026-10-29
Scope: documentation and host-filesystem evidence for the existing Windows
no-replace marker placement strategy. This does not change or activate the
publisher.

## API contract

The Windows adapter calls `MoveFileExW` with `MOVEFILE_WRITE_THROUGH` and
without `MOVEFILE_REPLACE_EXISTING` or `MOVEFILE_COPY_ALLOWED`. Microsoft
documents that the call does not return until the file is actually moved on
disk. The documentation separately says a copy/delete move is flushed before
return. The implementation uses same-volume paths, so copy/delete is not
requested. It then performs parent-handle `FlushFileBuffers` and validates a
read-back marker, but a successful directory-handle flush is not treated as the
documented durability guarantee.

Primary source: [Microsoft `MoveFileExW` documentation](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw).

## Host and existing native interruption coverage

- Windows host workspace drive `D:`: NTFS.
- Windows test temporary drive `C:`: NTFS; `std::env::temp_dir()` resolves to
  `C:\Users\Tarik\AppData\Local\Temp\`.
- Existing cross-process tests inject process termination after a canonical
  segment move, committed-marker temporary sync, and final-marker placement
  and read-back. Those tests verify recovery state after process interruption.

## Limits

This evidence does not simulate sudden power loss, storage-controller cache
loss, or filesystems other than the current NTFS test volume. It therefore
does not establish a general Windows power-loss durability guarantee. Keep the
Windows commit/replay activation gate open pending platform review and the
remaining supported-filesystem/native evidence. No code, commit, push, or
Actions were performed for this check.
