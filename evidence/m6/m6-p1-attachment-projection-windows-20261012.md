# M6 P1 — Windows attachment projection spike

**Status:** Windows cases recorded; companion macOS same-volume observations are in `m6-p1-attachment-projection-macos-20261013.md`. Synthetic local benchmark only.

## Environment

- Windows 11 build `10.0.26300`, Python `3.13.15`.
- Harness: `scripts/m6_attachment_projection_spike.py`.
- Initial synthetic root: `C:\Users\Tarik\AppData\Local\Temp\cryptex-m6-attachment-p1-1_gxxqm1`.
- No Cryptex canonical memory, personal attachment, or SQLite data was read or modified.

## Same-volume measurements

Single sample on this host/volume; timings are observations, not a policy threshold.

| Fixture size | Hard link | Copy | Copy verified |
|---:|---:|---:|---|
| 0 B | 0.397 ms | 0.849 ms | yes |
| 4 KiB | 0.296 ms | 1.091 ms | yes |
| 1 MiB | 0.511 ms | 1.359 ms | yes |
| 64 MiB | 0.418 ms | 31.977 ms | yes |

All same-volume links resolved to the same file record. Source SHA-256 remained unchanged after deleting the projection link and after deleting the copy.

## Integrity finding

On a disposable fixture, writing through the projected hard-link path changed the source bytes (`source_changed_through_link=true`). Therefore a writable hard link does not preserve an immutable source. M6 must not choose hard links as its default projection until the production boundary can guarantee the projected file cannot mutate canonical source bytes. Copying is slower in this single 64 MiB sample but yielded an independent verified file.

## Follow-up cases

Rerun root: `C:\Users\Tarik\AppData\Local\Temp\cryptex-m6-attachment-p1-fkr47pjl`.

- Cross-volume probe used separate NTFS volume E:. Hard link was rejected with Windows error 18 (`EXDEV`); copy fallback produced a matching 4 KiB file.
- A destination-directory copy failure raised `PermissionError` (errno 13); source bytes stayed unchanged and no final output appeared. This tests a failed destination operation, not ACL-denied source access.
- Interrupted staging was simulated by writing a partial temporary file and deleting only that staging file; no final output appeared and the source remained unchanged. This is not a process-kill-during-copy test.
- A Unicode/space filename was created. A case-only filename variant collided on this Windows volume, indicating case-insensitive behavior here; no production sanitization is established.
- Second run timings: 0 B link 0.533 ms/copy 0.881 ms; 4 KiB 0.300/0.946 ms; 1 MiB 0.363/1.365 ms; 64 MiB 0.558/28.055 ms.

## Open cases / limits

- Actual process interruption during an in-progress copy, ACL-denied operations, reserved Windows names, and larger repeated samples were not measured.
- This is a small local benchmark, not a production performance conclusion. Mac replication is required before policy selection.

The JSON timings and hashes were printed by the harness. Runtime fixtures were retained under the unique temp root for inspection. No product code, Actions, push, merge, or M6 completion claim.
