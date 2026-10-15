# M6 P1 — macOS attachment projection spike

**Status:** Owner-supplied summary reports exit 0. Same-volume observations recorded; Mac cross-volume case was not run.

## Source

Attachment `Spike --exit 0-- ile tamamlandı..txt`, received 2026-10-13. The report says the portable script completed successfully and gives output root `/var/folders/.../T/cryptex-m6-attachment-p1-twoulei1`. It does not include the complete OS build, architecture, Python version, raw JSON output, timing values, file hashes, or proof files; those are not independently verified here.

## Reported observations

- Same-volume hard link and copy completed for generated sizes from 0 B through 64 MiB.
- Writing through the hard link changed the shared inode, confirming source mutation risk.
- Copy failure and interrupted staging left the source unchanged.
- `--other-volume` was not supplied; cross-volume behavior was not tested on Mac.

## Interpretation

Together with the Windows P1 run, evidence confirms hard links can alias and mutate source files on both platforms. Windows C:→E: hard-link failure and verified copy fallback passed. Copy-only is the M6 policy; Mac cross-volume behavior remains a documented limit. The Mac report contains no performance numbers, so no cross-platform speed comparison or threshold is inferred.
